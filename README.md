# PhotoStudio

A fully local **Windows desktop application** for photo management and editing:

- **Library** — import folders (read-only on originals), virtualized thumbnail grid, search, sort, favorites, duplicate detection (SHA-256)
- **EXIF** — capture time, camera, lens, ISO, aperture, focal length, exposure, GPS
- **Editor** — non-destructive crop / rotate / flip / exposure / contrast / brightness / saturation / sharpen with undo-redo, live preview, atomic JPEG export (originals never modified)
- **Albums & Timeline** — organize photos, browse by capture date
- **AI Enhance** — local Real-ESRGAN upscaling (2×/4×) with GPU (CUDA) or CPU; first-run setup downloads a portable Python runtime + SHA-256-verified models into `%APPDATA%\PhotoStudio\ai-runtime` — no system Python/Node/CUDA needed
- **Settings** — theme, processing defaults, database backup, runtime self-test

All data stays on the PC. No account, no cloud.

## Architecture

```
index.html / src/          React + TypeScript + Tailwind frontend (Vite)
src-tauri/                 Tauri v2 app shell (Rust): commands, dialogs, AI bridge
rust/core/                 photostudio-core: SQLite library, scanning, EXIF, edit pipeline, thumbnails
rust/worker/               ai-worker sidecar binary: manages the portable AI runtime, runs jobs
ai-worker/                 photostudio_worker.py — Real-ESRGAN inference script (runs inside the runtime)
dist/                      built frontend (produced by `npm run build`)
```

The frontend talks to Rust through Tauri commands (`src/lib/api.ts`) and receives
AI progress through the `ai-event` Tauri event.

## Development

```bash
npm install
npm run dev          # Vite dev server on :1420
npm run tauri dev    # desktop window (needs Rust + webkit2gtk on Linux)
npm test             # frontend unit tests (vitest)
cd rust && cargo test          # core + worker unit tests
cd ai-worker && python3 -m pytest tests/   # python worker tests
```

## Building the Windows installer

On a Windows machine (or a Linux host with mingw-w64 + the `x86_64-pc-windows-gnu`
Rust target — see `scripts/build-windows.sh`):

```bash
# 1. build the AI sidecar for Windows and place it where Tauri expects it:
cargo build --release -p photostudio-ai-worker --target x86_64-pc-windows-gnu
mkdir -p src-tauri/binaries
cp rust/target/x86_64-pc-windows-gnu/release/ai-worker.exe \
   src-tauri/binaries/ai-worker-x86_64-pc-windows-gnu.exe

# 2. build frontend + app + NSIS installer:
npm run build
npx tauri build -- --target x86_64-pc-windows-gnu
# -> src-tauri/target/x86_64-pc-windows-gnu/release/bundle/nsis/PhotoStudio_0.1.0_x64-setup.exe
```

Set `"targets": ["all"]` in `src-tauri/tauri.conf.json` on Windows to additionally
produce an MSI (requires WiX, Windows only).

At runtime the app looks for `ai-worker-<triple>.exe` next to the main executable
(override with the `PHOTOSTUDIO_AI_WORKER` env var, e.g. in dev).

## First-run AI setup (on the end user's PC)

Settings → AI Setup downloads, all SHA-256 verified:

1. portable Python 3.11 (python.org embeddable zip)
2. pip + packages (`torch`/`torchvision` from the CUDA 12.1 wheel index with CPU
   fallback, `realesrgan`, `opencv-python-headless`, …)
3. Real-ESRGAN models (`RealESRGAN_x4plus.pth`, `RealESRGAN_x2plus.pth`)

…into `%APPDATA%\PhotoStudio\ai-runtime`, then runs a self-test and reports
GPU/CPU mode.

## License

MIT
