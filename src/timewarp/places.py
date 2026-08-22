"""Named places for sun/moon. Coordinates are WGS84; time zones are IANA names."""

from __future__ import annotations

from dataclasses import dataclass

from timewarp.errors import TimeWarpError


@dataclass(frozen=True)
class Place:
    name: str
    lat: float
    lon: float
    tz: str


# A short list so `sun`/`moon` work without looking up coordinates.
PLACES: dict[str, Place] = {}


def _add(name: str, lat: float, lon: float, tz: str) -> None:
    place = Place(name, lat, lon, tz)
    PLACES[name.lower()] = place
    slug = name.lower().replace(",", " ").replace("  ", " ")
    PLACES[slug] = place


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


def lookup_place(name: str) -> Place:
    key = " ".join(name.strip().lower().replace("_", " ").split())
    if key in PLACES:
        return PLACES[key]
    known = sorted({p.name for p in PLACES.values()})
    raise TimeWarpError(
        f"unknown city {name!r}. Pass --lat/--lon/--tz, or one of: {', '.join(known)}"
    )
