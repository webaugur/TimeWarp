"""ISO 8601 parsing and formatting. No locale date orders."""

from __future__ import annotations

import calendar
import re
from datetime import date, datetime, time, timedelta, timezone
from typing import Union

from timewarp.errors import TimeWarpError

Instant = Union[date, datetime]

ISO_WEEKDAYS = (
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
)
MONTHS = (
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
)

_WEEK = re.compile(r"^(\d{4})-W(\d{2})(?:-(\d))?$", re.IGNORECASE)
_ORDINAL = re.compile(r"^(\d{4})-(\d{3})$")
_YMD = re.compile(r"^(\d{4})-(\d{2})-(\d{2})(.*)$")
_HMS = re.compile(
    r"^T?(\d{2}):(\d{2})(?::(\d{2})(?:\.(\d+))?)?(Z|[+-]\d{2}:?\d{2})?$",
    re.IGNORECASE,
)
_BEATS = re.compile(r"^@(\d{1,3}(?:\.\d+)?)$")
# Biel Mean Time: UTC+1 all year (Swatch Internet Time).
BMT = timezone(timedelta(hours=1), "BMT")

_RELATIVE = {
    "today": lambda: date.today(),
    "yesterday": lambda: date.today() - timedelta(days=1),
    "tomorrow": lambda: date.today() + timedelta(days=1),
    "now": lambda: datetime.now().replace(microsecond=0),
}


class ParseError(TimeWarpError):
    pass


def weekday_name(d: date) -> str:
    return ISO_WEEKDAYS[d.isoweekday() - 1]


def month_name(month: int) -> str:
    if not 1 <= month <= 12:
        raise ParseError(f"month {month} is not in 1..12")
    return MONTHS[month - 1]


def format_instant(value: Instant) -> str:
    if isinstance(value, datetime):
        if value.tzinfo is None:
            return value.replace(microsecond=0).isoformat(timespec="seconds")
        return value.astimezone(value.tzinfo).replace(microsecond=0).isoformat(timespec="seconds")
    return value.isoformat()


# NATO / military zone letters. J is unused (observer's local). Whole hours only;
# fractional offsets (India +5:30) use the letter of the truncated hour.
_MIL_EAST = "ZABCDEFGHIKLM"  # UTC+0 .. UTC+12
_MIL_WEST = "ZNOPQRSTUVWXY"  # UTC+0 .. UTC-12


def tz_letter(value: datetime) -> str:
    """NATO zone letter for a timezone-aware datetime (Q = UTC−4, R = UTC−5, Z = UTC)."""
    if value.tzinfo is None:
        return "J"
    off = value.utcoffset()
    if off is None:
        return "J"
    hours = int(off.total_seconds() / 3600)  # toward zero; +5:30 → E, −3:30 → P
    if hours >= 0:
        return _MIL_EAST[min(hours, 12)]
    return _MIL_WEST[min(-hours, 12)]


def swatch_beats(value: datetime) -> float:
    """Swatch beats after midnight BMT (UTC+1, no DST). One beat = 86.4 seconds.

    Naive datetimes are treated as UTC. Range is 0 inclusive to 1000 exclusive.
    """
    if value.tzinfo is None:
        utc = value.replace(tzinfo=timezone.utc)
    else:
        utc = value.astimezone(timezone.utc)
    secs = utc.hour * 3600 + utc.minute * 60 + utc.second + utc.microsecond / 1_000_000
    bmt = (secs + 3600.0) % 86400.0
    return bmt / 86.4


def format_swatch(value: datetime) -> str:
    """Canonical Swatch Internet Time: @000 … @999."""
    n = int(round(swatch_beats(value))) % 1000
    return f"@{n:03d}"


def datetime_from_beats(day: date, beats: float) -> datetime:
    """Instant at `beats` after midnight BMT on `day` (aware, UTC+1)."""
    if not 0 <= beats < 1000:
        raise ParseError(f"Swatch beats must be @000 … @999, got @{beats}")
    start = datetime(day.year, day.month, day.day, tzinfo=BMT)
    return start + timedelta(seconds=beats * 86.4)


def parse_beats(text: str) -> float:
    """Parse `@000` … `@999` (optional fraction). Leading T/space is allowed."""
    raw = text.strip()
    if raw[:1] in "Tt":
        raw = raw[1:].strip()
    m = _BEATS.fullmatch(raw)
    if not m:
        raise ParseError(
            f"invalid Swatch time {text!r}; use @000 … @999 (optional fraction, e.g. @500.5)"
        )
    beats = float(m.group(1))
    if not 0 <= beats < 1000:
        raise ParseError(f"Swatch beats must be @000 … @999, got {raw}")
    return beats


def format_clock(value: datetime) -> str:
    """Local HH:MM plus NATO zone letter and Swatch beats: 17:52R @994, 13:00Z @583."""
    local = value
    if value.tzinfo is not None:
        local = value.astimezone(value.tzinfo)
    return f"{local:%H:%M}{tz_letter(local)} {format_swatch(value)}"


def format_labeled(value: Instant) -> str:
    d = value.date() if isinstance(value, datetime) else value
    return f"{format_instant(value)} {weekday_name(d)}"


def _invalid_ymd(year: int, month: int, day: int) -> ParseError:
    if not 1 <= month <= 12:
        return ParseError(f"{year:04d}-{month:02d}-{day:02d} is not a valid date (month must be 01-12)")
    last = calendar.monthrange(year, month)[1]
    return ParseError(
        f"{year:04d}-{month:02d}-{day:02d} is not a valid date "
        f"({month_name(month)} {year} has {last} days). "
        f"Did you mean {year:04d}-{month:02d}-{last:02d}?"
    )


def _parse_tz(token: str | None) -> timezone | None:
    if token is None:
        return None
    if token.upper() == "Z":
        return timezone.utc
    sign = 1 if token[0] == "+" else -1
    digits = token[1:].replace(":", "")
    if len(digits) != 4 or not digits.isdigit():
        raise ParseError(f"invalid UTC offset {token!r}; use Z or +HH:MM")
    hours = int(digits[:2])
    minutes = int(digits[2:])
    if hours > 23 or minutes > 59:
        raise ParseError(f"invalid UTC offset {token!r}")
    return timezone(sign * timedelta(hours=hours, minutes=minutes))


def _parse_time_suffix(suffix: str) -> tuple[time, timezone | None] | None:
    text = suffix.strip()
    if not text:
        return None
    if text[0] in "Tt ":
        text = "T" + text[1:].strip()
    elif text[0].isdigit():
        text = "T" + text
    elif text[0] == "@":
        raise ParseError(f"invalid time {suffix!r}; use ISO 8601 HH:MM[:SS][Z|+HH:MM] or @beats")
    else:
        raise ParseError(f"unexpected trailing date text {suffix!r}")
    m = _HMS.fullmatch(text)
    if not m:
        raise ParseError(
            f"invalid time {suffix!r}; use ISO 8601 HH:MM[:SS][Z|+HH:MM] or @beats"
        )
    hour = int(m.group(1))
    minute = int(m.group(2))
    second = int(m.group(3) or 0)
    tz = _parse_tz(m.group(5))
    try:
        return time(hour, minute, second), tz
    except ValueError as exc:
        raise ParseError(f"invalid time {suffix.strip()!r}: {exc}") from exc


def looks_like_instant(text: str) -> bool:
    """True when `text` is a relative word, ISO date/week/ordinal, or @beats (not a body name)."""
    raw = text.strip()
    if not raw:
        return False
    if raw.lower() in _RELATIVE:
        return True
    if raw.startswith("@") or "T@" in raw.upper() or " @" in raw:
        return True
    if _WEEK.fullmatch(raw) or _ORDINAL.fullmatch(raw) or _YMD.match(raw):
        return True
    return False


def parse_instant(text: str) -> Instant:
    """Parse an ISO 8601 date or date-time, or a Swatch `@beats` time.

    Relative words today/now/yesterday/tomorrow are also accepted.
    `@500` is that beat on the current BMT date. `YYYY-MM-DDT@500` (or a space
    before `@`) is that beat on that BMT calendar date.
    """
    raw = text.strip()
    if not raw:
        raise ParseError("empty date")

    key = raw.lower()
    if key in _RELATIVE:
        return _RELATIVE[key]()

    if raw.startswith("@"):
        beats = parse_beats(raw)
        bmt_day = datetime.now(timezone.utc).astimezone(BMT).date()
        return datetime_from_beats(bmt_day, beats)

    week = _WEEK.fullmatch(raw)
    if week:
        year = int(week.group(1))
        weekno = int(week.group(2))
        weekday = int(week.group(3) or 1)
        if not 1 <= weekday <= 7:
            raise ParseError(f"ISO weekday must be 1-7 (Monday-Sunday), got {weekday}")
        try:
            return date.fromisocalendar(year, weekno, weekday)
        except ValueError as exc:
            raise ParseError(f"invalid ISO week date {raw!r}: {exc}") from exc

    ordinal = _ORDINAL.fullmatch(raw)
    if ordinal:
        year = int(ordinal.group(1))
        daynum = int(ordinal.group(2))
        try:
            return date(year, 1, 1) + timedelta(days=daynum - 1)
        except ValueError as exc:
            raise ParseError(f"invalid ordinal date {raw!r}: {exc}") from exc

    ymd = _YMD.match(raw)
    if ymd:
        year, month, day = int(ymd.group(1)), int(ymd.group(2)), int(ymd.group(3))
        rest = ymd.group(4)
        try:
            d = date(year, month, day)
        except ValueError as exc:
            raise _invalid_ymd(year, month, day) from exc
        if not rest.strip():
            return d
        beat_text = rest.strip()
        if beat_text[:1] in "Tt":
            beat_body = beat_text[1:].strip()
        else:
            beat_body = beat_text
        if beat_body.startswith("@"):
            return datetime_from_beats(d, parse_beats(beat_body))
        parsed_time = _parse_time_suffix(rest)
        if parsed_time is None:
            return d
        clock, tz = parsed_time
        return datetime.combine(d, clock, tzinfo=tz)

    # datetime.fromisoformat as a last resort for extended forms
    try:
        return datetime.fromisoformat(raw)
    except ValueError:
        pass
    raise ParseError(
        f"invalid date {raw!r}; use ISO 8601 (YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS[Z|+HH:MM]) "
        f"or Swatch @beats (@500, 2026-07-04T@500)"
    )


def as_date(value: Instant) -> date:
    return value.date() if isinstance(value, datetime) else value


def promote_datetime(value: Instant) -> datetime:
    if isinstance(value, datetime):
        return value
    return datetime.combine(value, time.min)


def same_kind(a: Instant, b: Instant) -> tuple[Instant, Instant]:
    """If either side has a time, promote the other to midnight. Refuse mixed aware/naive."""
    a_dt = isinstance(a, datetime)
    b_dt = isinstance(b, datetime)
    if not a_dt and not b_dt:
        return a, b
    a2 = promote_datetime(a)
    b2 = promote_datetime(b)
    a_aware = a2.tzinfo is not None
    b_aware = b2.tzinfo is not None
    if a_aware != b_aware:
        raise ParseError(
            "cannot mix timezone-aware and naive datetimes; give both a Z/+HH:MM offset or neither"
        )
    return a2, b2
