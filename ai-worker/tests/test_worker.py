"""Tests for photostudio_worker (run with the system python; no torch needed —
a fake backend is injected)."""

import io
import json
import os
import sys
from contextlib import redirect_stdout

import numpy as np
import pytest
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import photostudio_worker as pw


def make_jpeg(path, w=48, h=32):
    arr = (np.random.rand(h, w, 3) * 255).astype("uint8")
    Image.fromarray(arr).save(path, "JPEG", quality=90)


class FakeBackend:
    device_name = "FakeCPU (test)"

    def load_model(self, model_path, scale, fp16):
        assert os.path.isfile(model_path)
        return {"scale": scale, "fp16": fp16}

    def enhance(self, img_bgr, upsampler, scale, tile, progress_cb):
        h, w = img_bgr.shape[:2]
        # "upscale" by nearest neighbour
        out = np.repeat(np.repeat(img_bgr, scale, axis=0), scale, axis=1)
        tiles = 4
        for i in range(tiles):
            progress_cb("inference", i, tiles, f"tile {i}")
        progress_cb("inference", tiles, tiles, "done")
        return out


def run_enhance(argv, backend):
    buf = io.StringIO()
    with redirect_stdout(buf):
        rc = pw.main(argv, enhance_backend=backend)
    lines = [json.loads(l) for l in buf.getvalue().splitlines() if l.strip()]
    return rc, lines


def test_choose_tile():
    assert pw.choose_tile(100, 100, 0) == 0
    assert pw.choose_tile(100, 100, 256) == 256
    assert 128 <= pw.choose_tile(100, 100, -1) <= 512
    assert pw.choose_tile(8000, 6000, -1) == 128  # huge image -> smallest tile
    assert pw.choose_tile(500, 400, -1) == 512


def test_enhance_happy_path(tmp_path):
    inp = tmp_path / "in.jpg"
    out = tmp_path / "out" / "result.jpg"   # nested dir must be created
    models = tmp_path / "models"
    models.mkdir()
    model_file = models / "RealESRGAN_x4plus.pth"
    model_file.write_bytes(b"fake-weights")
    make_jpeg(inp)

    argv = [
        "enhance",
        "--input", str(inp),
        "--output", str(out),
        "--models-dir", str(models),
        "--model", "realesrgan_x4plus",
        "--model-file", model_file.name,
        "--scale", "4",
        "--tile", "-1",
        "--fp16", "0",
    ]
    rc, lines = run_enhance(argv, FakeBackend())
    assert rc == 0
    kinds = [l["kind"] for l in lines]
    assert "progress" in kinds
    assert kinds[-1] == "result"
    result = lines[-1]["result"]
    assert result["scale"] == 4
    assert result["model"] == "realesrgan_x4plus"
    assert result["device"] == "FakeCPU (test)"
    assert result["elapsed_ms"] >= 0
    # output written, 4x dimensions
    assert out.is_file()
    with Image.open(out) as im:
        assert im.size == (48 * 4, 32 * 4)
    # progress lines are well-formed
    for l in lines:
        if l["kind"] == "progress":
            p = l["progress"]
            assert 0 <= p["percent"] <= 100
            assert p["total"] >= p["current"] >= 0


def test_enhance_missing_model(tmp_path):
    inp = tmp_path / "in.jpg"
    make_jpeg(inp)
    argv = [
        "enhance", "--input", str(inp), "--output", str(tmp_path / "o.jpg"),
        "--models-dir", str(tmp_path), "--model", "realesrgan_x4plus",
        "--model-file", "missing.pth", "--scale", "4", "--tile", "-1", "--fp16", "0",
    ]
    rc, lines = run_enhance(argv, FakeBackend())
    assert rc == 1
    assert lines[-1]["kind"] == "error"
    assert "model file not found" in lines[-1]["error"]


def test_enhance_undecodable_input(tmp_path):
    inp = tmp_path / "in.jpg"
    inp.write_bytes(b"not an image")
    models = tmp_path / "models"
    models.mkdir()
    (models / "m.pth").write_bytes(b"x")
    argv = [
        "enhance", "--input", str(inp), "--output", str(tmp_path / "o.jpg"),
        "--models-dir", str(models), "--model", "m", "--model-file", "m.pth",
        "--scale", "2", "--tile", "0", "--fp16", "0",
    ]
    rc, lines = run_enhance(argv, FakeBackend())
    assert rc == 1
    assert lines[-1]["kind"] == "error"


def test_selftest_reports_missing_torch_gracefully():
    buf = io.StringIO()
    with redirect_stdout(buf):
        try:
            import torch  # noqa
            rc = pw.main(["selftest"])
        except ImportError:
            rc = pw.main(["selftest"])
    line = json.loads(buf.getvalue().splitlines()[-1])
    assert line["kind"] == "selftest"
    assert isinstance(line["ok"], bool)


def test_read_write_image_roundtrip(tmp_path):
    p = tmp_path / "x.jpg"
    arr = (np.random.rand(20, 30, 3) * 255).astype("uint8")
    img_bgr = arr[:, :, ::-1].copy()
    assert pw.write_image(str(p), img_bgr)
    back = pw.read_image(str(p))
    assert back.shape == (20, 30, 3)
    assert back.dtype == "uint8"
