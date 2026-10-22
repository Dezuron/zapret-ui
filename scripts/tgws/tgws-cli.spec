# -*- mode: python ; coding: utf-8 -*-
"""Builds the headless Telegram proxy consumed by zapret-ui.

The proxy is an unmodified copy of Flowseal/tg-ws-proxy (MIT). Upstream's
Windows release is a tray GUI; this spec bundles only the console entry point,
without pystray/customtkinter/Pillow, so zapret-ui can supervise it as a child
process and drive it entirely through CLI flags.

Set `TGWS_UPSTREAM` to point at the upstream checkout; CI checks it out into
`./upstream`.
"""

import os

from PyInstaller.utils.hooks import collect_data_files

ROOT = os.environ.get("TGWS_UPSTREAM") or os.path.abspath(
    os.path.join(SPECPATH, "..", "..", "upstream")
)
ENTRY = os.path.abspath(os.path.join(SPECPATH, "entry.py"))

certifi_datas = collect_data_files("certifi")

a = Analysis(
    [ENTRY],
    pathex=[ROOT],
    binaries=[],
    datas=certifi_datas,
    hiddenimports=[
        "httpx",
        "h2",
        "cryptography.hazmat.primitives.ciphers",
        "cryptography.hazmat.primitives.ciphers.algorithms",
        "cryptography.hazmat.primitives.ciphers.modes",
        "cryptography.hazmat.backends.openssl",
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    # GUI-only dependencies of the upstream tray application.
    excludes=["pystray", "customtkinter", "PIL", "pyperclip", "psutil", "tkinter"],
    noarchive=False,
)

pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name="tgws-cli-windows-x64",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    runtime_tmpdir=None,
    # A console build keeps stdout/stderr attached. zapret-ui spawns it with
    # CREATE_NO_WINDOW, so the user never sees a console window.
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
