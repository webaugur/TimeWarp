"""Tropical/sidereal chart geometry from TimeWarp ecliptic longitudes.

Schlyter planets (~1–2′), mean node/Lilith from the Schlyter moon orbit,
Chiron from SBDB osculating elements. Ayanamsa is a mean polynomial, not DE.
`--explain` is geometry in English, not delineation.
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone

from timewarp.ephem import (
    acosd,
    asind,
    cosd,
    day_number,
    julian_day,
    lst_deg,
    position,
    rev,
    sind,
    sun_state,
    tand,
)
from timewarp.errors import TimeWarpError
from timewarp.iso import format_instant
from timewarp.places import Place

SIGNS = (
    "Aries",
    "Taurus",
    "Gemini",
    "Cancer",
    "Leo",
    "Virgo",
    "Libra",
    "Scorpio",
    "Sagittarius",
    "Capricorn",
    "Aquarius",
    "Pisces",
)
SIGN_ABBR = ("Ari", "Tau", "Gem", "Can", "Leo", "Vir", "Lib", "Sco", "Sag", "Cap", "Aqu", "Pis")

PLANETS = (
    "sun",
    "moon",
    "mercury",
    "venus",
    "mars",
    "jupiter",
    "saturn",
    "uranus",
    "neptune",
    "pluto",
)

ASPECTS = (
    ("conjunction", 0.0),
    ("sextile", 60.0),
    ("square", 90.0),
    ("trine", 120.0),
    ("opposition", 180.0),
)

HOUSE_SYSTEMS = ("placidus", "equal", "whole")
AYANAMSA_NAMES = ("lahiri", "fagan", "krishnamurti")

_LIGHTS = {"sun", "moon"}
_ANGLES = {"asc", "mc"}
_POINTS = {"node", "lilith", "chiron"}

# Polar Placidus is undefined when |tan φ tan ε| ≥ 1 (~66.5°).
_PLACIDUS_LAT_MAX = 66.0


@dataclass(frozen=True)
class SignPos:
    lon: float
    sign: str
    sign_index: int
    degree: float
    house: int | None = None
    retrograde: bool = False

    def label(self) -> str:
        d = int(self.degree)
        m = int(round((self.degree - d) * 60.0))
        if m == 60:
            d += 1
            m = 0
        return f"{self.sign} {d}°{m:02d}′"

    def to_dict(self) -> dict:
        return {
            "lon": round(self.lon, 4),
            "sign": self.sign,
            "degree": round(self.degree, 4),
            "house": self.house,
            "retrograde": self.retrograde,
            "label": self.label(),
        }


@dataclass(frozen=True)
class AspectHit:
    a: str
    b: str
    kind: str
    angle: float
    orb: float
    applying: bool | None

    def to_dict(self) -> dict:
        return {
            "a": self.a,
            "b": self.b,
            "kind": self.kind,
            "angle": self.angle,
            "orb": round(self.orb, 3),
            "applying": self.applying,
        }


@dataclass
class Chart:
    when: datetime
    place: Place
    frame: str
    ayanamsa_name: str | None
    ayanamsa_deg: float
    house_system: str
    house_note: str | None
    cusps: list[float]  # index 1..12, [0] unused
    angles: dict[str, SignPos]
    bodies: dict[str, SignPos]
    lots: dict[str, SignPos]
    aspects: list[AspectHit]
    notes: list[str] = field(default_factory=list)
    natal: bool = False
    transits: bool = False

    def to_dict(self) -> dict:
        ay = None
        if self.frame == "sidereal":
            ay = {"name": self.ayanamsa_name, "deg": round(self.ayanamsa_deg, 4)}
        return {
            "when": format_instant(self.when),
            "place": self.place.name,
            "latitude": self.place.lat,
            "longitude": self.place.lon,
            "tz": self.place.tz,
            "frame": self.frame,
            "ayanamsa": ay,
            "houses": {
                "system": self.house_system,
                "cusps": [round(self.cusps[i], 4) for i in range(1, 13)],
                "note": self.house_note,
            },
            "angles": {k: v.to_dict() for k, v in self.angles.items()},
            "bodies": {k: v.to_dict() for k, v in self.bodies.items()},
            "lots": {k: v.to_dict() for k, v in self.lots.items()},
            "aspects": [a.to_dict() for a in self.aspects],
            "notes": list(self.notes),
            "natal": self.natal,
            "transits": self.transits,
        }


def wrap180(deg: float) -> float:
    x = (deg + 180.0) % 360.0 - 180.0
    return x + 360.0 if x == -180.0 else x


def lon_to_sign(lon: float) -> tuple[str, int, float]:
    x = rev(lon)
    i = min(int(x // 30.0), 11)
    return SIGNS[i], i, x - 30.0 * i


def sign_pos(lon: float, *, house: int | None = None, retrograde: bool = False) -> SignPos:
    sign, idx, deg = lon_to_sign(lon)
    return SignPos(rev(lon), sign, idx, deg, house, retrograde)


def ayanamsa_deg(jd: float, name: str) -> float:
    """Mean ayanamsa in degrees. Not a DE/IAU precession series."""
    key = name.strip().lower()
    if key not in AYANAMSA_NAMES:
        raise TimeWarpError(
            f"unknown ayanamsa {name!r}; known: {', '.join(AYANAMSA_NAMES)}"
        )
    t_j2000 = (jd - 2451545.0) / 365.25
    lahiri = 23.85351111 + t_j2000 * (50.2388475 / 3600.0)
    if key == "lahiri":
        return lahiri
    if key == "krishnamurti":
        return lahiri - 5.8 / 60.0
    t_1950 = (jd - 2433282.5) / 365.25
    return 24.04121667 + t_1950 * (50.270955 / 3600.0)


def apply_frame(lon: float, ayan: float) -> float:
    return rev(lon - ayan)


def mc_longitude(ramc_deg: float, eps_deg: float) -> float:
    ramc = math.radians(ramc_deg)
    eps = math.radians(eps_deg)
    return rev(math.degrees(math.atan2(math.sin(ramc), math.cos(ramc) * math.cos(eps))))


def asc_longitude(ramc_deg: float, eps_deg: float, lat_deg: float) -> float:
    ramc = math.radians(ramc_deg)
    eps = math.radians(eps_deg)
    lat = math.radians(lat_deg)
    num = math.cos(ramc)
    den = -(math.sin(ramc) * math.cos(eps) + math.tan(lat) * math.sin(eps))
    return rev(math.degrees(math.atan2(num, den)))


def _placidus_iter(
    origin: float, eps: float, lat: float, frac: float, *, nocturnal: bool = False
) -> float:
    sign = -1.0 if nocturnal else 1.0
    ra = origin + sign * 90.0 * frac
    for _ in range(30):
        lon = mc_longitude(ra, eps)
        dec = asind(sind(eps) * sind(lon))
        arg = -tand(lat) * tand(dec)
        if abs(arg) >= 1.0:
            raise TimeWarpError("placidus")
        sa = acosd(arg)
        step = (180.0 - sa) if nocturnal else sa
        ra_n = origin + sign * step * frac
        if abs(wrap180(ra_n - ra)) < 1e-6:
            ra = ra_n
            break
        ra = ra_n
    return mc_longitude(ra, eps)


def placidus_cusps(ramc: float, eps: float, lat: float) -> list[float]:
    if abs(lat) >= _PLACIDUS_LAT_MAX:
        raise TimeWarpError("placidus")
    cusps = [0.0] * 13
    cusps[10] = mc_longitude(ramc, eps)
    cusps[4] = rev(cusps[10] + 180.0)
    cusps[1] = asc_longitude(ramc, eps, lat)
    cusps[7] = rev(cusps[1] + 180.0)
    try:
        cusps[11] = _placidus_iter(ramc, eps, lat, 1.0 / 3.0)
        cusps[12] = _placidus_iter(ramc, eps, lat, 2.0 / 3.0)
        raic = ramc + 180.0
        cusps[3] = _placidus_iter(raic, eps, lat, 1.0 / 3.0, nocturnal=True)
        cusps[2] = _placidus_iter(raic, eps, lat, 2.0 / 3.0, nocturnal=True)
    except (TimeWarpError, ValueError, ZeroDivisionError) as exc:
        raise TimeWarpError("placidus") from exc
    cusps[5] = rev(cusps[11] + 180.0)
    cusps[6] = rev(cusps[12] + 180.0)
    cusps[8] = rev(cusps[2] + 180.0)
    cusps[9] = rev(cusps[3] + 180.0)
    return cusps


def equal_cusps(asc: float) -> list[float]:
    return [0.0] + [rev(asc + 30.0 * i) for i in range(12)]


def whole_cusps(asc: float) -> list[float]:
    _, idx, _ = lon_to_sign(asc)
    start = idx * 30.0
    return [0.0] + [rev(start + 30.0 * i) for i in range(12)]


def house_of(lon: float, cusps: list[float], *, whole: bool = False, asc: float = 0.0) -> int:
    if whole:
        _, body_i, _ = lon_to_sign(lon)
        _, asc_i, _ = lon_to_sign(asc)
        return (body_i - asc_i) % 12 + 1
    for i in range(1, 13):
        a = cusps[i]
        b = cusps[i % 12 + 1]
        span = (b - a) % 360.0
        off = (lon - a) % 360.0
        if off < span or span < 1e-9:
            return i
    return 12


def mean_node_lon(d: float) -> float:
    return rev(125.1228 - 0.0529538083 * d)


def mean_lilith_lon(d: float) -> float:
    n = rev(125.1228 - 0.0529538083 * d)
    w = rev(318.0634 + 0.1643573223 * d)
    return rev(n + w + 180.0)


def _lot(asc: float, b: float, c: float) -> float:
    return rev(asc + b - c)


def hermetic_lots(asc: float, bodies: dict[str, float], *, day: bool) -> dict[str, float]:
    sun, moon = bodies["sun"], bodies["moon"]
    mer, ven = bodies["mercury"], bodies["venus"]
    mar, jup, sat = bodies["mars"], bodies["jupiter"], bodies["saturn"]
    if day:
        fortune = _lot(asc, moon, sun)
        spirit = _lot(asc, sun, moon)
    else:
        fortune = _lot(asc, sun, moon)
        spirit = _lot(asc, moon, sun)
    if day:
        necessity = _lot(asc, fortune, mer)
        eros = _lot(asc, ven, spirit)
        courage = _lot(asc, fortune, mar)
        victory = _lot(asc, jup, spirit)
        nemesis = _lot(asc, fortune, sat)
    else:
        necessity = _lot(asc, mer, fortune)
        eros = _lot(asc, spirit, ven)
        courage = _lot(asc, mar, fortune)
        victory = _lot(asc, spirit, jup)
        nemesis = _lot(asc, sat, fortune)
    return {
        "fortune": fortune,
        "spirit": spirit,
        "necessity": necessity,
        "eros": eros,
        "courage": courage,
        "victory": victory,
        "nemesis": nemesis,
    }


def default_orb(name: str, override: float | None) -> float:
    if override is not None:
        return override
    if name in _LIGHTS:
        return 8.0
    if name in _ANGLES:
        return 4.0
    if name in _POINTS or name in {
        "fortune",
        "spirit",
        "necessity",
        "eros",
        "courage",
        "victory",
        "nemesis",
    }:
        return 3.0
    return 6.0


def _sep(a: float, b: float) -> float:
    return abs(wrap180(a - b))


def scan_aspects(
    longs: dict[str, float],
    later: dict[str, float] | None,
    *,
    orb: float | None,
) -> list[AspectHit]:
    names = list(longs)
    hits: list[AspectHit] = []
    for i, a in enumerate(names):
        for b in names[i + 1 :]:
            if a in _ANGLES and b in _ANGLES:
                continue
            max_orb = max(default_orb(a, orb), default_orb(b, orb))
            delta = _sep(longs[a], longs[b])
            for kind, exact in ASPECTS:
                o = abs(delta - exact)
                if o <= max_orb:
                    applying = None
                    if later and a in later and b in later:
                        later_o = abs(_sep(later[a], later[b]) - exact)
                        applying = later_o < o - 1e-6
                    hits.append(AspectHit(a, b, kind, exact, o, applying))
                    break
    hits.sort(key=lambda h: (h.orb, h.a, h.b))
    return hits


def _chiron_lon(when: datetime) -> float | None:
    """Use a cached SBDB dump or file; do not fetch JPL here."""
    from timewarp.ephem import _DYNAMIC
    from timewarp.jpl import lookup_catalog

    hit = lookup_catalog("chiron") or lookup_catalog("2060")
    if hit is None:
        return None
    slug = hit.name
    _DYNAMIC[slug] = hit.designation or "2060"
    try:
        return position(slug, when).ecl_lon
    except TimeWarpError:
        return None


def _raw_longitudes(when: datetime) -> tuple[dict[str, float], list[str]]:
    notes: list[str] = []
    out: dict[str, float] = {}
    year = when.astimezone(timezone.utc).year
    for name in PLANETS:
        if name == "pluto" and not 1800 <= year <= 2100:
            notes.append("Pluto omitted (fit is only valid 1800–2100)")
            continue
        out[name] = position(name, when).ecl_lon
    d = day_number(when)
    out["node"] = mean_node_lon(d)
    out["lilith"] = mean_lilith_lon(d)
    chi = _chiron_lon(when)
    if chi is None:
        notes.append("Chiron omitted (no SBDB elements)")
    else:
        out["chiron"] = chi
    return out, notes


def _with_houses(
    lon: float,
    cusps: list[float],
    *,
    whole: bool,
    asc: float,
    later: float | None,
) -> SignPos:
    h = house_of(lon, cusps, whole=whole, asc=asc)
    retro = False
    if later is not None:
        retro = wrap180(later - lon) < 0
    return sign_pos(lon, house=h, retrograde=retro)


def compute_chart(
    when: datetime,
    place: Place,
    *,
    houses: str = "placidus",
    sidereal: str | None = None,
    orb: float | None = None,
    natal: bool = False,
) -> Chart:
    if when.tzinfo is None:
        raise TimeWarpError("chart needs a timezone-aware datetime")
    sys_name = (houses or "placidus").strip().lower()
    if sys_name not in HOUSE_SYSTEMS:
        raise TimeWarpError(f"unknown house system {houses!r}; known: {', '.join(HOUSE_SYSTEMS)}")
    if orb is not None and not 0 < orb <= 15:
        raise TimeWarpError("--orb must be in 0..15 degrees")

    d = day_number(when)
    sun = sun_state(d)
    ramc = lst_deg(when, place.lon, sun)
    eps = sun.ecl
    ayan = 0.0
    ay_name = None
    frame = "tropical"
    if sidereal:
        ay_name = sidereal.strip().lower() or "lahiri"
        if ay_name not in AYANAMSA_NAMES:
            raise TimeWarpError(
                f"unknown ayanamsa {sidereal!r}; known: {', '.join(AYANAMSA_NAMES)}"
            )
        ayan = ayanamsa_deg(julian_day(when), ay_name)
        frame = "sidereal"

    trop_asc = asc_longitude(ramc, eps, place.lat)
    trop_mc = mc_longitude(ramc, eps)
    house_note = None
    used = sys_name
    try:
        if sys_name == "placidus":
            trop_cusps = placidus_cusps(ramc, eps, place.lat)
        elif sys_name == "equal":
            trop_cusps = equal_cusps(trop_asc)
        else:
            trop_cusps = whole_cusps(trop_asc)
    except TimeWarpError:
        trop_cusps = equal_cusps(trop_asc)
        used = "equal"
        house_note = "Placidus undefined at this latitude; using equal houses"

    asc = apply_frame(trop_asc, ayan)
    mc = apply_frame(trop_mc, ayan)
    dsc = rev(asc + 180.0)
    ic = rev(mc + 180.0)
    cusps = [0.0] + [apply_frame(trop_cusps[i], ayan) for i in range(1, 13)]
    whole = used == "whole"

    raw, notes = _raw_longitudes(when)
    later_when = when + timedelta(hours=6)
    raw_later, _ = _raw_longitudes(later_when)
    framed = {k: apply_frame(v, ayan) for k, v in raw.items()}
    framed_later = {k: apply_frame(v, ayan) for k, v in raw_later.items()}

    angles = {
        "asc": sign_pos(asc, house=1),
        "mc": _with_houses(mc, cusps, whole=whole, asc=asc, later=None),
        "dsc": sign_pos(dsc, house=7),
        "ic": _with_houses(ic, cusps, whole=whole, asc=asc, later=None),
    }
    bodies = {
        name: _with_houses(
            lon,
            cusps,
            whole=whole,
            asc=asc,
            later=framed_later.get(name),
        )
        for name, lon in framed.items()
    }
    sun_house = bodies["sun"].house or 1
    day_chart = sun_house >= 7
    lot_lons = hermetic_lots(asc, framed, day=day_chart)
    lots = {
        name: _with_houses(lon, cusps, whole=whole, asc=asc, later=None)
        for name, lon in lot_lons.items()
    }
    aspect_longs = {**{k: v.lon for k, v in bodies.items()}, "asc": asc, "mc": mc}
    aspect_later = {**framed_later, "asc": asc, "mc": mc}
    aspects = scan_aspects(aspect_longs, aspect_later, orb=orb)
    notes.append("Schlyter planets; mean node and mean Lilith; Chiron is SBDB two-body")
    if frame == "sidereal":
        notes.append(f"Ayanamsa {ay_name} {ayan:.4f}° (mean polynomial, not DE)")
    return Chart(
        when=when,
        place=place,
        frame=frame,
        ayanamsa_name=ay_name,
        ayanamsa_deg=ayan,
        house_system=used,
        house_note=house_note,
        cusps=cusps,
        angles=angles,
        bodies=bodies,
        lots=lots,
        aspects=aspects,
        notes=notes,
        natal=natal,
    )


def transit_aspects(natal: Chart, trans: Chart, *, orb: float | None = None) -> list[AspectHit]:
    nlong = {f"n.{k}": v.lon for k, v in natal.bodies.items()}
    tlong = {f"t.{k}": v.lon for k, v in trans.bodies.items()}
    hits: list[AspectHit] = []
    for tn, tl in tlong.items():
        for nn, nl in nlong.items():
            max_orb = max(default_orb(tn.split(".", 1)[1], orb), default_orb(nn.split(".", 1)[1], orb))
            delta = _sep(tl, nl)
            for kind, exact in ASPECTS:
                o = abs(delta - exact)
                if o <= max_orb:
                    hits.append(AspectHit(tn, nn, kind, exact, o, None))
                    break
    hits.sort(key=lambda h: (h.orb, h.a, h.b))
    return hits


def explain(chart: Chart, transits: list[AspectHit] | None = None) -> list[str]:
    lines: list[str] = []
    frame = chart.frame
    lines.append(
        f"{frame.capitalize()} chart, {chart.house_system} houses, "
        f"{chart.place.name}."
    )
    if chart.house_note:
        lines.append(chart.house_note + ".")
    asc = chart.angles["asc"]
    lines.append(f"ASC {asc.label()}, house 1.")
    sun = chart.bodies["sun"]
    moon = chart.bodies["moon"]
    sh = f", house {sun.house}" if sun.house else ""
    mh = f", house {moon.house}" if moon.house else ""
    lines.append(f"Sun in {sun.label()}{sh}.")
    lines.append(f"Moon in {moon.label()}{mh}.")
    if "fortune" in chart.lots:
        f = chart.lots["fortune"]
        fh = f", house {f.house}" if f.house else ""
        lines.append(f"Part of Fortune in {f.label()}{fh}.")
    src = transits if transits is not None else chart.aspects
    for hit in src[:12]:
        motion = ""
        if hit.applying is True:
            motion = ", applying"
        elif hit.applying is False:
            motion = ", separating"
        lines.append(
            f"{hit.a} {hit.kind} {hit.b} (orb {hit.orb:.1f}°{motion})."
        )
    return lines


def format_quiet(chart: Chart) -> str:
    asc = chart.angles["asc"]
    sun = chart.bodies["sun"]
    moon = chart.bodies["moon"]
    return (
        f"{chart.frame}  ASC {SIGN_ABBR[asc.sign_index]} {asc.degree:04.1f}  "
        f"Sun {SIGN_ABBR[sun.sign_index]} {sun.degree:04.1f}  "
        f"Moon {SIGN_ABBR[moon.sign_index]} {moon.degree:04.1f}"
    )
