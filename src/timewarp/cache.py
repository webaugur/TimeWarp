"""Remembered CLI settings (--city, --tz, and similar)."""

from __future__ import annotations

import json

from timewarp.errors import TimeWarpError
from timewarp.paths import config_file

# Flag name (without --) → cache key
CACHEABLE = {
    "tle": "tle",
    "city": "city",
    "lat": "lat",
    "lon": "lon",
    "tz": "tz",
    "color": "color",
    "no-color": "no_color",
    "holidays": "holidays",
    "weekend": "weekend",
    "country": "country",
}

CACHE_KEYS = tuple(dict.fromkeys(CACHEABLE.values()))


def cache_path() -> Path:
    return config_file("TIMEWARP_CACHE", "cache.json")


def load() -> dict:
    path = cache_path()
    if not path.is_file():
        return {}
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise TimeWarpError(f"could not read cache {path}: {exc}") from exc
    if not isinstance(data, dict):
        raise TimeWarpError(f"cache {path} is not a JSON object")
    return data


def save(data: dict) -> None:
    path = cache_path()
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    except OSError as exc:
        raise TimeWarpError(f"could not write cache {path}: {exc}") from exc


def clear(keys: list[str] | None = None) -> None:
    if not keys:
        path = cache_path()
        try:
            path.unlink(missing_ok=True)
        except OSError as exc:
            raise TimeWarpError(f"could not remove cache {path}: {exc}") from exc
        return
    data = load()
    for key in keys:
        data.pop(key, None)
        if key == "city":
            # location extras that only exist because of --city
            for extra in ("lat", "lon", "tz"):
                if extra not in keys:
                    data.pop(extra, None)
    if data:
        save(data)
    else:
        path = cache_path()
        try:
            path.unlink(missing_ok=True)
        except OSError as exc:
            raise TimeWarpError(f"could not remove cache {path}: {exc}") from exc


def flags_on_argv(argv: list[str]) -> set[str]:
    found: set[str] = set()
    for a in argv:
        if a.startswith("--"):
            name = a[2:]
            if name in CACHEABLE:
                found.add(CACHEABLE[name])
    return found


def quote_value(value: str) -> str:
    if any(c.isspace() for c in value) or not value:
        return f'"{value}"'
    return value


def format_pulled_cli(pulled: list[tuple[str, str | bool]], *, prog: str = "timewarp") -> str:
    parts = [prog] if prog else []
    for flag, value in pulled:
        if value is True:
            parts.append(flag)
        else:
            parts.append(f"{flag} {quote_value(str(value))}")
    return " ".join(parts)


def data_as_pulled(data: dict) -> list[tuple[str, str | bool]]:
    """Stable flag order for scripting."""
    order = (
        ("tle", "--tle"),
        ("city", "--city"),
        ("lat", "--lat"),
        ("lon", "--lon"),
        ("tz", "--tz"),
        ("color", "--color"),
        ("no_color", "--no-color"),
        ("holidays", "--holidays"),
        ("weekend", "--weekend"),
        ("country", "--country"),
    )
    pulled: list[tuple[str, str | bool]] = []
    for key, flag in order:
        if key not in data:
            continue
        val = data[key]
        if val is True:
            pulled.append((flag, True))
        elif val not in (None, False, ""):
            pulled.append((flag, val))
    return pulled
