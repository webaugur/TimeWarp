"""One-screen civil day: sun, moon, RC note, holiday, ISS.

Composes existing calculators. ISS TLE fetch is fail-soft unless `--tle` is set.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date, datetime
from pathlib import Path
from zoneinfo import ZoneInfo

from timewarp.astro import MoonInfo, SeasonEvent, SunTimes, moon_info, seasons_for_year, sun_times
from timewarp.cycle import daily_period, rosicrucian_stamp
from timewarp.eclipses import Eclipse, list_eclipses
from timewarp.errors import TimeWarpError
from timewarp.holidays import holidays_for_year
from timewarp.iso import Instant, format_clock, format_instant, weekday_name
from timewarp.passes import (
    DEFAULT_MIN_ELEV,
    Pass,
    fetch_tle,
    load_tle_file,
    passes_for_day,
    select_sats,
)
from timewarp.places import Place
from timewarp.rise import events_for_day, local_civil_date

MAX_PASSES = 6
_QUARTERS = (
    ("next_new", "New moon"),
    ("next_first_quarter", "First quarter"),
    ("next_full", "Full moon"),
    ("next_last_quarter", "Last quarter"),
)


def iso_week_date(d: date) -> str:
    iso = d.isocalendar()
    return f"{iso.year:04d}-W{iso.week:02d}-{iso.weekday}"


def holiday_country(holidays: str | None, country: str | None) -> str:
    raw = (holidays or country or "US").strip()
    return raw or "US"


def _iso(when: datetime | None) -> str | None:
    return format_instant(when) if when else None


def _local_date(when: datetime, tz: str) -> date:
    return when.astimezone(ZoneInfo(tz)).date()


@dataclass(frozen=True)
class TodayView:
    date: date
    weekday: str
    iso_week: str
    place: Place
    holiday: str | None
    holiday_error: str | None
    sun: SunTimes
    moon: MoonInfo
    moonrise: datetime | None
    moonset: datetime | None
    moon_event_name: str | None
    moon_event_time: datetime | None
    stamp: str
    daily: dict
    season: SeasonEvent | None
    eclipse: Eclipse | None
    passes: tuple[Pass, ...]
    passes_error: str | None

    def to_dict(self) -> dict:
        ev = None
        if self.moon_event_name and self.moon_event_time:
            ev = {"name": self.moon_event_name, "time": format_instant(self.moon_event_time)}
        ecl = None
        if self.eclipse is not None:
            from timewarp.eclipses import eclipse_to_dict

            ecl = eclipse_to_dict(self.eclipse)
        season = self.season.to_dict() if self.season is not None else None
        return {
            "date": self.date.isoformat(),
            "weekday": self.weekday,
            "iso_week": self.iso_week,
            "place": self.place.name,
            "latitude": self.place.lat,
            "longitude": self.place.lon,
            "tz": self.place.tz,
            "holiday": self.holiday,
            "holiday_error": self.holiday_error,
            "sun": {
                "civil_dawn": _iso(self.sun.civil_dawn),
                "sunrise": _iso(self.sun.sunrise),
                "solar_noon": _iso(self.sun.solar_noon),
                "sunset": _iso(self.sun.sunset),
                "civil_dusk": _iso(self.sun.civil_dusk),
                "day_length_iso8601": self.sun.to_dict()["day_length_iso8601"],
                "note": self.sun.note,
            },
            "moon": {
                "phase": self.moon.phase,
                "illumination": round(self.moon.illumination, 4),
                "rise": _iso(self.moonrise),
                "set": _iso(self.moonset),
                "event": ev,
            },
            "rc": {
                "stamp": self.stamp,
                "letter": self.daily["letter"],
                "color": self.daily["color"],
                "color_hex": self.daily["color_hex"],
                "period": self.daily["period"],
                "time": self.daily["time"],
                "weekday": self.daily["weekday"],
                "planet": self.daily["planet"],
            },
            "season": season,
            "eclipse": ecl,
            "passes": [p.to_dict() for p in self.passes],
            "passes_error": self.passes_error,
        }


def _holiday_name(civil: date, country: str, region: str | None) -> tuple[str | None, str | None]:
    try:
        rows, _note = holidays_for_year(civil.year, country, region=region)
    except TimeWarpError as exc:
        return None, str(exc)
    for d, name in rows:
        if d == civil:
            return name, None
    return None, None


def _moon_event(info: MoonInfo, civil: date, tz: str) -> tuple[str | None, datetime | None]:
    for attr, label in _QUARTERS:
        when = getattr(info, attr)
        if isinstance(when, datetime) and _local_date(when, tz) == civil:
            return label, when
    return None, None


def _season_today(civil: date, tz: str) -> SeasonEvent | None:
    try:
        rows = seasons_for_year(civil.year)
    except TimeWarpError:
        return None
    for ev in rows:
        if _local_date(ev.time, tz) == civil:
            return ev
    return None


def _eclipse_today(civil: date) -> Eclipse | None:
    if civil.year < 1900 or civil.year > 2199:
        return None
    for ev in list_eclipses(year=civil.year):
        if ev.date == civil or ev.end_date == civil:
            return ev
    return None


def _iss_passes(
    civil: date,
    place: Place,
    *,
    tle_path: Path | None,
    min_elev: float,
) -> tuple[tuple[Pass, ...], str | None]:
    try:
        if tle_path is not None:
            sats = load_tle_file(tle_path)
        else:
            sats = fetch_tle("ISS")
        picked = select_sats(sats, "ISS", all_sats=False)
        rows: list[Pass] = []
        for sat in picked:
            rows.extend(passes_for_day(sat, civil, place, min_elev=min_elev))
        rows.sort(key=lambda p: (p.tca, p.sat.name))
        return tuple(rows[:MAX_PASSES]), None
    except TimeWarpError as exc:
        if tle_path is not None:
            raise
        return (), str(exc)


def snapshot(
    when: Instant,
    place: Place,
    *,
    country: str = "US",
    region: str | None = None,
    tle_path: Path | None = None,
    min_elev: float = DEFAULT_MIN_ELEV,
) -> TodayView:
    """Build the dashboard for the local civil day containing `when`."""
    civil = local_civil_date(when, place)
    sun = sun_times(civil, place)
    moon = moon_info(civil)
    moon_ev = events_for_day("moon", civil, place)
    moonrise = moon_ev.rises[0] if moon_ev.rises else None
    moonset = moon_ev.sets[0] if moon_ev.sets else None
    event_name, event_time = _moon_event(moon, civil, place.tz)
    stamp = rosicrucian_stamp(when, place)
    daily = daily_period(when, place)
    holiday, holiday_error = _holiday_name(civil, country, region)
    passes, passes_error = _iss_passes(civil, place, tle_path=tle_path, min_elev=min_elev)
    return TodayView(
        date=civil,
        weekday=weekday_name(civil),
        iso_week=iso_week_date(civil),
        place=place,
        holiday=holiday,
        holiday_error=holiday_error,
        sun=sun,
        moon=moon,
        moonrise=moonrise,
        moonset=moonset,
        moon_event_name=event_name,
        moon_event_time=event_time,
        stamp=stamp.stamp(),
        daily=daily,
        season=_season_today(civil, place.tz),
        eclipse=_eclipse_today(civil),
        passes=passes,
        passes_error=passes_error,
    )


def format_quiet(view: TodayView) -> str:
    rise = format_clock(view.sun.sunrise) if view.sun.sunrise else "none"
    sset = format_clock(view.sun.sunset) if view.sun.sunset else "none"
    return (
        f"{view.date.isoformat()} {view.weekday}  {rise}–{sset}  "
        f"{view.stamp}  {view.daily['letter']}"
    )
