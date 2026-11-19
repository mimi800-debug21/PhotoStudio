# ai-worker — PhotoStudio local AI inference

`photostudio_worker.py` is the inference script that runs **inside the portable
Python runtime** managed by the Rust `ai-worker` sidecar (`rust/worker`). The
sidecar embeds this file (via `include_str!`) and writes it into
`%APPDATA%\PhotoStudio\ai-runtime\photostudio_worker.py` during first-run setup,
then invokes it as:

```
python.exe -u photostudio_worker.py selftest
python.exe -u photostudio_worker.py enhance --input I --output O --models-dir D \
    --model realesrgan_x4plus --model-file RealESRGAN_x4plus.pth \
    --scale 4 --tile -1 --fp16 0
```

It speaks JSON-lines on stdout (`selftest` / `progress` / `result` / `error`),
which the sidecar forwards to the app as `ai-event`s.

## Tests

The backend is injectable, so tests run without torch:

```bash
python3 -m pytest tests/ -q     # needs: pytest numpy pillow
```

## requirements.txt

Installed into the portable runtime by the sidecar during setup (torch is
installed separately from the CUDA wheel index when available).

## Optional: freeze with PyInstaller

The sidecar design does **not** need a frozen worker (the runtime is downloaded
at first run), but if you prefer a self-contained `ai-worker.exe` on Windows:

```bat
pip install pyinstaller
pyinstaller photostudio-worker.spec
```
