"""Basic eclipse catalog (NASA / Fred Espenak decade tables, 2021–2030)."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date

from timewarp.iso import Instant, as_date, weekday_name

# Eclipse Predictions by Fred Espenak, NASA GSFC.
# Solar: https://eclipse.gsfc.nasa.gov/SEdecade/SEdecade2021.html
# Lunar: https://eclipse.gsfc.nasa.gov/LEdecade/LEdecade2021.html


@dataclass(frozen=True)
class Eclipse:
    date: date
    kind: str  # solar | lunar
    type: str
    end_date: date | None = None  # lunar events that span two UTC dates


ECLIPSES: tuple[Eclipse, ...] = (
    Eclipse(date(2021, 5, 26), "lunar", "total"),
    Eclipse(date(2021, 6, 10), "solar", "annular"),
    Eclipse(date(2021, 11, 19), "lunar", "partial"),
    Eclipse(date(2021, 12, 4), "solar", "total"),
    Eclipse(date(2022, 4, 30), "solar", "partial"),
    Eclipse(date(2022, 5, 16), "lunar", "total"),
    Eclipse(date(2022, 10, 25), "solar", "partial"),
    Eclipse(date(2022, 11, 8), "lunar", "total"),
    Eclipse(date(2023, 4, 20), "solar", "hybrid"),
    Eclipse(date(2023, 5, 5), "lunar", "penumbral"),
    Eclipse(date(2023, 10, 14), "solar", "annular"),
    Eclipse(date(2023, 10, 28), "lunar", "partial"),
    Eclipse(date(2024, 3, 25), "lunar", "penumbral"),
    Eclipse(date(2024, 4, 8), "solar", "total"),
    Eclipse(date(2024, 9, 18), "lunar", "partial"),
    Eclipse(date(2024, 10, 2), "solar", "annular"),
    Eclipse(date(2025, 3, 14), "lunar", "total"),
    Eclipse(date(2025, 3, 29), "solar", "partial"),
    Eclipse(date(2025, 9, 7), "lunar", "total"),
    Eclipse(date(2025, 9, 21), "solar", "partial"),
    Eclipse(date(2026, 2, 17), "solar", "annular"),
    Eclipse(date(2026, 3, 3), "lunar", "total"),
    Eclipse(date(2026, 8, 12), "solar", "total"),
    Eclipse(date(2026, 8, 27), "lunar", "partial", end_date=date(2026, 8, 28)),
    Eclipse(date(2027, 2, 6), "solar", "annular"),
    Eclipse(date(2027, 2, 20), "lunar", "penumbral", end_date=date(2027, 2, 21)),
    Eclipse(date(2027, 7, 18), "lunar", "penumbral"),
    Eclipse(date(2027, 8, 2), "solar", "total"),
    Eclipse(date(2027, 8, 17), "lunar", "penumbral"),
    Eclipse(date(2028, 1, 12), "lunar", "partial"),
    Eclipse(date(2028, 1, 26), "solar", "annular"),
    Eclipse(date(2028, 7, 6), "lunar", "partial"),
    Eclipse(date(2028, 7, 22), "solar", "total"),
    Eclipse(date(2028, 12, 31), "lunar", "total"),
    Eclipse(date(2029, 1, 14), "solar", "partial"),
    Eclipse(date(2029, 6, 12), "solar", "partial"),
    Eclipse(date(2029, 6, 26), "lunar", "total"),
    Eclipse(date(2029, 7, 11), "solar", "partial"),
    Eclipse(date(2029, 12, 5), "solar", "partial"),
    Eclipse(date(2029, 12, 20), "lunar", "total"),
    Eclipse(date(2030, 6, 1), "solar", "annular"),
    Eclipse(date(2030, 6, 15), "lunar", "partial"),
    Eclipse(date(2030, 11, 25), "solar", "total"),
    Eclipse(date(2030, 12, 9), "lunar", "penumbral"),
)


def iso_range(e: Eclipse) -> str:
    if e.end_date:
        return f"{e.date.isoformat()}/{e.end_date.isoformat()}"
    return e.date.isoformat()


def list_eclipses(*, year: int | None = None, after: Instant | None = None, limit: int | None = None) -> list[Eclipse]:
    rows = list(ECLIPSES)
    if year is not None:
        rows = [e for e in rows if e.date.year == year or (e.end_date and e.end_date.year == year)]
    if after is not None:
        cutoff = as_date(after)
        rows = [e for e in rows if (e.end_date or e.date) >= cutoff]
    rows.sort(key=lambda e: e.date)
    if limit is not None:
        rows = rows[:limit]
    return rows


def eclipse_to_dict(e: Eclipse) -> dict:
    return {
        "date": iso_range(e),
        "kind": e.kind,
        "type": e.type,
        "weekday": weekday_name(e.date),
    }
