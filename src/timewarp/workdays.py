"""Workdays (count) and Add Workdays. Weekends default Sat+Sun; optional US holidays."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date, timedelta

from timewarp.errors import TimeWarpError
from timewarp.holidays import holidays_in_range, parse_weekend
from timewarp.iso import Instant, as_date, weekday_name


@dataclass(frozen=True)
class WorkdaySpan:
    start: date
    end: date
    workdays: int
    calendar_days: int
    weekend_days: int
    holiday_days: int
    include_end: bool
    weekend: frozenset[int]
    holidays_used: tuple[str, ...]

    def iso(self) -> str:
        n = self.workdays
        if n < 0:
            return f"-P{abs(n)}D"
        return f"P{n}D"

    def to_dict(self) -> dict:
        return {
            "start": self.start.isoformat(),
            "end": self.end.isoformat(),
            "start_weekday": weekday_name(self.start),
            "end_weekday": weekday_name(self.end),
            "workdays": self.workdays,
            "calendar_days": self.calendar_days,
            "weekend_days": self.weekend_days,
            "holiday_days": self.holiday_days,
            "include_end": self.include_end,
            "iso8601": self.iso(),
        }


def is_workday(d: date, weekend: frozenset[int], holidays: set[date]) -> bool:
    return d.weekday() not in weekend and d not in holidays


def count_workdays(
    start: Instant,
    end: Instant,
    *,
    include_end: bool = False,
    weekend: frozenset[int] | None = None,
    holidays: set[date] | None = None,
    holiday_country: str | None = None,
    holiday_refresh: bool = False,
    holiday_region: str | None = None,
) -> WorkdaySpan:
    """Count workdays the same way Count Days counts calendar days: half-open, signed.

    Default: workdays in [start, end). --include-end extends end by one calendar day
    toward the same direction as (end - start).
    """
    a = as_date(start)
    b = as_date(end)
    weekend = weekend if weekend is not None else parse_weekend(None)
    names: tuple[str, ...] = ()
    if holidays is None:
        holidays = set()
        if holiday_country:
            holidays = holidays_in_range(
                a, b, holiday_country, refresh=holiday_refresh, region=holiday_region
            )
            names = (holiday_country,)
    else:
        names = ("custom",)

    adj = b
    if include_end:
        try:
            if b >= a:
                adj = b + timedelta(days=1)
            else:
                adj = b - timedelta(days=1)
        except OverflowError as exc:
            raise TimeWarpError("date overflow counting workdays") from exc

    calendar_days = (adj - a).days

    work = weekend_n = hols = 0
    if adj >= a:
        d = a
        while d < adj:
            if d in holidays:
                hols += 1
            elif d.weekday() in weekend:
                weekend_n += 1
            else:
                work += 1
            d += timedelta(days=1)
        signed = 1
    else:
        d = adj
        while d < a:
            if d in holidays:
                hols += 1
            elif d.weekday() in weekend:
                weekend_n += 1
            else:
                work += 1
            d += timedelta(days=1)
        signed = -1

    return WorkdaySpan(
        start=a,
        end=b,
        workdays=signed * work,
        calendar_days=calendar_days,
        weekend_days=signed * weekend_n,
        holiday_days=signed * hols,
        include_end=include_end,
        weekend=weekend,
        holidays_used=names,
    )


def add_workdays(
    start: Instant,
    n: int,
    *,
    weekend: frozenset[int] | None = None,
    holidays: set[date] | None = None,
    holiday_country: str | None = None,
    holiday_refresh: bool = False,
    holiday_region: str | None = None,
) -> date:
    d = as_date(start)
    weekend = weekend if weekend is not None else parse_weekend(None)
    if holidays is None:
        holidays = set()
    if n == 0:
        return d
    step = 1 if n > 0 else -1
    remaining = abs(n)
    loaded: set[int] = set()

    def ensure(year: int) -> None:
        if holiday_country and year not in loaded:
            holidays.update(
                holidays_in_range(
                    date(year, 1, 1),
                    date(year, 12, 31),
                    holiday_country,
                    refresh=holiday_refresh,
                    region=holiday_region,
                )
            )
            loaded.add(year)

    ensure(d.year)
    guard = 0
    while remaining:
        try:
            d += timedelta(days=step)
        except OverflowError as exc:
            raise TimeWarpError("date overflow while adding workdays") from exc
        ensure(d.year)
        guard += 1
        if guard > abs(n) * 10 + 400:
            raise TimeWarpError(
                "too many skipped days while adding workdays; check --weekend/--holidays"
            )
        if is_workday(d, weekend, holidays):
            remaining -= 1
    return d


def parse_workday_count(text: str) -> int:
    raw = text.strip()
    upper = raw.upper()
    if upper.startswith("P") or upper.startswith("-P"):
        from timewarp.duration import parse_offset

        off = parse_offset(raw)
        extra = (off.years, off.months, off.weeks, off.hours, off.minutes, off.seconds)
        if any(extra):
            raise ValueError("add-workdays offset must be a number of days (for example 10 or P10D)")
        return off.days
    try:
        return int(raw)
    except ValueError as exc:
        raise ValueError(f"invalid workday count {text!r}; use an integer or P10D") from exc
