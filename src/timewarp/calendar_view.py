"""Year calendar (Create calendar: year + country holidays)."""

from __future__ import annotations

import calendar
from datetime import date

from timewarp.holidays import holidays_for_year
from timewarp.iso import MONTHS, weekday_name

# Screenshot uses Sunday-first US layout; --iso switches to Monday-first.


def year_calendar(
    year: int,
    *,
    country: str = "US",
    iso_weeks: bool = False,
    refresh: bool = False,
    region: str | None = None,
) -> str:
    first = calendar.MONDAY if iso_weeks else calendar.SUNDAY
    cal = calendar.Calendar(firstweekday=first)
    holiday_map = {}
    country_key = country.strip() if country else ""
    if not country_key:
        title_country = "no holidays"
    else:
        rows, _note = holidays_for_year(year, country_key, refresh=refresh, region=region)
        holiday_map = {d: name for d, name in rows}
        title_country = country_key.strip().upper()

    header_days = (
        ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
        if iso_weeks
        else ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
    )
    lines = [f"Calendar {year} ({title_country})", ""]

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
        lines.append("Holidays (*):")
        for d in sorted(holiday_map):
            lines.append(f"  {d.isoformat()} {weekday_name(d)}  {holiday_map[d]}")
    lines.append("")
    lines.append("Dates are ISO 8601 (YYYY-MM-DD). * = holiday.")
    return "\n".join(lines).rstrip() + "\n"
