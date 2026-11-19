# Optional PyInstaller spec — only needed if you want a frozen ai-worker.exe
# instead of the Rust sidecar + downloaded runtime approach.
# pyinstaller photostudio-worker.spec
# -*- mode: python ; coding: utf-8 -*-
a = Analysis(
    ['photostudio_worker.py'],
    pathex=[],
    binaries=[],
    datas=[],
    hiddenimports=['torch', 'torchvision', 'realesrgan', 'basicsr', 'cv2'],
    hookspath=[],
    runtime_hooks=[],
    excludes=[],
)
pyz = PYZ(a.pure)
exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    name='ai-worker',
    console=True,
)
