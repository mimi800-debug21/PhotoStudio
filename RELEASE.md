# PhotoStudio 0.1.0 — Windows release artifacts

Built on 2026-11-19 from this repository (cross-compiled on Linux aarch64 with
`x86_64-pc-windows-gnu` + mingw-w64; NSIS installer produced with makensis).

| File | SHA-256 | Description |
|---|---|---|
| `releases/PhotoStudio_0.1.0_x64-setup.exe` | `0d4520fa5dc0c00b390af3719d9a0c3220ee940110eddca05449b7c229a26e20` | NSIS installer (installs app + AI sidecar + WebView2 loader, Start Menu shortcut, uninstaller) |
| `releases/PhotoStudio.exe` | `e0c517ab5a80e5f09c1ef47be67f3f29a2b8167fd1bdd0d51e62d4a84a9ee0b3` | Standalone app executable (frontend embedded) |
| `releases/ai-worker-x86_64-pc-windows-gnu.exe` | `d499a7fe5744f0f1c754a64e866b362a86bb6c8d5540c0e1b2824b15e3a15d02` | AI sidecar (bundled inside the installer as `ai-worker.exe`; also shipped standalone for dev) |

## What's inside

- React/TS frontend embedded in the exe (library, editor, AI enhance, albums,
  timeline, settings)
- `photostudio-core` (SQLite library, EXIF, non-destructive edit pipeline,
  thumbnails, SHA-256 duplicate detection)
- `ai-worker` sidecar — manages the portable AI runtime
  (`%APPDATA%\com.photostudio.desktop\ai-runtime`) and runs Real-ESRGAN jobs

## First run on Windows

1. Run the installer (or `PhotoStudio.exe` with `ai-worker.exe` next to it).
2. Import a folder of photos from the Library view.
3. For AI upscaling: Settings → AI Setup downloads the portable Python runtime
   + SHA-256-verified Real-ESRGAN models (CUDA if a GPU is present, else CPU).

## Rebuild

```bash
scripts/build-windows.sh        # from a Linux host with mingw-w64, or on Windows
```

## Test status at release time

- Frontend unit tests (vitest): 13 passed
- Rust unit tests (`rust/`: core + worker): 24 passed
- Python worker tests (pytest): 6 passed
- Sidecar end-to-end (spawn binary → ping/status/selftest/enhance → job-done,
  4× output verified): passed
- `cargo check` for host (aarch64-linux) and `x86_64-pc-windows-gnu`: clean
- `npx tauri build --target x86_64-pc-windows-gnu`: NSIS installer produced
