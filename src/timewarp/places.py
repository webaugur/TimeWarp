"""Named places for sun/moon. Coordinates are WGS84; time zones are IANA names.

Python's locale module does not list cities. Coordinates come from:
- a built-in world list
- IANA tzdata zone1970.tab (via zoneinfo)
- US / Canadian / Mexican capitals
"""

from __future__ import annotations

from dataclasses import dataclass

from timewarp.errors import TimeWarpError
from timewarp.iana_places import iter_na_tz_places
from timewarp.na_capitals import ALIASES, all_capitals


@dataclass(frozen=True)
class Place:
    name: str
    lat: float
    lon: float
    tz: str


PLACES: dict[str, Place] = {}
_CANONICAL: dict[str, str] = {}


def _slug(name: str) -> str:
    return " ".join(name.strip().lower().replace("_", " ").replace(",", " ").split())


def _add(name: str, lat: float, lon: float, tz: str, *aliases: str) -> None:
    place = Place(name, lat, lon, tz)
    keys = [_slug(name), *[_slug(a) for a in aliases]]
    for key in keys:
        PLACES[key] = place
        _CANONICAL[key] = name


_add("UTC", 0.0, 0.0, "UTC")
_add("New York", 40.7128, -74.0060, "America/New_York")
_add("Los Angeles", 34.0522, -118.2437, "America/Los_Angeles")
_add("Chicago", 41.8781, -87.6298, "America/Chicago")
_add("Denver", 39.7392, -104.9903, "America/Denver")
_add("Phoenix", 33.4484, -112.0740, "America/Phoenix")
_add("Seattle", 47.6062, -122.3321, "America/Los_Angeles")
_add("Miami", 25.7617, -80.1918, "America/New_York")
_add("Honolulu", 21.3069, -157.8583, "Pacific/Honolulu")
_add("Anchorage", 61.2181, -149.9003, "America/Anchorage")
_add("London", 51.5074, -0.1278, "Europe/London")
_add("Greenwich", 51.4779, -0.0015, "Europe/London")
_add("Paris", 48.8566, 2.3522, "Europe/Paris")
_add("Berlin", 52.5200, 13.4050, "Europe/Berlin")
_add("Madrid", 40.4168, -3.7038, "Europe/Madrid")
_add("Rome", 41.9028, 12.4964, "Europe/Rome")
_add("Cairo", 30.0444, 31.2357, "Africa/Cairo")
_add("Johannesburg", -26.2041, 28.0473, "Africa/Johannesburg")
_add("Dubai", 25.2048, 55.2708, "Asia/Dubai")
_add("Mumbai", 19.0760, 72.8777, "Asia/Kolkata")
_add("Singapore", 1.3521, 103.8198, "Asia/Singapore")
_add("Hong Kong", 22.3193, 114.1694, "Asia/Hong_Kong")
_add("Tokyo", 35.6762, 139.6503, "Asia/Tokyo")
_add("Sydney", -33.8688, 151.2093, "Australia/Sydney")
_add("Auckland", -36.8509, 174.7645, "Pacific/Auckland")
_add("Sao Paulo", -23.5505, -46.6333, "America/Sao_Paulo")
_add("Mexico City", 19.4326, -99.1332, "America/Mexico_City")
_add("Toronto", 43.6532, -79.3832, "America/Toronto")
_add("Vancouver", 49.2827, -123.1207, "America/Vancouver")

for name, lat, lon, tz in iter_na_tz_places():
    _add(name, lat, lon, tz)

for name, lat, lon, tz in all_capitals():
    _add(name, lat, lon, tz)

for alias, canonical in ALIASES.items():
    place = PLACES.get(_slug(canonical))
    if place is not None:
        PLACES[_slug(alias)] = place
        _CANONICAL[_slug(alias)] = place.name


def place_names() -> list[str]:
    return sorted({p.name for p in PLACES.values()}, key=str.casefold)


def lookup_place(name: str) -> Place:
    key = _slug(name)
    if key in PLACES:
        return PLACES[key]
    names = place_names()
    sample = ", ".join(names[:8])
    raise TimeWarpError(
        f"unknown city {name!r}. Pass --lat/--lon/--tz, or a named place "
        f"({len(names)} loaded, e.g. {sample}). List them with: timewarp cities"
    )
