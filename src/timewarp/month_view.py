"""One-month sun/moon/twilight sheet."""

from __future__ import annotations

import calendar
from dataclasses import dataclass
from datetime import date, timedelta

from timewarp.astro import moon_info, sun_times
from timewarp.errors import TimeWarpError
from timewarp.iso import MONTHS, format_clock, weekday_name
from timewarp.places import Place
from timewarp.rise import events_for_day


@dataclass(frozen=True)
class DaySheet:
    date: date
    weekday: str
    civil_dawn: object
    sunrise: object
    sunset: object
    civil_dusk: object
    nautical_dawn: object
    nautical_dusk: object
    astronomical_dawn: object
    astronomical_dusk: object
    day_length_seconds: int | None
    moonrise: object
    moonset: object
    illumination: float
    phase: str

    def to_dict(self) -> dict:
        def clk(v):
            return format_clock(v) if v else None

        return {
            "date": self.date.isoformat(),
            "weekday": self.weekday,
            "civil_dawn": clk(self.civil_dawn),
            "sunrise": clk(self.sunrise),
            "sunset": clk(self.sunset),
            "civil_dusk": clk(self.civil_dusk),
            "nautical_dawn": clk(self.nautical_dawn),
            "nautical_dusk": clk(self.nautical_dusk),
            "astronomical_dawn": clk(self.astronomical_dawn),
            "astronomical_dusk": clk(self.astronomical_dusk),
            "day_length_seconds": self.day_length_seconds,
            "moonrise": clk(self.moonrise),
            "moonset": clk(self.moonset),
            "illumination": round(self.illumination, 3),
            "phase": self.phase,
        }


def parse_year_month(text: str | None) -> tuple[int, int, bool]:
    """Return (year, month, assumed). YYYY-MM or YYYY-MM-DD; omit for this month."""
    if not text:
        today = date.today()
        return today.year, today.month, True
    raw = text.strip()
    if len(raw) >= 7 and raw[4] == "-":
        y = int(raw[0:4])
        mo = int(raw[5:7])
        if not 1 <= mo <= 12:
            raise TimeWarpError(f"month {raw!r} is not 01–12")
        if not 1 <= y <= 9999:
            raise TimeWarpError(f"year {y} is out of range 1..9999")
        return y, mo, False
    raise TimeWarpError(f"month {raw!r} is not YYYY-MM (ISO 8601)")


def month_days(year: int, month: int) -> list[date]:
    last = calendar.monthrange(year, month)[1]
    return [date(year, month, d) for d in range(1, last + 1)]


def _first(times) -> object:
    return times[0] if times else None


def sheet_for_month(year: int, month: int, place: Place) -> list[DaySheet]:
    rows = []
    for day in month_days(year, month):
        sun = sun_times(day, place)
        moon_ev = events_for_day("moon", day, place)
        info = moon_info(day)
        rows.append(
            DaySheet(
                date=day,
                weekday=weekday_name(day)[:2],
                civil_dawn=sun.civil_dawn,
                sunrise=sun.sunrise,
                sunset=sun.sunset,
                civil_dusk=sun.civil_dusk,
                nautical_dawn=sun.nautical_dawn,
                nautical_dusk=sun.nautical_dusk,
                astronomical_dawn=sun.astronomical_dawn,
                astronomical_dusk=sun.astronomical_dusk,
                day_length_seconds=sun.day_length_seconds,
                moonrise=_first(moon_ev.rises),
                moonset=_first(moon_ev.sets),
                illumination=info.illumination,
                phase=info.phase,
            )
        )
    return rows


def _clk(v) -> str:
    return format_clock(v) if v else "    —"


def _len_hm(seconds: int | None) -> str:
    if seconds is None:
        return "    —"
    h, rem = divmod(max(0, int(seconds)), 3600)
    m = rem // 60
    return f"{h:2d}:{m:02d}"


def format_month_sheet(rows: list[DaySheet], place: Place, *, twilight: bool = False) -> str:
    if not rows:
        return "No days.\n"
    y, m = rows[0].date.year, rows[0].date.month
    title = f"{place.name}  {y:04d}-{m:02d}  {MONTHS[m - 1]}  {place.tz}"
    if twilight:
        hdr = (
            f"{'date':10}  wd  {'adawn':8}  {'ndawn':8}  {'dawn':8}  {'rise':8}  "
            f"{'set':8}  {'dusk':8}  {'ndusk':8}  {'adusk':8}  {'len':5}  "
            f"{'moon↑':8}  {'moon↓':8}  ph"
        )
    else:
        hdr = (
            f"{'date':10}  wd  {'dawn':8}  {'rise':8}  {'set':8}  {'dusk':8}  "
            f"{'len':5}  {'moon↑':8}  {'moon↓':8}  ph"
        )
    lines = [title, hdr]
    for r in rows:
        illum = f"{r.illumination:3.0%}"
        if twilight:
            lines.append(
                f"{r.date.isoformat():10}  {r.weekday:2}  "
                f"{_clk(r.astronomical_dawn):8}  {_clk(r.nautical_dawn):8}  "
                f"{_clk(r.civil_dawn):8}  {_clk(r.sunrise):8}  {_clk(r.sunset):8}  "
                f"{_clk(r.civil_dusk):8}  {_clk(r.nautical_dusk):8}  {_clk(r.astronomical_dusk):8}  "
                f"{_len_hm(r.day_length_seconds):5}  {_clk(r.moonrise):8}  {_clk(r.moonset):8}  {illum}"
            )
        else:
            lines.append(
                f"{r.date.isoformat():10}  {r.weekday:2}  "
                f"{_clk(r.civil_dawn):8}  {_clk(r.sunrise):8}  {_clk(r.sunset):8}  "
                f"{_clk(r.civil_dusk):8}  {_len_hm(r.day_length_seconds):5}  "
                f"{_clk(r.moonrise):8}  {_clk(r.moonset):8}  {illum}"
            )
    lines.append("dawn/dusk are civil twilight (sun −6°). --twilight adds nautical and astronomical.")
    return "\n".join(lines) + "\n"
