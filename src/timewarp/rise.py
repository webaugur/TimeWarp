"""Rise, set, and transit for the Sun, Moon, and planets.

Events are found on a local calendar day by scanning altitude. That avoids an
infinite loop on days the Moon never rises (about one day per month).
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date, datetime, time, timedelta
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError

from timewarp.ephem import SkyPos, altitude_azimuth, normalize_body, position
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
    note: str | None
    position: SkyPos

    def to_dict(self) -> dict:
        return {
            "body": self.body,
            "date": self.date.isoformat(),
            "place": self.place.name,
            "latitude": self.place.lat,
            "longitude": self.place.lon,
            "tz": self.place.tz,
            "rise": [format_instant(t) for t in self.rises],
            "set": [format_instant(t) for t in self.sets],
            "transit": [format_instant(t) for t in self.transits],
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


def _alt(body: str, when: datetime, place: Place) -> tuple[float, float, SkyPos]:
    pos = position(body, when)
    alt, az = altitude_azimuth(pos, when, place.lat, place.lon)
    return alt - _h0(pos), az, pos


def _bisect_zero(
    body: str, place: Place, t0: datetime, t1: datetime, a0: float, a1: float
) -> datetime:
    lo, hi, alo, ahi = t0, t1, a0, a1
    for _ in range(40):
        if (hi - lo).total_seconds() < 0.5:
            break
        mid = lo + (hi - lo) / 2
        amid, _, _ = _alt(body, mid, place)
        # Keep a sign-changing bracket
        if alo == 0:
            return lo
        if (alo > 0) == (amid > 0):
            lo, alo = mid, amid
        else:
            hi, ahi = mid, amid
    del ahi
    return lo + (hi - lo) / 2


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

    alts: list[float] = []
    azs: list[float] = []
    poses: list[SkyPos] = []
    for when in times:
        alt, az, pos = _alt(name, when, place)
        alts.append(alt)
        azs.append(az)
        poses.append(pos)

    rises: list[datetime] = []
    sets: list[datetime] = []
    for i in range(len(times) - 1):
        a0, a1 = alts[i], alts[i + 1]
        if a0 == 0:
            when = times[i]
            if i + 1 < len(alts) and alts[i + 1] > 0:
                rises.append(when)
            elif i + 1 < len(alts) and alts[i + 1] < 0:
                sets.append(when)
            continue
        if a0 * a1 > 0:
            continue
        if a0 * a1 == 0 and a1 == 0:
            continue
        crossing = _bisect_zero(name, place, times[i], times[i + 1], a0, a1)
        if not (start <= crossing < end):
            continue
        if a1 > a0:
            rises.append(crossing)
        else:
            sets.append(crossing)

    transits: list[datetime] = []
    # Transit ≈ maximum altitude in the window (meridian).
    peak_i = max(range(len(alts)), key=lambda i: alts[i])
    if 0 < peak_i < len(alts) - 1:
        transits.append(times[peak_i].replace(microsecond=0))
    elif alts[peak_i] > 0:
        transits.append(times[peak_i].replace(microsecond=0))

    noon = start + timedelta(hours=12)
    noon_pos = position(name, noon)

    note = None
    if not rises and not sets:
        if alts[len(alts) // 2] > 0:
            note = f"{name.capitalize()} stays above the horizon"
        else:
            note = f"{name.capitalize()} stays below the horizon"
    elif not rises:
        note = f"{name.capitalize()} does not rise this local day"
    elif not sets:
        note = f"{name.capitalize()} does not set this local day"

    def _trim(seq: list[datetime]) -> tuple[datetime, ...]:
        out = []
        for item in seq:
            item = item.astimezone(tz).replace(microsecond=0)
            if start <= item < end:
                out.append(item)
        return tuple(out)

    return RiseSet(
        body=name,
        date=civil,
        place=place,
        rises=_trim(rises),
        sets=_trim(sets),
        transits=_trim(transits),
        note=note,
        position=noon_pos,
    )
