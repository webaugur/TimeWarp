"""Solar and lunar eclipse catalog, 1900–2199 (Meeus, Astronomical Algorithms ch. 54).

Dates of greatest eclipse are in UTC. Types follow Meeus γ and u (solar:
partial / annular / total / hybrid; lunar: penumbral / partial / total).
Acknowledgement: classification matches the Espenak/Meeus canon in spirit;
this is a compact implementation, not a copy of NASA decade HTML tables.
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from datetime import date, datetime, timedelta, timezone

from timewarp.iso import Instant, as_date, weekday_name

COVERAGE_START = date(1900, 1, 1)
COVERAGE_END = date(2199, 12, 31)


@dataclass(frozen=True)
class Eclipse:
    date: date
    kind: str  # solar | lunar
    type: str
    end_date: date | None = None  # lunar events that span two UTC dates


def _sind(x: float) -> float:
    return math.sin(math.radians(x))


def _jde_to_datetime(jde: float) -> datetime:
    return datetime(2000, 1, 1, 12, tzinfo=timezone.utc) + timedelta(days=jde - 2451545.0)


def _syzygy(k: float) -> tuple[float, float, float, float, float, float, float]:
    """Return JDE, T, E, M°, M'°, F°, Ω° for lunation k (integer=new, +0.5=full)."""
    t = k / 1236.85
    jde = (
        2451550.09766
        + 29.530588861 * k
        + 0.00015437 * t * t
        - 0.000000150 * t**3
        + 0.00000000073 * t**4
    )
    e = 1.0 - 0.002516 * t - 0.0000074 * t * t
    m = 2.5534 + 29.10535670 * k - 0.0000014 * t * t - 0.00000011 * t**3
    mp = 201.5643 + 385.81693528 * k + 0.0107582 * t * t + 0.00001238 * t**3 - 0.000000058 * t**4
    f = 160.7108 + 390.67050284 * k - 0.0016118 * t * t - 0.00000227 * t**3 + 0.000000011 * t**4
    om = 124.7746 - 1.56375588 * k + 0.0020672 * t * t + 0.00000215 * t**3
    jde += (
        -0.40720 * _sind(mp)
        + 0.17241 * e * _sind(m)
        + 0.01608 * _sind(2 * mp)
        + 0.01039 * _sind(2 * f)
        + 0.00739 * e * _sind(mp - m)
        - 0.00514 * e * _sind(mp + m)
        + 0.00208 * e * e * _sind(2 * m)
        - 0.00111 * _sind(mp - 2 * f)
        - 0.00057 * _sind(mp + 2 * f)
        + 0.00056 * e * _sind(2 * mp + m)
        - 0.00042 * _sind(3 * mp)
        + 0.00042 * e * _sind(m + 2 * f)
        + 0.00038 * e * _sind(m - 2 * f)
        - 0.00024 * e * _sind(2 * mp - m)
        - 0.00017 * _sind(om)
    )
    return jde, t, e, m, mp, f, om


def _gamma_u(jde: float, k: float, e: float, m: float, mp: float, f: float, om: float) -> tuple[float, float]:
    """Meeus ch. 54: γ (g) and u in Earth radii."""
    t = (jde - 2451545.0) / 36525.0
    fr = math.radians(f)
    mr = math.radians(m)
    mm = math.radians(mp)
    wr = math.radians(om)
    # Recompute F, M, M' at T from JDE for the P,Q series (Meeus uses this T).
    f1 = fr - math.radians(0.02665) * math.sin(wr)
    ee = 1.0 - 0.002516 * t - 0.0000047 * t * t
    p = (
        0.2070 * ee * math.sin(mr)
        + 0.0024 * ee * math.sin(2 * mr)
        - 0.0392 * math.sin(mm)
        + 0.0116 * math.sin(2 * mm)
        - 0.0073 * ee * math.sin(mm + mr)
        + 0.0067 * ee * math.sin(mm - mr)
        + 0.0118 * math.sin(2 * f1)
    )
    q = (
        5.2207
        - 0.0048 * ee * math.cos(mr)
        + 0.0020 * ee * math.cos(2 * mr)
        - 0.3299 * math.cos(mm)
        - 0.0060 * ee * math.cos(mm + mr)
        + 0.0041 * ee * math.cos(mm - mr)
    )
    w = abs(math.cos(f1))
    g = (p * math.cos(f1) + q * math.sin(f1)) * (1.0 - 0.0048 * w)
    u = 0.0059 + 0.0046 * ee * math.cos(mr) - 0.0182 * math.cos(mm) + 0.0004 * math.cos(2 * mm) - 0.0005 * ee * math.cos(mr + mm)
    return g, u


def _solar_type(g: float, u: float) -> str | None:
    ag = abs(g)
    if ag > 1.5433 + u:
        return None
    if ag >= 0.9972 + abs(u):
        return "partial"
    if u < 0:
        return "total"
    if u > 0.0047:
        return "annular"
    if u < 0.00464 * math.sqrt(max(0.0, 1.0 - g * g)):
        return "hybrid"
    return "annular"


def _lunar_type(g: float, u: float) -> str | None:
    mp = (1.5573 + u - abs(g)) / 0.5450
    if mp < 0:
        return None
    mu = (1.0128 - u - abs(g)) / 0.5450
    if mu < 0:
        return "penumbral"
    if mu < 1:
        return "partial"
    return "total"


def _eclipse_at(k: float) -> Eclipse | None:
    jde, t, e, m, mp, f, om = _syzygy(k)
    g, u = _gamma_u(jde, k, e, m, mp, f, om)
    solar = abs(k - round(k)) < 1e-9
    kind = "solar" if solar else "lunar"
    typ = _solar_type(g, u) if solar else _lunar_type(g, u)
    if typ is None:
        return None
    when = _jde_to_datetime(jde)
    d = when.date()
    end = None
    if not solar and when.hour < 8:
        # Greatest eclipse in the first hours of UTC often started the previous date.
        prev = d - timedelta(days=1)
        end = d
        d = prev
    return Eclipse(d, kind, typ, end)


def _build_catalog() -> tuple[Eclipse, ...]:
    k0 = math.floor((COVERAGE_START.year - 2000) * 12.3685) - 2
    k1 = math.ceil((COVERAGE_END.year + 1 - 2000) * 12.3685) + 2
    rows: list[Eclipse] = []
    k = float(k0)
    while k <= k1:
        for kk in (k, k + 0.5):
            ev = _eclipse_at(kk)
            if ev is None:
                continue
            if ev.date < COVERAGE_START or ev.date > COVERAGE_END:
                continue
            rows.append(ev)
        k += 1.0
    rows.sort(key=lambda e: e.date)
    return tuple(rows)


ECLIPSES: tuple[Eclipse, ...] = _build_catalog()


def iso_range(e: Eclipse) -> str:
    if e.end_date:
        return f"{e.date.isoformat()}/{e.end_date.isoformat()}"
    return e.date.isoformat()


def list_eclipses(*, year: int | None = None, after: Instant | None = None, limit: int | None = None) -> list[Eclipse]:
    rows = list(ECLIPSES)
    if year is not None:
        if year < COVERAGE_START.year or year > COVERAGE_END.year:
            return []
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
