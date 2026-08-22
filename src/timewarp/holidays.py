"""US federal holidays (observed). Country calendars beyond US are out of scope for this version."""

from __future__ import annotations

from datetime import date, timedelta

from timewarp.errors import TimeWarpError

WEEKDAY_INDEX = {
    "mon": 0,
    "monday": 0,
    "tue": 1,
    "tues": 1,
    "tuesday": 1,
    "wed": 2,
    "wednesday": 2,
    "thu": 3,
    "thur": 3,
    "thurs": 3,
    "thursday": 3,
    "fri": 4,
    "friday": 4,
    "sat": 5,
    "saturday": 5,
    "sun": 6,
    "sunday": 6,
}


def nth_weekday(year: int, month: int, weekday: int, n: int) -> date:
    """n is 1-based. weekday is Monday=0 (datetime.weekday)."""
    d = date(year, month, 1)
    shift = (weekday - d.weekday()) % 7
    return d + timedelta(days=shift + 7 * (n - 1))


def last_weekday(year: int, month: int, weekday: int) -> date:
    if month == 12:
        d = date(year + 1, 1, 1) - timedelta(days=1)
    else:
        d = date(year, month + 1, 1) - timedelta(days=1)
    shift = (d.weekday() - weekday) % 7
    return d - timedelta(days=shift)


def observed(d: date) -> date:
    """Saturday -> Friday before; Sunday -> Monday after."""
    if d.weekday() == 5:
        return d - timedelta(days=1)
    if d.weekday() == 6:
        return d + timedelta(days=1)
    return d


def us_federal_holidays(year: int) -> list[tuple[date, str]]:
    """Return observed federal holiday dates for a year (unique dates, chronological)."""
    raw: list[tuple[date, str]] = [
        (observed(date(year, 1, 1)), "New Year's Day"),
        (nth_weekday(year, 1, 0, 3), "Martin Luther King Jr. Day"),
        (nth_weekday(year, 2, 0, 3), "Presidents' Day"),
        (last_weekday(year, 5, 0), "Memorial Day"),
        (observed(date(year, 6, 19)), "Juneteenth National Independence Day"),
        (observed(date(year, 7, 4)), "Independence Day"),
        (nth_weekday(year, 9, 0, 1), "Labor Day"),
        (nth_weekday(year, 10, 0, 2), "Columbus Day"),
        (observed(date(year, 11, 11)), "Veterans Day"),
        (nth_weekday(year, 11, 3, 4), "Thanksgiving Day"),
        (observed(date(year, 12, 25)), "Christmas Day"),
    ]
    # If New Year's observed falls in previous year, drop it here; next year's Jan 1 Sunday
    # observes on Jan 2 of this year — already handled when year has Jan 1 Sunday.
    filtered = [(d, name) for d, name in raw if d.year == year]
    filtered.sort(key=lambda item: item[0])
    return filtered


def us_holiday_set(year: int) -> set[date]:
    return {d for d, _ in us_federal_holidays(year)}


def holidays_in_range(start: date, end: date, country: str = "US") -> set[date]:
    country_key = country.strip().upper()
    if country_key in {"US", "USA", "UNITED STATES"}:
        lo, hi = (start, end) if start <= end else (end, start)
        days: set[date] = set()
        for year in range(lo.year, hi.year + 1):
            for d, _ in us_federal_holidays(year):
                if lo <= d <= hi:
                    days.add(d)
        return days
    raise TimeWarpError(
        f"holiday calendar {country!r} is not available yet; use US or omit --holidays"
    )


def parse_weekend(spec: str | None) -> frozenset[int]:
    """Return datetime.weekday() indices treated as weekend. Default Saturday+Sunday."""
    if spec is None or spec.strip() == "":
        return frozenset({5, 6})
    days = set()
    for part in spec.replace(" ", "").split(","):
        if not part:
            continue
        key = part.lower()
        if key.isdigit():
            n = int(key)
            # ISO 1-7 Monday-Sunday
            if 1 <= n <= 7:
                days.add(n - 1 if n < 7 else 6)
                continue
        if key not in WEEKDAY_INDEX:
            raise TimeWarpError(
                f"unknown weekday {part!r} in --weekend; use Mon,Tue,... or ISO 1-7"
            )
        days.add(WEEKDAY_INDEX[key])
    if not days:
        return frozenset({5, 6})
    return frozenset(days)


def country_label(country: str | None) -> str:
    if not country:
        return "none"
    return country.strip().upper()
