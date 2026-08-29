"""Config/cache directories and tzdata for frozen (PyInstaller) builds.

Unix: XDG (~/.config, ~/.cache). Windows: APPDATA / LOCALAPPDATA.
TIMEWARP_* env vars still override a specific file or tree.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path


def is_frozen() -> bool:
    return bool(getattr(sys, "frozen", False))


def bundle_dir() -> Path | None:
    meipass = getattr(sys, "_MEIPASS", None)
    if meipass:
        return Path(meipass)
    return None


def user_config_dir() -> Path:
    env = os.environ.get("XDG_CONFIG_HOME")
    if env:
        return Path(env)
    if os.name == "nt":
        appdata = os.environ.get("APPDATA")
        if appdata:
            return Path(appdata)
        return Path.home() / "AppData" / "Roaming"
    return Path.home() / ".config"


def user_cache_dir() -> Path:
    env = os.environ.get("XDG_CACHE_HOME")
    if env:
        return Path(env)
    if os.name == "nt":
        local = os.environ.get("LOCALAPPDATA")
        if local:
            return Path(local)
        return Path.home() / "AppData" / "Local"
    return Path.home() / ".cache"


def config_file(env_name: str, name: str) -> Path:
    env = os.environ.get(env_name)
    if env:
        return Path(env)
    return user_config_dir() / "timewarp" / name


def cache_subdir(env_name: str, name: str) -> Path:
    env = os.environ.get(env_name)
    if env:
        return Path(env)
    return user_cache_dir() / "timewarp" / name


def tzdata_zoneinfo_dir() -> Path | None:
    """Directory that contains zone1970.tab (tzdata package or a frozen copy)."""
    try:
        import tzdata
    except ImportError:
        tzdata = None
    if tzdata is not None:
        path = Path(tzdata.__file__).resolve().parent / "zoneinfo"
        if (path / "zone1970.tab").is_file():
            return path
    root = bundle_dir()
    if root is not None:
        for cand in (root / "tzdata" / "zoneinfo", root / "zoneinfo"):
            if (cand / "zone1970.tab").is_file():
                return cand
    return None


def ensure_zoneinfo() -> None:
    """Make ZoneInfo and zone1970.tab work when the OS has no IANA files."""
    extra = tzdata_zoneinfo_dir()
    if extra is None:
        return
    import zoneinfo

    extra_s = str(extra)
    current = [str(p) for p in zoneinfo.TZPATH]
    resolved = {str(Path(p).resolve()) for p in current}
    if str(extra.resolve()) in resolved:
        return
    if os.name == "nt" or is_frozen():
        zoneinfo.reset_tzpath([extra_s, *current])
    else:
        zoneinfo.reset_tzpath([*current, extra_s])
