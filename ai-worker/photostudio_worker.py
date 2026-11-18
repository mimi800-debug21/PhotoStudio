#!/usr/bin/env python3
"""PhotoStudio AI worker.

Runs inside the portable Python runtime managed by the `ai-worker` sidecar.
Speaks JSON-lines on stdout:

  commands:
    selftest                                  -> {"kind":"selftest", ...}
    enhance --input I --output O --models-dir D --model M --model-file F
            --scale N --tile T --fp16 0|1     -> progress... then
                                                 {"kind":"result","result":{...}}
                                                 or {"kind":"error","error":"..."}

  progress line: {"kind":"progress","progress":{"phase":..,"current":..,"total":..,"percent":..,"message":..}}

The heavy lifting (Real-ESRGAN) is imported lazily so `selftest` and argument
errors work even before torch is installed. The backend is injectable for
tests (see tests/test_worker.py).
"""

import argparse
import json
import os
import sys
import time
import traceback


def emit(obj):
    print(json.dumps(obj), flush=True)


def emit_progress(phase, current, total, message):
    percent = (current / total * 100.0) if total > 0 else 0.0
    emit({
        "kind": "progress",
        "progress": {
            "phase": phase,
            "current": int(current),
            "total": int(total),
            "percent": percent,
            "message": message,
        },
    })


# --------------------------------------------------------------------- selftest

def cmd_selftest(args):
    out = {
        "kind": "selftest",
        "ok": False,
        "torch_version": None,
        "cuda_available": False,
        "cuda_version": None,
        "device_name": None,
        "realesrgan": False,
        "error": None,
    }
    try:
        import torch  # noqa
        out["torch_version"] = torch.__version__
        out["cuda_available"] = bool(torch.cuda.is_available())
        out["cuda_version"] = getattr(torch.version, "cuda", None)
        if out["cuda_available"]:
            try:
                out["device_name"] = torch.cuda.get_device_name(0)
            except Exception:
                out["device_name"] = "CUDA device"
    except Exception as e:
        out["error"] = f"torch: {type(e).__name__}: {e}"
    try:
        import realesrgan  # noqa
        out["realesrgan"] = True
    except Exception as e:
        if out["error"] is None:
            out["error"] = f"realesrgan: {type(e).__name__}: {e}"
    out["ok"] = out["torch_version"] is not None and out["realesrgan"]
    emit(out)
    return 0 if out["ok"] else 1


# --------------------------------------------------------------------- backend

class RealESRGANBackend:
    """Real-ESRGAN inference backend (torch)."""

    def __init__(self):
        import torch
        self.torch = torch
        self.device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
        self.device_name = (
            torch.cuda.get_device_name(0) if torch.cuda.is_available() else "CPU"
        )

    def load_model(self, model_path, scale, fp16):
        from realesrgan import RealESRGANer
        half = bool(fp16) and self.device.type == "cuda"
        return RealESRGANer(
            scale=scale,
            model_path=model_path,
            device=self.device,
            tile=0,          # we tile manually to report progress
            tile_pad=10,
            pre_pad=0,
            half=half,
        )

    def enhance(self, img_bgr, upsampler, scale, tile, progress_cb):
        """img_bgr: HxWx3 uint8 BGR (cv2). Returns upscaled BGR image."""
        h, w = img_bgr.shape[:2]
        if tile is None or tile <= 0:
            progress_cb("inference", 0, 1, "Upscaling (single pass)…")
            out, _ = upsampler.enhance(img_bgr, outscale=scale)
            progress_cb("inference", 1, 1, "Done")
            return out
        # manual tiling with overlap, cropped back after each tile
        pad = 10
        tiles_x = max(1, -(-w // tile))
        tiles_y = max(1, -(-h // tile))
        total = tiles_x * tiles_y
        canvas = None
        done = 0
        for ty in range(tiles_y):
            for tx in range(tiles_x):
                x0 = max(0, tx * tile - pad)
                y0 = max(0, ty * tile - pad)
                x1 = min(w, (tx + 1) * tile + pad)
                y1 = min(h, (ty + 1) * tile + pad)
                sub = img_bgr[y0:y1, x0:x1]
                out, _ = upsampler.enhance(sub, outscale=scale)
                # crop the pad region (scaled)
                cx0 = (tx * tile - x0) * scale
                cy0 = (ty * tile - y0) * scale
                cx1 = cx0 + min(tile, w - tx * tile) * scale
                cy1 = cy0 + min(tile, h - ty * tile) * scale
                out = out[cy0:cy1, cx0:cx1]
                if canvas is None:
                    canvas = __import__("numpy").zeros(
                        (h * scale, w * scale, 3), dtype=out.dtype
                    )
                canvas[
                    ty * tile * scale : ty * tile * scale + out.shape[0],
                    tx * tile * scale : tx * tile * scale + out.shape[1],
                ] = out
                done += 1
                progress_cb(
                    "inference", done, total,
                    f"Tile {done}/{total} ({tx + 1},{ty + 1}) of {tiles_x}x{tiles_y}",
                )
        return canvas


def choose_tile(width, height, requested):
    """requested: -1 = auto, 0 = no tiling, >0 = fixed tile size."""
    if requested == 0:
        return 0
    if requested > 0:
        return requested
    # auto: aim for ~1-4 MP per tile, clamped to [128, 512]
    total = width * height
    for t in (512, 384, 256, 192, 128):
        if total <= t * t * 4:
            return t
    return 128


# --------------------------------------------------------------------- image IO

def read_image(path):
    """Read an image as an HxWx3 uint8 BGR array (cv2 convention)."""
    try:
        import cv2
        img = cv2.imread(path, cv2.IMREAD_COLOR)
        if img is not None:
            return img
    except ImportError:
        pass
    from PIL import Image
    import numpy as np
    with Image.open(path) as im:
        arr = np.asarray(im.convert("RGB"))
    return arr[:, :, ::-1].copy()  # RGB -> BGR


def write_image(path, img_bgr, quality=95):
    try:
        import cv2
        return bool(cv2.imwrite(path, img_bgr, [cv2.IMWRITE_JPEG_QUALITY, quality]))
    except ImportError:
        from PIL import Image
        rgb = img_bgr[:, :, ::-1]
        Image.fromarray(rgb).save(path, "JPEG", quality=quality)
        return True


# --------------------------------------------------------------------- enhance

def cmd_enhance(args, backend=None):
    t0 = time.time()
    try:
        model_path = os.path.join(args.models_dir, args.model_file)
        if not os.path.isfile(model_path):
            emit({"kind": "error", "error": f"model file not found: {model_path}"})
            return 1
        img = read_image(args.input)
        if img is None or img.size == 0:
            emit({"kind": "error", "error": f"cannot decode input image: {args.input}"})
            return 1
        h, w = img.shape[:2]

        emit_progress("load", 0, 4, f"Loading model {args.model}…")
        if backend is None:
            backend = RealESRGANBackend()
        device = backend.device_name
        upsampler = backend.load_model(model_path, args.scale, args.fp16)
        emit_progress("load", 1, 4, f"Model loaded on {device}")

        tile = choose_tile(w, h, args.tile)
        emit_progress("prepare", 2, 4, f"Input {w}x{h}, tile={tile if tile > 0 else 'none'}")

        emit_progress("inference", 0, 100, "Starting inference…")
        out = backend.enhance(
            img, upsampler, args.scale, tile,
            lambda phase, cur, tot, msg: emit_progress(phase, cur, tot, msg),
        )

        emit_progress("save", 3, 4, "Writing output…")
        out_dir = os.path.dirname(os.path.abspath(args.output))
        os.makedirs(out_dir, exist_ok=True)
        if not write_image(args.output, out, quality=95):
            emit({"kind": "error", "error": f"failed to write output: {args.output}"})
            return 1
        oh, ow = out.shape[:2]
        emit_progress("save", 4, 4, "Saved")
        elapsed_ms = int((time.time() - t0) * 1000)
        emit({
            "kind": "result",
            "result": {
                "input": os.path.abspath(args.input),
                "output": os.path.abspath(args.output),
                "scale": args.scale,
                "model": args.model,
                "device": device,
                "elapsed_ms": elapsed_ms,
                "width": int(ow),
                "height": int(oh),
            },
        })
        return 0
    except KeyboardInterrupt:
        emit({"kind": "error", "error": "cancelled"})
        return 130
    except Exception as e:
        emit({"kind": "error", "error": f"{type(e).__name__}: {e}", "trace": traceback.format_exc(limit=3)})
        return 1


# --------------------------------------------------------------------- main

def build_parser():
    p = argparse.ArgumentParser(prog="photostudio_worker")
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("selftest", help="print runtime capabilities as JSON")
    pe = sub.add_parser("enhance", help="upscale an image with Real-ESRGAN")
    pe.add_argument("--input", required=True)
    pe.add_argument("--output", required=True)
    pe.add_argument("--models-dir", required=True)
    pe.add_argument("--model", required=True, help="model id, e.g. realesrgan_x4plus")
    pe.add_argument("--model-file", required=True, help="file name inside models-dir")
    pe.add_argument("--scale", type=int, default=4)
    pe.add_argument("--tile", type=int, default=-1, help="-1 auto, 0 none, >0 fixed px")
    pe.add_argument("--fp16", type=int, default=0)
    return p


def main(argv=None, enhance_backend=None):
    args = build_parser().parse_args(argv)
    if args.cmd == "selftest":
        return cmd_selftest(args)
    if args.cmd == "enhance":
        return cmd_enhance(args, backend=enhance_backend)
    return 2


if __name__ == "__main__":
    sys.exit(main())
