"""Sunrise/sunset and moon phase. NOAA solar; synodic moon age."""

from __future__ import annotations

import math
from dataclasses import dataclass
from datetime import date, datetime, time, timedelta, timezone
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError

from timewarp.errors import TimeWarpError
from timewarp.iso import Instant, as_date, format_instant
from timewarp.places import Place

ZENITH_OFFICIAL = 90.833  # sunrise/sunset with refraction
SYNODIC_DAYS = 29.530588853
# New moon near 2000-01-06 18:14 UTC ( Meeus-style epoch )
NEW_MOON_JD = 2451550.1


@dataclass(frozen=True)
class SunTimes:
    date: date
    place: Place
    sunrise: datetime | None
    solar_noon: datetime | None
    sunset: datetime | None
    day_length_seconds: int | None
    note: str | None = None

    def to_dict(self) -> dict:
        return {
            "date": self.date.isoformat(),
            "place": self.place.name,
            "latitude": self.place.lat,
            "longitude": self.place.lon,
            "tz": self.place.tz,
            "sunrise": format_instant(self.sunrise) if self.sunrise else None,
            "solar_noon": format_instant(self.solar_noon) if self.solar_noon else None,
            "sunset": format_instant(self.sunset) if self.sunset else None,
            "day_length_iso8601": _seconds_iso(self.day_length_seconds),
            "note": self.note,
        }


@dataclass(frozen=True)
class MoonInfo:
    date: date
    phase: str
    illumination: float
    age_days: float
    next_new: date
    next_full: date

    def to_dict(self) -> dict:
        return {
            "date": self.date.isoformat(),
            "phase": self.phase,
            "illumination": round(self.illumination, 4),
            "age_days": round(self.age_days, 4),
            "next_new": self.next_new.isoformat(),
            "next_full": self.next_full.isoformat(),
        }


def _seconds_iso(seconds: int | None) -> str | None:
    if seconds is None:
        return None
    sign = "-" if seconds < 0 else ""
    s = abs(seconds)
    h, rem = divmod(s, 3600)
    m, sec = divmod(rem, 60)
    return f"{sign}PT{h}H{m}M{sec}S"


def _to_zone(name: str) -> ZoneInfo:
    try:
        return ZoneInfo(name)
    except ZoneInfoNotFoundError as exc:
        raise TimeWarpError(f"unknown time zone {name!r}; use an IANA name like America/New_York") from exc


def _sin_deg(x: float) -> float:
    return math.sin(math.radians(x))


def _cos_deg(x: float) -> float:
    return math.cos(math.radians(x))


def _tan_deg(x: float) -> float:
    return math.tan(math.radians(x))


def _wrap360(x: float) -> float:
    return x % 360.0


def _sun_utc_hours(d: date, lat: float, lon: float, rising: bool, zenith: float = ZENITH_OFFICIAL) -> float | None:
    """USNO/NOAA algorithm. Returns hours from 00:00 UTC, or None if no rise/set."""
    n = d.timetuple().tm_yday
    lng_hour = lon / 15.0
    t = n + ((6 - lng_hour) / 24.0 if rising else (18 - lng_hour) / 24.0)
    m_anom = (0.9856 * t) - 3.289
    l = _wrap360(m_anom + 1.916 * _sin_deg(m_anom) + 0.020 * _sin_deg(2 * m_anom) + 282.634)
    ra = math.degrees(math.atan(0.91764 * _tan_deg(l)))
    ra = _wrap360(ra)
    l_quad = math.floor(l / 90.0) * 90.0
    ra_quad = math.floor(ra / 90.0) * 90.0
    ra = (ra + (l_quad - ra_quad)) / 15.0
    sin_dec = 0.39782 * _sin_deg(l)
    cos_dec = math.cos(math.asin(sin_dec))
    cos_h = (_cos_deg(zenith) - (sin_dec * _sin_deg(lat))) / (cos_dec * _cos_deg(lat))
    if cos_h > 1:
        return None  # never rises
    if cos_h < -1:
        return None  # never sets
    h = (360.0 - math.degrees(math.acos(cos_h))) if rising else math.degrees(math.acos(cos_h))
    h /= 15.0
    t_local = h + ra - (0.06571 * t) - 6.622
    return (t_local - lng_hour) % 24.0


def _hours_to_datetime(d: date, hours: float, tz: ZoneInfo) -> datetime:
    utc = datetime.combine(d, time.min, tzinfo=timezone.utc) + timedelta(hours=hours)
    # rise/set near midnight can land on the previous/next UTC date; that's correct
    return utc.astimezone(tz).replace(microsecond=0)


def sun_times(d: Instant, place: Place) -> SunTimes:
    day = as_date(d)
    tz = _to_zone(place.tz)
    rise_h = _sun_utc_hours(day, place.lat, place.lon, True)
    set_h = _sun_utc_hours(day, place.lat, place.lon, False)
    note = None
    sunrise = sunset = noon = None
    length = None
    if rise_h is None and set_h is None:
        # polar day or night: distinguish by solar declination vs latitude
        n = day.timetuple().tm_yday
        decl = 23.44 * math.sin(math.radians((360.0 / 365.0) * (n - 81)))
        if (place.lat >= 0 and decl >= 0) or (place.lat < 0 and decl <= 0):
            if abs(place.lat) + abs(decl) >= 90.833:
                note = "Sun does not set"
            else:
                note = "Sun does not rise"
        else:
            note = "Sun does not rise"
    else:
        if rise_h is not None:
            sunrise = _hours_to_datetime(day, rise_h, tz)
        if set_h is not None:
            sunset = _hours_to_datetime(day, set_h, tz)
        if rise_h is not None and set_h is not None:
            if set_h < rise_h:
                set_h += 24.0
            noon_h = (rise_h + set_h) / 2.0
            noon = _hours_to_datetime(day, noon_h % 24.0, tz)
            length = int(round((set_h - rise_h) * 3600.0))
    return SunTimes(day, place, sunrise, noon, sunset, length, note)


def _julian_date(d: date) -> float:
    # Julian date at 12:00 UTC
    y = d.year
    m = d.month
    day = d.day + 0.5
    if m <= 2:
        y -= 1
        m += 12
    a = y // 100
    b = 2 - a + a // 4
    return int(365.25 * (y + 4716)) + int(30.6001 * (m + 1)) + day + b - 1524.5


def moon_age_days(d: date) -> float:
    jd = _julian_date(d)
    return (jd - NEW_MOON_JD) % SYNODIC_DAYS


def _phase_name(age: float) -> str:
    # 8 named phases from synodic age
    t = SYNODIC_DAYS
    slice_ = t / 8.0
    idx = int((age + slice_ / 2.0) // slice_) % 8
    return (
        "New Moon",
        "Waxing Crescent",
        "First Quarter",
        "Waxing Gibbous",
        "Full Moon",
        "Waning Gibbous",
        "Last Quarter",
        "Waning Crescent",
    )[idx]


def _illumination(age: float) -> float:
    # 0 at new, 1 at full
    return (1.0 - math.cos(2.0 * math.pi * age / SYNODIC_DAYS)) / 2.0


def _next_age(d: date, target_age: float) -> date:
    age = moon_age_days(d)
    wait = (target_age - age) % SYNODIC_DAYS
    # if we are essentially there today, still report today
    if wait < 0.5:
        return d
    return d + timedelta(days=int(round(wait)))


def moon_info(d: Instant) -> MoonInfo:
    day = as_date(d)
    age = moon_age_days(day)
    return MoonInfo(
        date=day,
        phase=_phase_name(age),
        illumination=_illumination(age),
        age_days=age,
        next_new=_next_age(day, 0.0),
        next_full=_next_age(day, SYNODIC_DAYS / 2.0),
    )
