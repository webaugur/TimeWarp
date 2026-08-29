"""Sunrise/sunset, twilight, seasons, and moon phase. NOAA solar; Schlyter moon."""

from __future__ import annotations

import math
from dataclasses import dataclass
from datetime import date, datetime, time, timedelta, timezone
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError

from timewarp.errors import TimeWarpError
from timewarp.iso import Instant, as_date, format_instant
from timewarp.places import Place

ZENITH_OFFICIAL = 90.833  # sunrise/sunset with refraction
ZENITH_CIVIL = 96.0  # sun altitude −6°
ZENITH_NAUTICAL = 102.0
ZENITH_ASTRONOMICAL = 108.0
SYNODIC_DAYS = 29.530588853
# New moon near 2000-01-06 18:14 UTC ( Meeus-style epoch )
NEW_MOON_JD = 2451550.1


def _iso_or_none(value: datetime | None) -> str | None:
    return format_instant(value) if value else None


@dataclass(frozen=True)
class SunTimes:
    date: date
    place: Place
    sunrise: datetime | None
    solar_noon: datetime | None
    sunset: datetime | None
    day_length_seconds: int | None
    note: str | None = None
    sunrise_az: float | None = None
    sunset_az: float | None = None
    civil_dawn: datetime | None = None
    civil_dusk: datetime | None = None
    nautical_dawn: datetime | None = None
    nautical_dusk: datetime | None = None
    astronomical_dawn: datetime | None = None
    astronomical_dusk: datetime | None = None

    def to_dict(self) -> dict:
        return {
            "date": self.date.isoformat(),
            "place": self.place.name,
            "latitude": self.place.lat,
            "longitude": self.place.lon,
            "tz": self.place.tz,
            "sunrise": _iso_or_none(self.sunrise),
            "sunrise_azimuth_deg": None if self.sunrise_az is None else round(self.sunrise_az, 1),
            "solar_noon": _iso_or_none(self.solar_noon),
            "sunset": _iso_or_none(self.sunset),
            "sunset_azimuth_deg": None if self.sunset_az is None else round(self.sunset_az, 1),
            "civil_dawn": _iso_or_none(self.civil_dawn),
            "civil_dusk": _iso_or_none(self.civil_dusk),
            "nautical_dawn": _iso_or_none(self.nautical_dawn),
            "nautical_dusk": _iso_or_none(self.nautical_dusk),
            "astronomical_dawn": _iso_or_none(self.astronomical_dawn),
            "astronomical_dusk": _iso_or_none(self.astronomical_dusk),
            "day_length_iso8601": _seconds_iso(self.day_length_seconds),
            "note": self.note,
        }


@dataclass(frozen=True)
class MoonInfo:
    date: date
    phase: str
    illumination: float
    age_days: float
    next_new: datetime
    next_full: datetime
    next_first_quarter: datetime
    next_last_quarter: datetime

    def to_dict(self) -> dict:
        return {
            "date": self.date.isoformat(),
            "phase": self.phase,
            "illumination": round(self.illumination, 4),
            "age_days": round(self.age_days, 4),
            "next_new": format_instant(self.next_new),
            "next_full": format_instant(self.next_full),
            "next_first_quarter": format_instant(self.next_first_quarter),
            "next_last_quarter": format_instant(self.next_last_quarter),
        }


@dataclass(frozen=True)
class SeasonEvent:
    name: str
    time: datetime  # UTC

    def to_dict(self) -> dict:
        return {"name": self.name, "time": format_instant(self.time)}


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
    local = utc.astimezone(tz).replace(microsecond=0)
    # NOAA hours are measured from 00:00 UTC of calendar date d. Convert to that
    # local civil day so an evening event is not shown on the previous evening.
    if local.date() < d:
        local += timedelta(days=1)
    elif local.date() > d:
        local -= timedelta(days=1)
    return local


def _twilight_pair(day: date, place: Place, tz: ZoneInfo, zenith: float) -> tuple[datetime | None, datetime | None]:
    dawn_h = _sun_utc_hours(day, place.lat, place.lon, True, zenith)
    dusk_h = _sun_utc_hours(day, place.lat, place.lon, False, zenith)
    dawn = _hours_to_datetime(day, dawn_h, tz) if dawn_h is not None else None
    dusk = _hours_to_datetime(day, dusk_h, tz) if dusk_h is not None else None
    return dawn, dusk


def _sun_azimuth(when: datetime | None, place: Place) -> float | None:
    if when is None:
        return None
    from timewarp.ephem import altitude_azimuth, position

    pos = position("sun", when)
    _alt, az = altitude_azimuth(pos, when, place.lat, place.lon)
    return az


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
    civil_dawn, civil_dusk = _twilight_pair(day, place, tz, ZENITH_CIVIL)
    nautical_dawn, nautical_dusk = _twilight_pair(day, place, tz, ZENITH_NAUTICAL)
    astro_dawn, astro_dusk = _twilight_pair(day, place, tz, ZENITH_ASTRONOMICAL)
    return SunTimes(
        day,
        place,
        sunrise,
        noon,
        sunset,
        length,
        note,
        sunrise_az=_sun_azimuth(sunrise, place),
        sunset_az=_sun_azimuth(sunset, place),
        civil_dawn=civil_dawn,
        civil_dusk=civil_dusk,
        nautical_dawn=nautical_dawn,
        nautical_dusk=nautical_dusk,
        astronomical_dawn=astro_dawn,
        astronomical_dusk=astro_dusk,
    )


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


def _utc(dt: datetime) -> datetime:
    if dt.tzinfo is None:
        return dt.replace(tzinfo=timezone.utc)
    return dt.astimezone(timezone.utc)


def _ang_diff(value: float, target: float) -> float:
    return (value - target + 180.0) % 360.0 - 180.0


def _next_longitude_crossing(
    start: datetime,
    metric,
    target: float,
    *,
    step_hours: float,
    max_days: float,
) -> datetime:
    """Next time metric() increases through target degrees (0–360)."""
    t0 = _utc(start)
    end = t0 + timedelta(days=max_days)
    step = timedelta(hours=step_hours)
    prev_t = t0
    prev = _ang_diff(metric(t0), target)
    t = t0 + step
    guard = 0
    while t <= end:
        cur = _ang_diff(metric(t), target)
        if prev <= 0 <= cur and not (prev == 0 and cur == 0):
            lo, hi = prev_t, t
            for _ in range(40):
                if (hi - lo).total_seconds() < 0.5:
                    break
                mid = lo + (hi - lo) / 2
                amid = _ang_diff(metric(mid), target)
                if amid <= 0:
                    lo = mid
                else:
                    hi = mid
            found = lo + (hi - lo) / 2
            return found.replace(microsecond=0)
        prev, prev_t = cur, t
        t += step
        guard += 1
        if guard > 4000:
            raise TimeWarpError("longitude search exceeded its bound (internal error)")
    raise TimeWarpError(f"no longitude crossing of {target}° within {max_days:.0f} days of {t0.isoformat()}")


def _moon_rel_lon(dt: datetime) -> float:
    from timewarp.ephem import day_number, position, sun_state

    pos = position("moon", dt)
    sun = sun_state(day_number(dt))
    return (pos.ecl_lon - sun.lon) % 360.0


def _sun_ecl_lon(dt: datetime) -> float:
    from timewarp.ephem import day_number, sun_state

    return sun_state(day_number(dt)).lon


def moon_info(d: Instant) -> MoonInfo:
    day = as_date(d)
    age = moon_age_days(day)
    if isinstance(d, datetime):
        start = _utc(d)
    else:
        start = datetime(day.year, day.month, day.day, tzinfo=timezone.utc)
    return MoonInfo(
        date=day,
        phase=_phase_name(age),
        illumination=_illumination(age),
        age_days=age,
        next_new=_next_longitude_crossing(start, _moon_rel_lon, 0.0, step_hours=6, max_days=40),
        next_first_quarter=_next_longitude_crossing(start, _moon_rel_lon, 90.0, step_hours=6, max_days=40),
        next_full=_next_longitude_crossing(start, _moon_rel_lon, 180.0, step_hours=6, max_days=40),
        next_last_quarter=_next_longitude_crossing(start, _moon_rel_lon, 270.0, step_hours=6, max_days=40),
    )


_SEASONS = (
    (0.0, "March equinox"),
    (90.0, "June solstice"),
    (180.0, "September equinox"),
    (270.0, "December solstice"),
)


def seasons_for_year(year: int) -> list[SeasonEvent]:
    if not 1 <= year <= 9999:
        raise TimeWarpError(f"year {year} is out of range 1..9999")
    # Search each event from a month before its usual window.
    starts = (
        datetime(year, 2, 1, tzinfo=timezone.utc),
        datetime(year, 5, 1, tzinfo=timezone.utc),
        datetime(year, 8, 1, tzinfo=timezone.utc),
        datetime(year, 11, 1, tzinfo=timezone.utc),
    )
    rows = []
    for start, (target, name) in zip(starts, _SEASONS, strict=True):
        when = _next_longitude_crossing(start, _sun_ecl_lon, target, step_hours=12, max_days=80)
        if when.year != year:
            raise TimeWarpError(f"{name} for {year} landed on {when.date().isoformat()}")
        rows.append(SeasonEvent(name, when))
    return rows
