# -*- mode: python ; coding: utf-8 -*-
# Onedir: unzip dist/timewarp/ and run timewarp / timewarp.exe
# From the repo root: pyinstaller --noconfirm --clean timewarp.spec

from PyInstaller.utils.hooks import collect_all

datas: list = []
binaries: list = []
hiddenimports: list = ["timewarp", "zoneinfo", "tzdata"]
for pkg in ("holidays", "tzdata", "sgp4", "rich"):
    d, b, h = collect_all(pkg)
    datas += d
    binaries += b
    hiddenimports += h

a = Analysis(
    ["packaging/entry.py"],
    pathex=[],
    binaries=binaries,
    datas=datas,
    hiddenimports=hiddenimports,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
)
pyz = PYZ(a.pure)
exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name="timewarp",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    console=True,
    disable_windowed_traceback=False,
)
coll = COLLECT(
    exe,
    a.binaries,
    a.datas,
    strip=False,
    upx=False,
    name="timewarp",
)
