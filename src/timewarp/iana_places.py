"""Places derived from IANA tzdata (zone1970.tab), which zoneinfo uses.

Python's locale module has no city or coordinate list — it only does language
and number formats. The timezone database *does* ship coordinates for
representative cities; we read those here.
"""

from __future__ import annotations

import re
from pathlib import Path
from zoneinfo import TZPATH

from timewarp.paths import tzdata_zoneinfo_dir

# ISO 6709 as used by zone.tab: ±DDMM[SS]±DDDMM[SS]
_COORDS = re.compile(
    r"^([+-])(\d{2})(\d{2})(\d{2})?([+-])(\d{3})(\d{2})(\d{2})?$"
)


def _iso6709(token: str) -> tuple[float, float]:
    m = _COORDS.fullmatch(token)
    if not m:
        raise ValueError(f"unrecognized zone.tab coordinate {token!r}")
    lat_sign = 1 if m.group(1) == "+" else -1
    lat = int(m.group(2)) + int(m.group(3)) / 60.0
    if m.group(4):
        lat += int(m.group(4)) / 3600.0
    lon_sign = 1 if m.group(5) == "+" else -1
    lon = int(m.group(6)) + int(m.group(7)) / 60.0
    if m.group(8):
        lon += int(m.group(8)) / 3600.0
    return lat_sign * lat, lon_sign * lon


def _city_from_zone(zone: str) -> str:
    last = zone.split("/")[-1].replace("_", " ")
    special = {
        "St Johns": "St. John's",
        "Mexico City": "Mexico City",
        "Los Angeles": "Los Angeles",
        "New York": "New York",
        "Sao Paulo": "Sao Paulo",
    }
    return special.get(last, last)


def zone1970_tab() -> Path | None:
    roots = [Path(p) for p in TZPATH]
    extra = tzdata_zoneinfo_dir()
    if extra is not None and extra not in roots:
        roots.append(extra)
    for root in roots:
        path = root / "zone1970.tab"
        if path.is_file():
            return path
    return None


def iter_na_tz_places() -> list[tuple[str, float, float, str]]:
    """Return (name, lat, lon, tz) for US/CA/MX timezone representative cities."""
    path = zone1970_tab()
    if path is None:
        return []
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) < 3:
            continue
        countries = parts[0].split(",")
        if not any(c in {"US", "CA", "MX"} for c in countries):
            continue
        coords, zone = parts[1], parts[2]
        try:
            lat, lon = _iso6709(coords)
        except ValueError:
            continue
        name = _city_from_zone(zone)
        rows.append((name, round(lat, 6), round(lon, 6), zone))
    return rows
