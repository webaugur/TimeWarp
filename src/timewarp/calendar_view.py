"""Year, month, and week calendars (holidays marked)."""

from __future__ import annotations

import calendar
import re
from datetime import date, timedelta

from timewarp.errors import TimeWarpError
from timewarp.holidays import holidays_for_year
from timewarp.iso import MONTHS, weekday_name
from timewarp.ui import holiday_emoji

# Screenshot uses Sunday-first US layout; --iso switches to Monday-first.

_YEAR = re.compile(r"^\d{4}$")
_YM = re.compile(r"^(\d{4})-(\d{2})$")
_WEEK = re.compile(r"^(\d{4})-W(\d{2})(?:-(\d))?$", re.IGNORECASE)
_YMD = re.compile(r"^(\d{4})-(\d{2})-(\d{2})$")

_MONTH_CELL = 12


def _calendar_observances(year: int) -> list[tuple[date, str]]:
    """Display-only dates (not workday skips). Fill Christmas Eve / Orthodox pair."""
    try:
        return [
            (date(year, 12, 24), "Christmas Eve"),
            (date(year, 1, 6), "Orthodox Christmas Eve"),
            (date(year, 1, 7), "Orthodox Christmas"),
        ]
    except ValueError:
        return []


def _holiday_map(year: int, country: str, *, refresh: bool, region: str | None) -> tuple[dict[date, str], str]:
    country_key = country.strip() if country else ""
    if not country_key:
        return {}, "no holidays"
    rows, _note = holidays_for_year(year, country_key, refresh=refresh, region=region)
    mapping = {d: name for d, name in rows}
    for d, name in _calendar_observances(year):
        mapping.setdefault(d, name)
    return mapping, country_key.strip().upper()


def year_calendar(
    year: int,
    *,
    country: str = "US",
    iso_weeks: bool = False,
    refresh: bool = False,
    region: str | None = None,
    emoji: bool = False,
) -> str:
    first = calendar.MONDAY if iso_weeks else calendar.SUNDAY
    cal = calendar.Calendar(firstweekday=first)
    holiday_map, title_country = _holiday_map(year, country, refresh=refresh, region=region)

    header_days = (
        ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
        if iso_weeks
        else ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
    )
    title = f"📅 Calendar {year} ({title_country})" if emoji else f"Calendar {year} ({title_country})"
    lines = [title, ""]

    months = []
    for month in range(1, 13):
        block = [f"{MONTHS[month - 1]} {year}", " ".join(f"{h:>3}" for h in header_days)]
        weeks = cal.monthdayscalendar(year, month)
        for week in weeks:
            cells = []
            for day in week:
                if day == 0:
                    cells.append("   ")
                    continue
                d = date(year, month, day)
                mark = "*" if d in holiday_map else " "
                cells.append(f"{day:2d}{mark}")
            block.append(" ".join(cells))
        months.append(block)

    # 3 months across
    def pad(block: list[str], width: int = 7) -> list[str]:
        row_w = 4 * width - 1
        out = [block[0].center(row_w), *block[1:]]
        while len(out) < 8:
            out.append("")
        return [line.ljust(row_w) for line in out[:8]]

    for row in range(0, 12, 3):
        cols = [pad(months[row + i]) for i in range(3)]
        height = max(len(c) for c in cols)
        for i in range(height):
            lines.append("   ".join(c[i] if i < len(c) else " " * len(c[0]) for c in cols))
        lines.append("")

    if holiday_map:
        hol_h = "Holidays:" if emoji else "Holidays (*):"
        lines.append(hol_h)
        for d in sorted(holiday_map):
            tag = f"{holiday_emoji(holiday_map[d], when=d)} " if emoji else ""
            lines.append(f"  {d.isoformat()} {weekday_name(d)}  {tag}{holiday_map[d]}")
    lines.append("")
    foot = "Dates are ISO 8601 (YYYY-MM-DD). * = holiday."
    lines.append(foot)
    return "\n".join(lines).rstrip() + "\n"


def parse_calendar_spec(text: str | None) -> tuple[str, date]:
    """Return ('year'|'month'|'week', anchor date). Omit text → this year."""
    if not text or not str(text).strip():
        today = date.today()
        return "year", date(today.year, 1, 1)
    raw = text.strip()
    if _YEAR.fullmatch(raw):
        year = int(raw)
        if not 1 <= year <= 9999:
            raise TimeWarpError(f"year {year} is out of range 1..9999")
        return "year", date(year, 1, 1)
    ym = _YM.fullmatch(raw)
    if ym:
        year, month = int(ym.group(1)), int(ym.group(2))
        try:
            return "month", date(year, month, 1)
        except ValueError as exc:
            raise TimeWarpError(f"invalid month {raw}") from exc
    wk = _WEEK.fullmatch(raw)
    if wk:
        year, week = int(wk.group(1)), int(wk.group(2))
        try:
            anchor = date.fromisocalendar(year, week, 1)
        except ValueError as exc:
            raise TimeWarpError(f"invalid ISO week {raw}") from exc
        return "week", anchor
    ymd = _YMD.fullmatch(raw)
    if ymd:
        try:
            d = date(int(ymd.group(1)), int(ymd.group(2)), int(ymd.group(3)))
        except ValueError as exc:
            raise TimeWarpError(f"invalid date {raw}") from exc
        return "week", d
    raise TimeWarpError(
        f"invalid calendar date {raw!r}; use YYYY, YYYY-MM, YYYY-Www, or YYYY-MM-DD"
    )


def _week_num(d: date) -> int:
    return d.isocalendar().week


def month_calendar(
    year: int,
    month: int,
    *,
    country: str = "US",
    iso_weeks: bool = False,
    refresh: bool = False,
    region: str | None = None,
    emoji: bool = False,
) -> str:
    first = calendar.MONDAY if iso_weeks else calendar.SUNDAY
    cal = calendar.Calendar(firstweekday=first)
    holiday_map, title_country = _holiday_map(year, country, refresh=refresh, region=region)
    header_days = (
        ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"]
        if iso_weeks
        else ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"]
    )
    w = _MONTH_CELL
    name = MONTHS[month - 1]
    title = f"{name} {year} ({title_country})"
    if emoji:
        title = f"📅 {title}"
    lines = [title, ""]
    hdr = "Wk  " + " ".join(h.center(w) for h in header_days)
    lines.append(hdr)
    for week in cal.monthdatescalendar(year, month):
        nums = []
        names = []
        thu = week[3] if iso_weeks else week[4]
        wk = _week_num(thu)
        for d in week:
            if d.month != month:
                nums.append(" " * w)
                names.append(" " * w)
                continue
            hol = holiday_map.get(d)
            star = "*" if hol else " "
            nums.append(f"{d.day:2d}{star}".ljust(w))
            label = hol or ""
            if len(label) > w:
                label = label[: w - 1] + "…"
            names.append(label.ljust(w))
        lines.append(f"{wk:2d}  " + " ".join(nums))
        lines.append("    " + " ".join(names))
        lines.append("")
    month_hols = sorted(d for d in holiday_map if d.year == year and d.month == month)
    if month_hols:
        hol_h = "Holidays:" if emoji else "Holidays (*):"
        lines.append(hol_h)
        for d in month_hols:
            tag = f"{holiday_emoji(holiday_map[d], when=d)} " if emoji else ""
            lines.append(f"  {d.isoformat()} {weekday_name(d)}  {tag}{holiday_map[d]}")
        lines.append("")
    foot = "Dates are ISO 8601 (YYYY-MM-DD). * = holiday."
    lines.append(foot)
    return "\n".join(lines).rstrip() + "\n"


def week_calendar(
    start: date,
    *,
    country: str = "US",
    iso_weeks: bool = False,
    refresh: bool = False,
    region: str | None = None,
    emoji: bool = False,
) -> str:
    _ = iso_weeks
    monday = start - timedelta(days=start.isocalendar().weekday - 1)
    days = [monday + timedelta(days=i) for i in range(7)]
    iso = monday.isocalendar()
    label = f"{iso.year:04d}-W{iso.week:02d}"
    years = sorted({d.year for d in days})
    holiday_map: dict[date, str] = {}
    title_country = "no holidays"
    for y in years:
        hm, title_country = _holiday_map(y, country, refresh=refresh, region=region)
        holiday_map.update(hm)
    title = f"Week {label} ({title_country})"
    if emoji:
        title = f"📅 {title}"
    lines = [title, ""]
    for d in days:
        hol = holiday_map.get(d, "")
        mark = "*" if hol else " "
        tag = f"{holiday_emoji(hol, when=d)} " if emoji and hol else ""
        extra = f"  {tag}{hol}" if hol else ""
        lines.append(f"{weekday_name(d):10}  {d.isoformat()}{mark}{extra}")
    lines.append("")
    foot = "Dates are ISO 8601 (YYYY-MM-DD). * = holiday."
    lines.append(foot)
    return "\n".join(lines).rstrip() + "\n"


def format_calendar(
    spec: str | None,
    *,
    country: str = "US",
    iso_weeks: bool = False,
    refresh: bool = False,
    region: str | None = None,
    emoji: bool = False,
) -> tuple[str, str, date]:
    kind, anchor = parse_calendar_spec(spec)
    if kind == "year":
        text = year_calendar(
            anchor.year, country=country, iso_weeks=iso_weeks, refresh=refresh, region=region, emoji=emoji
        )
    elif kind == "month":
        text = month_calendar(
            anchor.year,
            anchor.month,
            country=country,
            iso_weeks=iso_weeks,
            refresh=refresh,
            region=region,
            emoji=emoji,
        )
    else:
        text = week_calendar(
            anchor, country=country, iso_weeks=iso_weeks, refresh=refresh, region=region, emoji=emoji
        )
    return kind, text, anchor

