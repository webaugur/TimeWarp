"""Calendar add/subtract and signed duration (Count Days / Add Days)."""

from __future__ import annotations

import calendar
import re
from dataclasses import dataclass
from datetime import date, datetime, timedelta
from typing import Iterable

from timewarp.errors import TimeWarpError
from timewarp.iso import Instant, as_date, format_instant, same_kind, weekday_name

_UNIT = re.compile(
    r"""
    (?P<sign>[+-])?
    (?P<n>\d+)
    (?:
        \s*
        (?P<unit>
            years?|yrs?|y|
            months?|mons?|mo|
            weeks?|wks?|w|
            days?|d|
            hours?|hrs?|h|
            minutes?|mins?|min|
            seconds?|secs?|s
        )
    )
    \b
    """,
    re.IGNORECASE | re.VERBOSE,
)

# Compact years-months[-days] when it cannot be an ISO date (year is 1–3 digits).
_HYPHEN_DUR = re.compile(
    r"^(?P<sign>-)?(?P<years>\d{1,3})-(?P<months>\d{1,2})(?:-(?P<days>\d{1,2}))?$"
)

_ISO_DUR = re.compile(
    r"""^
    (?P<sign>-)?
    P
    (?:(?P<years>\d+)Y)?
    (?:(?P<months>\d+)M)?
    (?:(?P<weeks>\d+)W)?
    (?:(?P<days>\d+)D)?
    (?:T
        (?:(?P<hours>\d+)H)?
        (?:(?P<minutes>\d+)M)?
        (?:(?P<seconds>\d+)S)?
    )?
    $""",
    re.IGNORECASE | re.VERBOSE,
)


class OffsetError(TimeWarpError):
    pass


@dataclass(frozen=True)
class Offset:
    years: int = 0
    months: int = 0
    weeks: int = 0
    days: int = 0
    hours: int = 0
    minutes: int = 0
    seconds: int = 0

    def negated(self) -> Offset:
        return Offset(
            years=-self.years,
            months=-self.months,
            weeks=-self.weeks,
            days=-self.days,
            hours=-self.hours,
            minutes=-self.minutes,
            seconds=-self.seconds,
        )

    def is_zero(self) -> bool:
        return not any(
            (self.years, self.months, self.weeks, self.days, self.hours, self.minutes, self.seconds)
        )

    def has_time(self) -> bool:
        return bool(self.hours or self.minutes or self.seconds)

    def human(self) -> str:
        parts = []
        for n, word in (
            (self.years, "year"),
            (self.months, "month"),
            (self.weeks, "week"),
            (self.days, "day"),
            (self.hours, "hour"),
            (self.minutes, "minute"),
            (self.seconds, "second"),
        ):
            if n:
                parts.append(_unit_phrase(n, word))
        return ", ".join(parts) if parts else "0 days"


@dataclass(frozen=True)
class CalendarSpan:
    """Signed calendar breakdown. Component signs match total_days (or total_seconds if times)."""

    years: int
    months: int
    days: int
    hours: int
    minutes: int
    seconds: int
    total_days: int
    total_seconds: int
    start: Instant
    end: Instant
    include_end: bool = False

    @property
    def sign(self) -> int:
        if self.total_seconds < 0:
            return -1
        if self.total_seconds > 0:
            return 1
        if self.total_days < 0:
            return -1
        if self.total_days > 0:
            return 1
        return 0

    def iso(self) -> str:
        if self.sign == 0:
            return "P0D"
        s = -1 if self.sign < 0 else 1
        y, mo, d = s * self.years, s * self.months, s * self.days
        h, mi, se = s * self.hours, s * self.minutes, s * self.seconds
        prefix = "-" if self.sign < 0 else ""
        date_part = ""
        if y:
            date_part += f"{y}Y"
        if mo:
            date_part += f"{mo}M"
        if d:
            date_part += f"{d}D"
        time_part = ""
        if h:
            time_part += f"{h}H"
        if mi:
            time_part += f"{mi}M"
        if se:
            time_part += f"{se}S"
        if not date_part and not time_part:
            return "P0D"
        if time_part:
            return f"{prefix}P{date_part}T{time_part}"
        return f"{prefix}P{date_part}"

    def weeks_and_days(self) -> tuple[int, int]:
        weeks, rem = divmod(abs(self.total_days), 7)
        if self.sign < 0:
            return -weeks, -rem
        return weeks, rem

    def human(self) -> str:
        if self.sign == 0:
            return "0 days"
        s = -1 if self.sign < 0 else 1
        parts = []
        for n, word in (
            (s * self.years, "year"),
            (s * self.months, "month"),
            (s * self.days, "day"),
            (s * self.hours, "hour"),
            (s * self.minutes, "minute"),
            (s * self.seconds, "second"),
        ):
            if n:
                parts.append(_unit_phrase(n, word))
        if not parts:
            parts.append("0 days")
        body = ", ".join(parts)
        if self.sign < 0:
            return f"-({body})"
        return body

    def to_dict(self) -> dict:
        weeks, week_days = self.weeks_and_days()
        return {
            "start": format_instant(self.start),
            "end": format_instant(self.end),
            "start_weekday": weekday_name(as_date(self.start)),
            "end_weekday": weekday_name(as_date(self.end)),
            "include_end": self.include_end,
            "sign": self.sign,
            "years": self.years,
            "months": self.months,
            "days": self.days,
            "hours": self.hours,
            "minutes": self.minutes,
            "seconds": self.seconds,
            "total_days": self.total_days,
            "total_seconds": self.total_seconds,
            "weeks": weeks,
            "week_days": week_days,
            "iso8601": self.iso(),
            "human": self.human(),
        }


def _unit_phrase(n: int, word: str) -> str:
    label = word if abs(n) == 1 else word + "s"
    return f"{n} {label}"


def _norm_unit(unit: str) -> str:
    u = unit.lower()
    if u in {"y", "yr", "yrs", "year", "years"}:
        return "years"
    if u in {"mo", "mon", "mons", "month", "months"}:
        return "months"
    if u in {"w", "wk", "wks", "week", "weeks"}:
        return "weeks"
    if u in {"d", "day", "days"}:
        return "days"
    if u in {"h", "hr", "hrs", "hour", "hours"}:
        return "hours"
    if u in {"min", "mins", "minute", "minutes"}:
        return "minutes"
    if u in {"s", "sec", "secs", "second", "seconds"}:
        return "seconds"
    raise OffsetError(
        f"unknown unit {unit!r}; use years, months, weeks, days, hours, minutes, seconds "
        "(abbreviations: y, mo, w, d, h, min, s — not bare 'm')"
    )


def parse_offset(tokens: Iterable[str] | str) -> Offset:
    if isinstance(tokens, str):
        text = tokens.strip()
    else:
        text = " ".join(str(t) for t in tokens).strip()
    if not text:
        raise OffsetError("missing offset; example: 7 months 6 days or P7M6D")

    compact = text.replace(" ", "")
    iso = _ISO_DUR.fullmatch(compact)
    if iso and "P" in compact.upper():
        sign = -1 if iso.group("sign") else 1
        def g(name: str) -> int:
            v = iso.group(name)
            return sign * int(v) if v else 0
        off = Offset(
            years=g("years"),
            months=g("months"),
            weeks=g("weeks"),
            days=g("days"),
            hours=g("hours"),
            minutes=g("minutes"),
            seconds=g("seconds"),
        )
        if off.is_zero() and compact.upper() not in {"P", "P0D", "-P0D"}:
            # P with no fields is invalid
            if re.fullmatch(r"-?P(?:T)?", compact, re.IGNORECASE):
                raise OffsetError(f"invalid ISO 8601 duration {text!r}")
        return off

    hyphen = _HYPHEN_DUR.fullmatch(compact)
    if hyphen:
        sign = -1 if hyphen.group("sign") else 1
        days = hyphen.group("days")
        return Offset(
            years=sign * int(hyphen.group("years")),
            months=sign * int(hyphen.group("months")),
            days=sign * int(days) if days else 0,
        )

    # Human: "7 months 6 days" or "7mo" "+3d" "-2 hours"
    joined = text
    matches = list(_UNIT.finditer(joined))
    if not matches:
        raise OffsetError(
            f"could not parse offset {text!r}; use '7 months 6 days', '7-6-13', or ISO 8601 'P7M6D'"
        )
    covered = "".join(joined[m.start() : m.end()] for m in matches)
    leftover = _UNIT.sub(" ", joined)
    leftover = re.sub(r"[\s,+/]+", "", leftover)
    if leftover:
        raise OffsetError(f"unrecognized offset text {text!r} (near {leftover!r})")

    fields = {
        "years": 0,
        "months": 0,
        "weeks": 0,
        "days": 0,
        "hours": 0,
        "minutes": 0,
        "seconds": 0,
    }
    for m in matches:
        n = int(m.group("n"))
        if m.group("sign") == "-":
            n = -n
        fields[_norm_unit(m.group("unit"))] += n
    # silence unused
    del covered
    return Offset(**fields)


def add_months(d: Instant, months: int) -> Instant:
    month0 = d.month - 1 + months
    year = d.year + month0 // 12
    month = month0 % 12 + 1
    try:
        last = calendar.monthrange(year, month)[1]
    except calendar.IllegalMonthError as exc:
        raise TimeWarpError(f"date overflow while adding {months} months to {format_instant(d)}") from exc
    day = min(d.day, last)
    try:
        return d.replace(year=year, month=month, day=day)
    except ValueError as exc:
        raise TimeWarpError(f"date overflow while adding {months} months to {format_instant(d)}") from exc


def add_years(d: Instant, years: int) -> Instant:
    return add_months(d, years * 12)


def apply_offset(start: Instant, offset: Offset) -> Instant:
    """Add years, then months, then weeks/days/time. Month-end days clamp (Jan 31 + 1 month = Feb 28/29)."""
    result: Instant = start
    if offset.years:
        result = add_years(result, offset.years)
    if offset.months:
        result = add_months(result, offset.months)
    delta = timedelta(
        weeks=offset.weeks,
        days=offset.days,
        hours=offset.hours,
        minutes=offset.minutes,
        seconds=offset.seconds,
    )
    if delta == timedelta(0):
        return result
    if not isinstance(result, datetime) and offset.has_time():
        result = datetime.combine(result, datetime.min.time())
    try:
        return result + delta
    except OverflowError as exc:
        raise TimeWarpError("date overflow applying offset") from exc


def _shift_end(start: Instant, end: Instant, include_end: bool) -> Instant:
    if not include_end:
        return end
    step = timedelta(days=1)
    if end >= start:
        return end + step
    return end - step


def _split_months(total_months: int) -> tuple[int, int]:
    """Toward-zero years so -13 months is -1 year, -1 month (not -2 years, +11 months)."""
    years = int(total_months / 12)
    return years, total_months - years * 12


def _split_remainder(delta: timedelta) -> tuple[int, int, int, int]:
    total = int(delta.total_seconds())
    sign = -1 if total < 0 else 1
    total = abs(total)
    days, rem = divmod(total, 86400)
    hours, rem = divmod(rem, 3600)
    minutes, seconds = divmod(rem, 60)
    return sign * days, sign * hours, sign * minutes, sign * seconds


def span(start: Instant, end: Instant, include_end: bool = False) -> CalendarSpan:
    """Signed duration from start to end. Components invert apply_offset from the start date."""
    start, end = same_kind(start, end)
    adjusted_end = _shift_end(start, end, include_end)

    raw = adjusted_end - start
    if isinstance(start, datetime):
        total_days = raw.days
        total_seconds = int(raw.total_seconds())
    else:
        total_days = raw.days
        total_seconds = total_days * 86400

    if adjusted_end == start:
        return CalendarSpan(0, 0, 0, 0, 0, 0, 0, 0, start, end, include_end)

    total_months = (adjusted_end.year - start.year) * 12 + (adjusted_end.month - start.month)
    candidate = add_months(start, total_months)
    if adjusted_end >= start:
        if candidate > adjusted_end:
            total_months -= 1
            candidate = add_months(start, total_months)
    else:
        if candidate < adjusted_end:
            total_months += 1
            candidate = add_months(start, total_months)

    years, months = _split_months(total_months)
    remainder = adjusted_end - candidate
    if not isinstance(remainder, timedelta):
        remainder = timedelta(days=int(remainder))
    days, hours, minutes, seconds = _split_remainder(remainder)
    return CalendarSpan(
        years=years,
        months=months,
        days=days,
        hours=hours,
        minutes=minutes,
        seconds=seconds,
        total_days=total_days,
        total_seconds=total_seconds,
        start=start,
        end=end,
        include_end=include_end,
    )
