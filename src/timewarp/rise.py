"""Rise, set, and transit for the Sun, Moon, and planets.

Events are found on a local calendar day by scanning altitude. That avoids an
infinite loop on days the Moon never rises (about one day per month).
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date, datetime, time, timedelta
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError

from timewarp.ephem import SkyPos, altitude_azimuth, body_symbol, normalize_body, position
from timewarp.errors import TimeWarpError
from timewarp.iso import Instant, as_date, format_instant
from timewarp.places import Place

# Horizon altitudes (degrees). Sun: upper limb + refraction. Planets: refraction.
# Moon: refraction only; parallax is already applied in altitude_azimuth.
H0_SUN = -0.833
H0_STAR = -0.5667
H0_MOON = -0.583  # plus upper-limb: subtract nothing extra here; we add semidiameter to alt


@dataclass(frozen=True)
class Event:
    kind: str  # rise | set | transit
    time: datetime
    azimuth_deg: float | None
    altitude_deg: float | None


@dataclass(frozen=True)
class RiseSet:
    body: str
    date: date
    place: Place
    rises: tuple[datetime, ...]
    sets: tuple[datetime, ...]
    transits: tuple[datetime, ...]
    after_rise_13: tuple[datetime, ...]
    before_set_13: tuple[datetime, ...]
    after_rise_33: tuple[datetime, ...]
    before_set_33: tuple[datetime, ...]
    note: str | None
    position: SkyPos
    visible: bool

    def to_dict(self) -> dict:
        return {
            "body": self.body,
            "symbol": body_symbol(self.body),
            "date": self.date.isoformat(),
            "place": self.place.name,
            "latitude": self.place.lat,
            "longitude": self.place.lon,
            "tz": self.place.tz,
            "visible": self.visible,
            "rise": [format_instant(t) for t in self.rises],
            "set": [format_instant(t) for t in self.sets],
            "transit": [format_instant(t) for t in self.transits],
            "after_rise_13": [format_instant(t) for t in self.after_rise_13],
            "before_set_13": [format_instant(t) for t in self.before_set_13],
            "after_rise_33": [format_instant(t) for t in self.after_rise_33],
            "before_set_33": [format_instant(t) for t in self.before_set_33],
            "note": self.note,
            "ra_deg": round(self.position.ra_deg, 4),
            "dec_deg": round(self.position.dec_deg, 4),
            "distance": round(self.position.distance, 6),
            "distance_unit": self.position.distance_unit,
            "elongation_deg": None
            if self.position.elongation_deg is None
            else round(self.position.elongation_deg, 3),
            "phase": None if self.position.phase is None else round(self.position.phase, 4),
            "magnitude": None if self.position.magnitude is None else round(self.position.magnitude, 2),
        }


def _zone(name: str) -> ZoneInfo:
    try:
        return ZoneInfo(name)
    except ZoneInfoNotFoundError as exc:
        raise TimeWarpError(f"unknown time zone {name!r}; use an IANA name like America/New_York") from exc


def local_civil_date(inst: Instant, place: Place) -> date:
    tz = _zone(place.tz)
    if isinstance(inst, datetime):
        if inst.tzinfo is not None:
            return inst.astimezone(tz).date()
        return inst.date()
    return as_date(inst)


def _h0(pos: SkyPos) -> float:
    if pos.body == "sun":
        return H0_SUN
    if pos.body == "moon":
        # Topocentric altitude of disk center vs refraction. Upper limb:
        # treat limb as reaching the refracted horizon.
        return H0_MOON - pos.semidiameter_deg
    return H0_STAR


def _observe(body: str, when: datetime, place: Place) -> tuple[float, float, SkyPos]:
    """Geometric altitude (degrees), azimuth, position."""
    pos = position(body, when)
    alt, az = altitude_azimuth(pos, when, place.lat, place.lon)
    return alt, az, pos


def _alt(body: str, when: datetime, place: Place) -> tuple[float, float, SkyPos]:
    alt, az, pos = _observe(body, when, place)
    return alt - _h0(pos), az, pos


def _bisect(t0: datetime, t1: datetime, a0: float, metric) -> datetime:
    lo, hi, alo = t0, t1, a0
    for _ in range(40):
        if (hi - lo).total_seconds() < 0.5:
            break
        mid = lo + (hi - lo) / 2
        amid = metric(mid)
        if alo == 0:
            return lo
        if (alo > 0) == (amid > 0):
            lo, alo = mid, amid
        else:
            hi = mid
    return lo + (hi - lo) / 2


def _crossings(
    times: list[datetime],
    samples: list[float],
    metric,
    start: datetime,
    end: datetime,
    tz: ZoneInfo,
) -> tuple[tuple[datetime, ...], tuple[datetime, ...]]:
    """Return (ascending, descending) crossings of metric=0."""
    up: list[datetime] = []
    down: list[datetime] = []
    for i in range(len(times) - 1):
        a0, a1 = samples[i], samples[i + 1]
        if a0 == 0:
            if a1 > 0:
                up.append(times[i])
            elif a1 < 0:
                down.append(times[i])
            continue
        if a0 * a1 > 0:
            continue
        if a1 == 0:
            continue
        crossing = _bisect(times[i], times[i + 1], a0, metric)
        if not (start <= crossing < end):
            continue
        if a1 > a0:
            up.append(crossing)
        else:
            down.append(crossing)

    def _trim(seq: list[datetime]) -> tuple[datetime, ...]:
        out = []
        for item in seq:
            item = item.astimezone(tz).replace(microsecond=0)
            if start <= item < end:
                out.append(item)
        return tuple(out)

    return _trim(up), _trim(down)


def events_for_day(body: str, day: Instant, place: Place) -> RiseSet:
    name = normalize_body(body)
    tz = _zone(place.tz)
    civil = local_civil_date(day, place)
    start = datetime.combine(civil, time.min, tzinfo=tz)
    end = start + timedelta(days=1)

    # 4-minute samples: 360 evaluations. Moon moves ~2° in that time; bisection finishes it.
    step = timedelta(minutes=4)
    times: list[datetime] = []
    t = start
    guard = 0
    while t <= end:
        times.append(t)
        t += step
        guard += 1
        if guard > 400:
            raise TimeWarpError("rise sampler exceeded the local day (internal error)")

    geom: list[float] = []
    horizon: list[float] = []
    for when in times:
        alt, _az, pos = _observe(name, when, place)
        geom.append(alt)
        horizon.append(alt - _h0(pos))

    def horizon_metric(when: datetime) -> float:
        a, _, p = _observe(name, when, place)
        return a - _h0(p)

    def alt_metric(target: float):
        def metric(when: datetime) -> float:
            a, _, _ = _observe(name, when, place)
            return a - target

        return metric

    rises, sets = _crossings(times, horizon, horizon_metric, start, end, tz)
    after_13, before_13 = _crossings(times, [g - 13.0 for g in geom], alt_metric(13.0), start, end, tz)
    after_33, before_33 = _crossings(times, [g - 33.0 for g in geom], alt_metric(33.0), start, end, tz)

    transits: list[datetime] = []
    peak_i = max(range(len(geom)), key=lambda i: geom[i])
    if 0 < peak_i < len(geom) - 1:
        transits.append(times[peak_i].replace(microsecond=0))
    elif horizon[peak_i] > 0:
        transits.append(times[peak_i].replace(microsecond=0))

    noon = start + timedelta(hours=12)
    noon_pos = position(name, noon)

    note = None
    if not rises and not sets:
        if horizon[len(horizon) // 2] > 0:
            note = f"{name.capitalize()} stays above the horizon"
        else:
            note = f"{name.capitalize()} stays below the horizon"
    elif not rises:
        note = f"{name.capitalize()} does not rise this local day"
    elif not sets:
        note = f"{name.capitalize()} does not set this local day"

    return RiseSet(
        body=name,
        date=civil,
        place=place,
        rises=rises,
        sets=sets,
        transits=tuple(transits),
        after_rise_13=after_13,
        before_set_13=before_13,
        after_rise_33=after_33,
        before_set_33=before_33,
        note=note,
        position=noon_pos,
        visible=any(a > 0 for a in horizon),
    )


MAX_PERIOD_DAYS = 366


def each_civil_day(start: Instant, end: Instant, place: Place) -> list[date]:
    a = local_civil_date(start, place)
    b = local_civil_date(end, place)
    if b < a:
        raise TimeWarpError(
            f"period start {a.isoformat()} is after end {b.isoformat()}"
        )
    days = (b - a).days + 1
    if days > MAX_PERIOD_DAYS:
        raise TimeWarpError(
            f"period {a.isoformat()}/{b.isoformat()} is {days} days; max is {MAX_PERIOD_DAYS}"
        )
    return [a + timedelta(days=i) for i in range(days)]


def events_for_period(
    body: str, start: Instant, end: Instant, place: Place
) -> list[RiseSet]:
    return [events_for_day(body, day, place) for day in each_civil_day(start, end, place)]
