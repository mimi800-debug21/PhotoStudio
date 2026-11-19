#!/usr/bin/env bash
# Build the complete Windows distribution of PhotoStudio from this repo.
#
# Works on a Windows machine (MSVC or GNU) and on Linux with:
#   sudo apt-get install gcc-mingw-w64-x86-64 binutils-mingw-w64-x86-64
#   rustup target add x86_64-pc-windows-gnu
# plus ~/.cargo/config.toml:
#   [target.x86_64-pc-windows-gnu]
#   linker = "x86_64-w64-mingw32-gcc"
#   ar = "x86_64-w64-mingw32-ar"
#
# Output: src-tauri/target/x86_64-pc-windows-gnu/release/bundle/nsis/*.exe
#         (copied to releases/)
set -euo pipefail
cd "$(dirname "$0")/.."

TARGET=x86_64-pc-windows-gnu

echo "==> 1/4 cargo: AI sidecar ($TARGET)"
cargo build --release -p photostudio-ai-worker --target "$TARGET"

mkdir -p src-tauri/binaries
cp "rust/target/$TARGET/release/ai-worker.exe" "src-tauri/binaries/ai-worker-$TARGET.exe"
echo "    sidecar: src-tauri/binaries/ai-worker-$TARGET.exe"

echo "==> 2/4 frontend"
npm run build

echo "==> 3/4 tauri app + NSIS installer ($TARGET)"
npx tauri build -- --target "$TARGET"

echo "==> 4/4 collect artifacts"
mkdir -p releases
BUNDLE="src-tauri/target/$TARGET/release/bundle"
cp "$BUNDLE"/nsis/*.exe releases/ 2>/dev/null || true
cp "src-tauri/target/$TARGET/release/photostudio-app.exe" "releases/PhotoStudio.exe" 2>/dev/null || true
ls -la releases/
echo "==> done. Windows installer is in releases/"
