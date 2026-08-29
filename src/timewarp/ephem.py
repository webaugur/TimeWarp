"""Low-precision solar-system positions (Schlyter / van Flandern–Pulkkinen).

Planets: about 1–2 arcminutes in the 20th–21st centuries — enough for rise/set.
Asteroids and comets: two-body Keplerian from JPL SBDB osculating elements
(cached; frozen table if SBDB is unreachable). Planetary moons: circular
orbits about the parent. Not a full JPL numerical ephemeris.
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from datetime import datetime, timezone

from timewarp.errors import TimeWarpError

BODIES = (
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
ASTEROIDS = ("ceres", "pallas", "juno", "vesta", "hygiea", "eros")
COMETS = ("halley", "encke", "tempel1", "67p")
MOONS = ("io", "europa", "ganymede", "callisto", "titan", "triton", "phobos", "deimos")
ALL_BODIES = BODIES + ASTEROIDS + COMETS + MOONS

# IAU/astronomical symbols (Unicode Miscellaneous Symbols).
SYMBOLS = {
    "sun": "☉",
    "moon": "☾",
    "mercury": "☿",
    "venus": "♀",
    "mars": "♂",
    "jupiter": "♃",
    "saturn": "♄",
    "uranus": "♅",
    "neptune": "♆",
    "pluto": "♇",
    "ceres": "⚳",
    "pallas": "⚴",
    "juno": "⚵",
    "vesta": "⚶",
    "hygiea": "⚙",
    "eros": "♡",
    "halley": "☄",
    "encke": "☄",
    "tempel1": "☄",
    "67p": "☄",
    "io": "I",
    "europa": "E",
    "ganymede": "G",
    "callisto": "C",
    "titan": "T",
    "triton": "t",
    "phobos": "p",
    "deimos": "d",
}

# Approximate visual colors for a color terminal (not albedo-accurate).
SYMBOL_RGB = {
    "sun": (255, 204, 0),
    "moon": (210, 215, 230),
    "mercury": (176, 176, 186),
    "venus": (255, 214, 90),
    "mars": (220, 68, 40),
    "jupiter": (232, 164, 72),
    "saturn": (214, 196, 118),
    "uranus": (110, 220, 228),
    "neptune": (56, 96, 230),
    "pluto": (186, 150, 118),
    "ceres": (168, 164, 156),
    "pallas": (160, 150, 140),
    "juno": (176, 156, 132),
    "vesta": (196, 176, 140),
    "hygiea": (150, 160, 150),
    "eros": (200, 120, 100),
    "halley": (180, 220, 255),
    "encke": (180, 220, 255),
    "tempel1": (180, 220, 255),
    "67p": (180, 220, 255),
    "io": (255, 200, 80),
    "europa": (220, 210, 190),
    "ganymede": (186, 170, 140),
    "callisto": (120, 110, 100),
    "titan": (210, 160, 80),
    "triton": (180, 190, 200),
    "phobos": (160, 130, 110),
    "deimos": (150, 130, 110),
}

_RESET = "\033[0m"


def body_symbol(name: str) -> str | None:
    return SYMBOLS.get(name.strip().lower())


def _color_symbol(symbol: str, rgb: tuple[int, int, int]) -> str:
    r, g, b = rgb
    return f"\033[1;38;2;{r};{g};{b}m{symbol}{_RESET}"


def format_body(name: str, *, color: bool = False, width: int = 0) -> str:
    """Plain '☉ sun' label. If color=True, tint only the symbol (ANSI), then pad to width."""
    key = name.strip().lower()
    symbol = SYMBOLS.get(key)
    plain = f"{symbol} {key}" if symbol else key
    if width > 0:
        plain = f"{plain:{width}}"
    if not color or not symbol or key not in SYMBOL_RGB:
        return plain
    return plain.replace(symbol, _color_symbol(symbol, SYMBOL_RGB[key]), 1)

# Apparent equatorial diameter at 1 AU, arcseconds (Schlyter).
DIAMETER_ARCSEC = {
    "mercury": 6.74,
    "venus": 16.92,
    "mars": 9.36,
    "jupiter": 196.94,
    "saturn": 165.6,
    "uranus": 65.8,
    "neptune": 62.2,
    "pluto": 3.0,
    "sun": 1919.26,
}

# N,i,w,a,e,M as (offset, rate per day). a in AU except moon (Earth radii).
_PLANET = {
    "mercury": {
        "N": (48.3313, 3.24587e-5),
        "i": (7.0047, 5.00e-8),
        "w": (29.1241, 1.01444e-5),
        "a": (0.387098, 0.0),
        "e": (0.205635, 5.59e-10),
        "M": (168.6562, 4.0923344368),
    },
    "venus": {
        "N": (76.6799, 2.46590e-5),
        "i": (3.3946, 2.75e-8),
        "w": (54.8910, 1.38374e-5),
        "a": (0.723330, 0.0),
        "e": (0.006773, -1.302e-9),
        "M": (48.0052, 1.6021302244),
    },
    "mars": {
        "N": (49.5574, 2.11081e-5),
        "i": (1.8497, -1.78e-8),
        "w": (286.5016, 2.92961e-5),
        "a": (1.523688, 0.0),
        "e": (0.093405, 2.516e-9),
        "M": (18.6021, 0.5240207766),
    },
    "jupiter": {
        "N": (100.4542, 2.76854e-5),
        "i": (1.3030, -1.557e-7),
        "w": (273.8777, 1.64505e-5),
        "a": (5.20256, 0.0),
        "e": (0.048498, 4.469e-9),
        "M": (19.8950, 0.0830853001),
    },
    "saturn": {
        "N": (113.6634, 2.38980e-5),
        "i": (2.4886, -1.081e-7),
        "w": (339.3939, 2.97661e-5),
        "a": (9.55475, 0.0),
        "e": (0.055546, -9.499e-9),
        "M": (316.9670, 0.0334442282),
    },
    "uranus": {
        "N": (74.0005, 1.3978e-5),
        "i": (0.7733, 1.9e-8),
        "w": (96.6612, 3.0565e-5),
        "a": (19.18171, -1.55e-8),
        "e": (0.047318, 7.45e-9),
        "M": (142.5905, 0.011725806),
    },
    "neptune": {
        "N": (131.7806, 3.0173e-5),
        "i": (1.7700, -2.55e-7),
        "w": (272.8461, -6.027e-6),
        "a": (30.05826, 3.313e-8),
        "e": (0.008606, 2.15e-9),
        "M": (260.2471, 0.005995147),
    },
}

# Frozen Keplerian heliocentric fallback if JPL SBDB is unreachable.
# a AU, angles deg, n deg/day, M0 at d=0 (2000 Jan 0.0 TT≈UT); comets M=0 at d_peri.
_MINOR = {
    "ceres": {"a": 2.766, "e": 0.0758, "i": 10.59, "N": 80.31, "w": 73.47, "M0": 95.99, "n": 0.214023},
    "pallas": {"a": 2.772, "e": 0.2306, "i": 34.85, "N": 173.09, "w": 310.05, "M0": 144.96, "n": 0.213737},
    "juno": {"a": 2.670, "e": 0.2562, "i": 12.99, "N": 169.87, "w": 247.93, "M0": 257.69, "n": 0.2258},
    "vesta": {"a": 2.362, "e": 0.0887, "i": 7.14, "N": 103.81, "w": 151.74, "M0": 169.42, "n": 0.271439},
    "hygiea": {"a": 3.142, "e": 0.1116, "i": 3.83, "N": 283.20, "w": 312.32, "M0": 241.69, "n": 0.1767},
    "eros": {"a": 1.458, "e": 0.2229, "i": 10.83, "N": 304.40, "w": 178.82, "M0": 320.60, "n": 0.559},
    # Periodic comets: M=0 at perihelion (d_peri is Schlyter d).
    "halley": {"a": 17.834, "e": 0.9671, "i": 162.26, "N": 58.42, "w": 111.33, "M0": 0.0, "n": 0.01296, "d_peri": -5072.56},
    "encke": {"a": 2.215, "e": 0.8483, "i": 11.78, "N": 334.57, "w": 186.55, "M0": 0.0, "n": 0.2981, "d_peri": 91.0},
    "tempel1": {"a": 3.122, "e": 0.509, "i": 10.47, "N": 68.76, "w": 179.19, "M0": 0.0, "n": 0.1786, "d_peri": 2005.0},
    "67p": {"a": 3.463, "e": 0.641, "i": 7.04, "N": 50.15, "w": 12.80, "M0": 0.0, "n": 0.1529, "d_peri": 1670.0},
}

# Circular about parent. a_km, n deg/day, L0 at d=0, i to ecliptic (deg).
_MOON = {
    "io": {"parent": "jupiter", "a_km": 421800, "n": 203.4889538, "L0": 106.08, "i": 3.1},
    "europa": {"parent": "jupiter", "a_km": 671100, "n": 101.3747246, "L0": 175.73, "i": 3.4},
    "ganymede": {"parent": "jupiter", "a_km": 1070400, "n": 50.3176092, "L0": 120.56, "i": 3.1},
    "callisto": {"parent": "jupiter", "a_km": 1882700, "n": 21.5710715, "L0": 84.44, "i": 3.0},
    "titan": {"parent": "saturn", "a_km": 1221870, "n": 22.577015, "L0": 261.6, "i": 0.3},
    "triton": {"parent": "neptune", "a_km": 354800, "n": -61.2573, "L0": 70.0, "i": 157.0},
    "phobos": {"parent": "mars", "a_km": 9376, "n": 1128.844, "L0": 40.0, "i": 1.1},
    "deimos": {"parent": "mars", "a_km": 23463, "n": 285.162, "L0": 80.0, "i": 1.8},
}
_KM_PER_AU = 149597870.7


def sind(x: float) -> float:
    return math.sin(math.radians(x))


def cosd(x: float) -> float:
    return math.cos(math.radians(x))


def tand(x: float) -> float:
    return math.tan(math.radians(x))


def asind(x: float) -> float:
    return math.degrees(math.asin(max(-1.0, min(1.0, x))))


def acosd(x: float) -> float:
    return math.degrees(math.acos(max(-1.0, min(1.0, x))))


def atan2d(y: float, x: float) -> float:
    return math.degrees(math.atan2(y, x))


def rev(x: float) -> float:
    return x % 360.0


def julian_day(dt: datetime) -> float:
    """Julian Date of a timezone-aware instant (converted to UTC)."""
    if dt.tzinfo is None:
        raise TimeWarpError("ephemeris needs a timezone-aware datetime")
    utc = dt.astimezone(timezone.utc)
    y, m, day = utc.year, utc.month, utc.day
    frac = (utc.hour + utc.minute / 60.0 + utc.second / 3600.0 + utc.microsecond / 3.6e9) / 24.0
    if m <= 2:
        y -= 1
        m += 12
    a = y // 100
    b = 2 - a + a // 4
    jd0 = int(365.25 * (y + 4716)) + int(30.6001 * (m + 1)) + day + b - 1524.5
    return jd0 + frac


def day_number(dt: datetime) -> float:
    """Days from 2000 Jan 0.0 UT (Schlyter d)."""
    return julian_day(dt) - 2451543.5


def _pair(base_rate: tuple[float, float], d: float) -> float:
    return base_rate[0] + base_rate[1] * d


def eccentric_anomaly(M_deg: float, e: float) -> float:
    M = math.radians(rev(M_deg))
    E = M + e * math.sin(M) * (1.0 + e * math.cos(M))
    if e > 0.05:
        for _ in range(30):
            denom = 1.0 - e * math.cos(E)
            if abs(denom) < 1e-12:
                raise TimeWarpError("Kepler iteration hit a singular eccentric anomaly")
            E1 = E - (E - e * math.sin(E) - M) / denom
            if abs(E1 - E) < 1e-10:
                E = E1
                break
            E = E1
        else:
            raise TimeWarpError("Kepler equation did not converge")
    return math.degrees(E)


def _true_anomaly_r(a: float, e: float, E_deg: float) -> tuple[float, float]:
    E = math.radians(E_deg)
    xv = a * (math.cos(E) - e)
    yv = a * (math.sqrt(max(0.0, 1.0 - e * e)) * math.sin(E))
    return atan2d(yv, xv), math.hypot(xv, yv)


def _ecliptic_xyz(r: float, N: float, i: float, w: float, v: float) -> tuple[float, float, float]:
    vw = v + w
    xh = r * (cosd(N) * cosd(vw) - sind(N) * sind(vw) * cosd(i))
    yh = r * (sind(N) * cosd(vw) + cosd(N) * sind(vw) * cosd(i))
    zh = r * (sind(vw) * sind(i))
    return xh, yh, zh


def _lon_lat_r(x: float, y: float, z: float) -> tuple[float, float, float]:
    r = math.sqrt(x * x + y * y + z * z)
    lon = rev(atan2d(y, x))
    lat = atan2d(z, math.hypot(x, y))
    return lon, lat, r


def _equatorial(xg: float, yg: float, zg: float, ecl: float) -> tuple[float, float, float]:
    xe = xg
    ye = yg * cosd(ecl) - zg * sind(ecl)
    ze = yg * sind(ecl) + zg * cosd(ecl)
    ra = rev(atan2d(ye, xe))
    dec = atan2d(ze, math.hypot(xe, ye))
    dist = math.sqrt(xe * xe + ye * ye + ze * ze)
    return ra, dec, dist


@dataclass(frozen=True)
class SkyPos:
    body: str
    ra_deg: float
    dec_deg: float
    distance: float
    distance_unit: str
    ecl_lon: float
    ecl_lat: float
    elongation_deg: float | None
    phase: float | None
    magnitude: float | None
    parallax_deg: float
    semidiameter_deg: float
    heliocentric_au: float | None


@dataclass(frozen=True)
class _Sun:
    d: float
    ecl: float
    M: float
    w: float
    L: float
    r: float
    lon: float
    xs: float
    ys: float
    ra: float
    dec: float


def sun_state(d: float) -> _Sun:
    w = 282.9404 + 4.70935e-5 * d
    e = 0.016709 - 1.151e-9 * d
    M = rev(356.0470 + 0.9856002585 * d)
    ecl = 23.4393 - 3.563e-7 * d
    L = rev(w + M)
    E = M + (180.0 / math.pi) * e * sind(M) * (1.0 + e * cosd(M))
    xv = cosd(E) - e
    yv = math.sqrt(max(0.0, 1.0 - e * e)) * sind(E)
    v = atan2d(yv, xv)
    r = math.hypot(xv, yv)
    lon = rev(v + w)
    xs = r * cosd(lon)
    ys = r * sind(lon)
    ra, dec, _ = _equatorial(xs, ys, 0.0, ecl)
    return _Sun(d, ecl, M, w, L, r, lon, xs, ys, ra, dec)


def gmst_deg(d: float, ut_hours: float, sun: _Sun | None = None) -> float:
    s = sun if sun is not None else sun_state(d)
    gmst0 = rev(s.L + 180.0)
    return rev(gmst0 + ut_hours * 15.0)


def lst_deg(dt: datetime, lon: float, sun: _Sun | None = None) -> float:
    utc = dt.astimezone(timezone.utc)
    ut = utc.hour + utc.minute / 60.0 + utc.second / 3600.0 + utc.microsecond / 3.6e9
    d = day_number(dt)
    return rev(gmst_deg(d, ut, sun) + lon)


def _moon_raw(d: float, sun: _Sun) -> tuple[float, float, float]:
    N = rev(125.1228 - 0.0529538083 * d)
    i = 5.1454
    w = rev(318.0634 + 0.1643573223 * d)
    a = 60.2666
    e = 0.054900
    M = rev(115.3654 + 13.0649929509 * d)
    E = eccentric_anomaly(M, e)
    v, r = _true_anomaly_r(a, e, E)
    xh, yh, zh = _ecliptic_xyz(r, N, i, w, v)
    lon, lat, r = _lon_lat_r(xh, yh, zh)

    Ms, Mm = sun.M, M
    Ls = sun.L
    Lm = rev(N + w + M)
    D = rev(Lm - Ls)
    F = rev(Lm - N)

    lon += (
        -1.274 * sind(Mm - 2 * D)
        + 0.658 * sind(2 * D)
        - 0.186 * sind(Ms)
        - 0.059 * sind(2 * Mm - 2 * D)
        - 0.057 * sind(Mm - 2 * D + Ms)
        + 0.053 * sind(Mm + 2 * D)
        + 0.046 * sind(2 * D - Ms)
        + 0.041 * sind(Mm - Ms)
        - 0.035 * sind(D)
        - 0.031 * sind(Mm + Ms)
        - 0.015 * sind(2 * F - 2 * D)
        + 0.011 * sind(Mm - 4 * D)
    )
    lat += (
        -0.173 * sind(F - 2 * D)
        - 0.055 * sind(Mm - F - 2 * D)
        - 0.046 * sind(Mm + F - 2 * D)
        + 0.033 * sind(F + 2 * D)
        + 0.017 * sind(2 * Mm + F)
    )
    r += -0.58 * cosd(Mm - 2 * D) - 0.46 * cosd(2 * D)
    return rev(lon), lat, r


def _planet_heliocentric(name: str, d: float) -> tuple[float, float, float, float]:
    el = _PLANET[name]
    N = rev(_pair(el["N"], d))
    i = _pair(el["i"], d)
    w = rev(_pair(el["w"], d))
    a = _pair(el["a"], d)
    e = _pair(el["e"], d)
    M = rev(_pair(el["M"], d))
    E = eccentric_anomaly(M, e)
    v, r = _true_anomaly_r(a, e, E)
    xh, yh, zh = _ecliptic_xyz(r, N, i, w, v)
    lon, lat, r = _lon_lat_r(xh, yh, zh)

    if name in {"jupiter", "saturn", "uranus"}:
        Mj = rev(_pair(_PLANET["jupiter"]["M"], d))
        Ms = rev(_pair(_PLANET["saturn"]["M"], d))
        Mu = rev(_pair(_PLANET["uranus"]["M"], d))
        if name == "jupiter":
            lon += (
                -0.332 * sind(2 * Mj - 5 * Ms - 67.6)
                - 0.056 * sind(2 * Mj - 2 * Ms + 21)
                + 0.042 * sind(3 * Mj - 5 * Ms + 21)
                - 0.036 * sind(Mj - 2 * Ms)
                + 0.022 * cosd(Mj - Ms)
                + 0.023 * sind(2 * Mj - 3 * Ms + 52)
                - 0.016 * sind(Mj - 5 * Ms - 69)
            )
        elif name == "saturn":
            lon += (
                +0.812 * sind(2 * Mj - 5 * Ms - 67.6)
                - 0.229 * cosd(2 * Mj - 4 * Ms - 2)
                + 0.119 * sind(Mj - 2 * Ms - 3)
                + 0.046 * sind(2 * Mj - 6 * Ms - 69)
                + 0.014 * sind(Mj - 3 * Ms + 32)
            )
            lat += -0.020 * cosd(2 * Mj - 4 * Ms - 2) + 0.018 * sind(2 * Mj - 6 * Ms - 49)
        else:
            lon += (
                +0.040 * sind(Ms - 2 * Mu + 6)
                + 0.035 * sind(Ms - 3 * Mu + 33)
                - 0.015 * sind(Mj - Mu + 20)
            )
        lon = rev(lon)
    return lon, lat, r, M


def _pluto(d: float) -> tuple[float, float, float]:
    S = 50.03 + 0.033459652 * d
    P = 238.95 + 0.003968789 * d
    lon = (
        238.9508
        + 0.00400703 * d
        - 19.799 * sind(P)
        + 19.848 * cosd(P)
        + 0.897 * sind(2 * P)
        - 4.956 * cosd(2 * P)
        + 0.610 * sind(3 * P)
        + 1.211 * cosd(3 * P)
        - 0.341 * sind(4 * P)
        - 0.190 * cosd(4 * P)
        + 0.128 * sind(5 * P)
        - 0.034 * cosd(5 * P)
        - 0.038 * sind(6 * P)
        + 0.031 * cosd(6 * P)
        + 0.020 * sind(S - P)
        - 0.010 * cosd(S - P)
    )
    lat = (
        -3.9082
        - 5.453 * sind(P)
        - 14.975 * cosd(P)
        + 3.527 * sind(2 * P)
        + 1.673 * cosd(2 * P)
        - 1.051 * sind(3 * P)
        + 0.328 * cosd(3 * P)
        + 0.179 * sind(4 * P)
        - 0.292 * cosd(4 * P)
        + 0.019 * sind(5 * P)
        + 0.100 * cosd(5 * P)
        - 0.031 * sind(6 * P)
        - 0.026 * cosd(6 * P)
        + 0.011 * cosd(S - P)
    )
    r = (
        40.72
        + 6.68 * sind(P)
        + 6.90 * cosd(P)
        - 1.18 * sind(2 * P)
        - 0.03 * cosd(2 * P)
        + 0.15 * sind(3 * P)
        - 0.14 * cosd(3 * P)
    )
    return rev(lon), lat, r


def _kepler_heliocentric(name: str, d: float) -> tuple[float, float, float]:
    from timewarp.jpl import load_elements

    sb = load_elements(name)
    if sb is not None:
        e = min(sb.e, 0.99)
        m = rev(sb.M0 + sb.n * (d - sb.d_epoch))
        ecc = eccentric_anomaly(m, e)
        v, r = _true_anomaly_r(sb.a, e, ecc)
        return _ecliptic_xyz(r, sb.N, sb.i, sb.w, v)
    el = _MINOR.get(name)
    if el is None:
        raise TimeWarpError(f"no Kepler elements for {name}")
    e = min(el["e"], 0.99)
    m = rev(el["M0"] + el["n"] * (d - el.get("d_peri", 0.0)))
    ecc = eccentric_anomaly(m, e)
    v, r = _true_anomaly_r(el["a"], e, ecc)
    return _ecliptic_xyz(r, el["N"], el["i"], el["w"], v)


def _moon_heliocentric(name: str, d: float) -> tuple[float, float, float]:
    el = _MOON[name]
    parent = el["parent"]
    if parent == "pluto":
        plon, plat, pr = _pluto(d)
    else:
        plon, plat, pr, _m = _planet_heliocentric(parent, d)
    phx = pr * cosd(plon) * cosd(plat)
    phy = pr * sind(plon) * cosd(plat)
    phz = pr * sind(plat)
    a = el["a_km"] / _KM_PER_AU
    arg = rev(el["L0"] + el["n"] * d)
    mx, my, mz = _ecliptic_xyz(a, 0.0, el["i"], 0.0, arg)
    return phx + mx, phy + my, phz + mz


def _magnitude(name: str, r: float, R: float, fv: float, sun: _Sun, ecl_lon: float, ecl_lat: float, d: float) -> float | None:
    logterm = 5.0 * math.log10(max(1e-12, r * R))
    if name == "mercury":
        return -0.36 + logterm + 0.027 * fv + 2.2e-13 * fv**6
    if name == "venus":
        return -4.34 + logterm + 0.013 * fv + 4.2e-7 * fv**3
    if name == "mars":
        return -1.51 + logterm + 0.016 * fv
    if name == "jupiter":
        return -9.25 + logterm + 0.014 * fv
    if name == "saturn":
        ir = 28.06
        nr = 169.51 + 3.82e-5 * d
        b = asind(sind(ecl_lat) * cosd(ir) - cosd(ecl_lat) * sind(ir) * sind(ecl_lon - nr))
        ring = -2.6 * sind(abs(b)) + 1.2 * (sind(b) ** 2)
        return -9.0 + logterm + 0.044 * fv + ring
    if name == "uranus":
        return -7.15 + logterm + 0.001 * fv
    if name == "neptune":
        return -6.90 + logterm + 0.001 * fv
    if name == "moon":
        # r Sun AU, R Moon Earth-radii
        return -21.62 + 5.0 * math.log10(max(1e-12, sun.r * R)) + 0.026 * fv + 4.0e-9 * fv**4
    return None


def position(body: str, dt: datetime) -> SkyPos:
    name = normalize_body(body)
    if name == "pluto":
        year = dt.astimezone(timezone.utc).year
        if not 1800 <= year <= 2100:
            raise TimeWarpError("Pluto fit is only valid 1800–2100")

    d = day_number(dt)
    sun = sun_state(d)

    if name == "sun":
        sd = (DIAMETER_ARCSEC["sun"] / 2.0) / 3600.0 / sun.r
        return SkyPos(
            body="sun",
            ra_deg=sun.ra,
            dec_deg=sun.dec,
            distance=sun.r,
            distance_unit="AU",
            ecl_lon=sun.lon,
            ecl_lat=0.0,
            elongation_deg=0.0,
            phase=1.0,
            magnitude=-26.74,
            parallax_deg=(8.794 / 3600.0) / sun.r,
            semidiameter_deg=sd,
            heliocentric_au=0.0,
        )

    if name == "moon":
        lon, lat, r = _moon_raw(d, sun)
        xg = r * cosd(lon) * cosd(lat)
        yg = r * sind(lon) * cosd(lat)
        zg = r * sind(lat)
        ra, dec, dist = _equatorial(xg, yg, zg, sun.ecl)
        elong = acosd(cosd(sun.lon - lon) * cosd(lat))
        fv = 180.0 - elong
        phase = (1.0 + cosd(fv)) / 2.0
        mpar = asind(1.0 / max(r, 1e-6))
        msd = (1873.7 * 60.0 / 2.0) / 3600.0 / r  # semi-diameter, degrees
        mag = _magnitude("moon", sun.r, r, fv, sun, lon, lat, d)
        return SkyPos(
            body="moon",
            ra_deg=ra,
            dec_deg=dec,
            distance=dist,
            distance_unit="Earth radii",
            ecl_lon=lon,
            ecl_lat=lat,
            elongation_deg=elong,
            phase=phase,
            magnitude=mag,
            parallax_deg=mpar,
            semidiameter_deg=msd,
            heliocentric_au=sun.r,
        )

    if name in ASTEROIDS or name in COMETS:
        xh, yh, zh = _kepler_heliocentric(name, d)
        rhel = math.hypot(xh, yh, zh)
        lon, lat, _rh = _lon_lat_r(xh, yh, zh)
    elif name in _MOON:
        xh, yh, zh = _moon_heliocentric(name, d)
        rhel = math.hypot(xh, yh, zh)
        lon, lat, _rh = _lon_lat_r(xh, yh, zh)
    elif name == "pluto":
        lon, lat, r = _pluto(d)
        xh = r * cosd(lon) * cosd(lat)
        yh = r * sind(lon) * cosd(lat)
        zh = r * sind(lat)
        rhel = r
    else:
        lon, lat, r, _M = _planet_heliocentric(name, d)
        xh = r * cosd(lon) * cosd(lat)
        yh = r * sind(lon) * cosd(lat)
        zh = r * sind(lat)
        rhel = r

    # heliocentric ecliptic → geocentric (sun.xs, sun.ys are ecliptic)
    xg = xh + sun.xs
    yg = yh + sun.ys
    zg = zh
    ra, dec, dist = _equatorial(xg, yg, zg, sun.ecl)
    s, Rgeo, rh = sun.r, dist, rhel
    elong = acosd((s * s + Rgeo * Rgeo - rh * rh) / (2.0 * s * Rgeo)) if s * Rgeo else None
    fv = acosd((rh * rh + Rgeo * Rgeo - s * s) / (2.0 * rh * Rgeo)) if rh * Rgeo else 0.0
    phase = (1.0 + cosd(fv)) / 2.0
    ppar = (8.794 / 3600.0) / max(Rgeo, 1e-9)
    d0 = DIAMETER_ARCSEC.get(name, 0.0)
    sd = (d0 / 2.0) / 3600.0 / max(Rgeo, 1e-9)
    mag = _magnitude(name, rh, Rgeo, fv, sun, lon, lat, d)
    return SkyPos(
        body=name,
        ra_deg=ra,
        dec_deg=dec,
        distance=dist,
        distance_unit="AU",
        ecl_lon=rev(atan2d(yg, xg)),
        ecl_lat=atan2d(zg, math.hypot(xg, yg)),
        elongation_deg=elong,
        phase=phase,
        magnitude=mag,
        parallax_deg=ppar,
        semidiameter_deg=sd,
        heliocentric_au=rh,
    )


def altitude_azimuth(pos: SkyPos, dt: datetime, lat: float, lon: float) -> tuple[float, float]:
    """Geocentric altitude (degrees) and azimuth (0=N, 90=E). Moon still needs parallax subtract."""
    d = day_number(dt)
    lst = lst_deg(dt, lon, sun_state(d))
    ha = rev(lst - pos.ra_deg)
    if ha > 180.0:
        ha -= 360.0
    x = cosd(ha) * cosd(pos.dec_deg)
    y = sind(ha) * cosd(pos.dec_deg)
    z = sind(pos.dec_deg)
    xhor = x * sind(lat) - z * cosd(lat)
    yhor = y
    zhor = x * cosd(lat) + z * sind(lat)
    az = rev(atan2d(yhor, xhor) + 180.0)
    alt = asind(zhor)
    if pos.body == "moon":
        alt -= pos.parallax_deg * cosd(alt)
    return alt, az


def normalize_body(name: str) -> str:
    key = name.strip().lower().replace(" ", "")
    aliases = {
        "sol": "sun",
        "luna": "moon",
        "terra": "earth",
        "1p": "halley",
        "1p/halley": "halley",
        "2p": "encke",
        "2p/encke": "encke",
        "9p": "tempel1",
        "tempel": "tempel1",
        "tempel-1": "tempel1",
        "67p/churyumov-gerasimenko": "67p",
        "churyumov": "67p",
        "1ceres": "ceres",
    }
    key = aliases.get(key, key)
    if key == "earth":
        raise TimeWarpError("Earth is the observer, not a rise/set body")
    if key not in ALL_BODIES:
        raise TimeWarpError(f"unknown body {name!r}; known: {', '.join(ALL_BODIES)}")
    return key
