"""Satellite passes from NORAD TLEs (SGP4), correlated with twilight and the moon."""

from __future__ import annotations

import math
import os
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from datetime import date, datetime, time, timedelta, timezone
from pathlib import Path
from zoneinfo import ZoneInfo

from timewarp.errors import TimeWarpError
from timewarp.iso import Instant, as_date, format_clock, format_instant
from timewarp.places import Place

try:
    from sgp4.api import Satrec, jday
except ImportError:  # pragma: no cover - exercised when sgp4 is missing
    Satrec = None  # type: ignore[misc, assignment]
    jday = None  # type: ignore[misc, assignment]

WGS84_A_KM = 6378.137
WGS84_E2 = 6.69437999014e-3
CELESTRAK_CATNR = "https://celestrak.org/NORAD/elements/gp.php?CATNR={id}&FORMAT=tle"
CELESTRAK_NAME = "https://celestrak.org/NORAD/elements/gp.php?NAME={name}&FORMAT=tle"
DEFAULT_SAT = "ISS"
DEFAULT_MIN_ELEV = 10.0
TLE_MAX_AGE_DAYS = 14
SCAN_STEP = timedelta(seconds=30)


def _need_sgp4() -> None:
    if Satrec is None:
        raise TimeWarpError(
            "satellite passes need the sgp4 package; from this tree: "
            ".venv/bin/pip install sgp4   (or: pip install sgp4)"
        )


def _gstime(jd: float) -> float:
    """Greenwich sidereal time (radians) from Julian date. Vallado."""
    tut1 = (jd - 2451545.0) / 36525.0
    temp = (
        -6.2e-6 * tut1 * tut1 * tut1
        + 0.093104 * tut1 * tut1
        + (876600.0 * 3600.0 + 8640184.812866) * tut1
        + 67310.54841
    )
    temp = math.fmod(temp * math.pi / 180.0 / 240.0, 2.0 * math.pi)
    if temp < 0.0:
        temp += 2.0 * math.pi
    return temp


def _geodetic_ecef(lat_deg: float, lon_deg: float, alt_km: float = 0.0) -> tuple[float, float, float]:
    lat = math.radians(lat_deg)
    lon = math.radians(lon_deg)
    slat, clat = math.sin(lat), math.cos(lat)
    slon, clon = math.sin(lon), math.cos(lon)
    n = WGS84_A_KM / math.sqrt(1.0 - WGS84_E2 * slat * slat)
    return (
        (n + alt_km) * clat * clon,
        (n + alt_km) * clat * slon,
        (n * (1.0 - WGS84_E2) + alt_km) * slat,
    )


def _teme_to_ecef(r: tuple[float, float, float], jd: float) -> tuple[float, float, float]:
    gmst = _gstime(jd)
    c, s = math.cos(gmst), math.sin(gmst)
    x, y, z = r
    return (x * c + y * s, -x * s + y * c, z)


def _look(
    r_ecef: tuple[float, float, float],
    obs: tuple[float, float, float],
    lat_deg: float,
    lon_deg: float,
) -> tuple[float, float, float]:
    """Return (altitude_deg, azimuth_deg, range_km). Azimuth 0=N, 90=E."""
    dx = r_ecef[0] - obs[0]
    dy = r_ecef[1] - obs[1]
    dz = r_ecef[2] - obs[2]
    lat = math.radians(lat_deg)
    lon = math.radians(lon_deg)
    slat, clat = math.sin(lat), math.cos(lat)
    slon, clon = math.sin(lon), math.cos(lon)
    south = slat * clon * dx + slat * slon * dy - clat * dz
    east = -slon * dx + clon * dy
    zen = clat * clon * dx + clat * slon * dy + slat * dz
    rng = math.hypot(south, east, zen)
    if rng < 1e-9:
        return 90.0, 0.0, 0.0
    alt = math.degrees(math.asin(max(-1.0, min(1.0, zen / rng))))
    az = math.degrees(math.atan2(east, -south)) % 360.0
    return alt, az, rng


def _sep_deg(alt1: float, az1: float, alt2: float, az2: float) -> float:
    a1, a2 = math.radians(alt1), math.radians(alt2)
    daz = math.radians(az1 - az2)
    c = math.sin(a1) * math.sin(a2) + math.cos(a1) * math.cos(a2) * math.cos(daz)
    return math.degrees(math.acos(max(-1.0, min(1.0, c))))


@dataclass(frozen=True)
class TleSat:
    name: str
    line1: str
    line2: str
    catalog: int
    rec: object
    epoch: datetime

    def to_dict(self) -> dict:
        return {
            "name": self.name,
            "catalog": self.catalog,
            "epoch": format_instant(self.epoch),
        }


@dataclass(frozen=True)
class Pass:
    sat: TleSat
    place: Place
    aos: datetime
    tca: datetime
    los: datetime
    max_alt_deg: float
    az_aos: float
    az_tca: float
    az_los: float
    twilight: str
    sun_alt_deg: float
    moon_alt_deg: float
    moon_illum: float
    moon_sep_deg: float

    def to_dict(self) -> dict:
        return {
            "sat": self.sat.name,
            "catalog": self.sat.catalog,
            "place": self.place.name,
            "aos": format_instant(self.aos),
            "tca": format_instant(self.tca),
            "los": format_instant(self.los),
            "max_alt_deg": round(self.max_alt_deg, 1),
            "az_aos_deg": round(self.az_aos, 1),
            "az_tca_deg": round(self.az_tca, 1),
            "az_los_deg": round(self.az_los, 1),
            "twilight": self.twilight,
            "sun_alt_deg": round(self.sun_alt_deg, 1),
            "moon_alt_deg": round(self.moon_alt_deg, 1),
            "moon_illumination": round(self.moon_illum, 3),
            "moon_sep_deg": round(self.moon_sep_deg, 1),
        }


def parse_tle_text(text: str) -> list[TleSat]:
    _need_sgp4()
    lines = [ln.strip() for ln in text.splitlines() if ln.strip()]
    out: list[TleSat] = []
    i = 0
    while i < len(lines):
        name = None
        if not lines[i].startswith("1 "):
            name = lines[i]
            i += 1
            if i >= len(lines):
                break
        if i + 1 >= len(lines) or not lines[i].startswith("1 ") or not lines[i + 1].startswith("2 "):
            raise TimeWarpError(f"malformed TLE near {lines[i][:20]!r}")
        l1, l2 = lines[i], lines[i + 1]
        i += 2
        try:
            rec = Satrec.twoline2rv(l1, l2)
        except Exception as exc:
            raise TimeWarpError(f"could not parse TLE for {name or l1[2:7]!r}: {exc}") from exc
        catalog = int(l1[2:7])
        if name is None:
            name = str(catalog)
        jd = rec.jdsatepoch + getattr(rec, "jdsatepochF", 0.0)
        epoch = datetime(2000, 1, 1, 12, tzinfo=timezone.utc) + timedelta(days=jd - 2451545.0)
        out.append(TleSat(name=name, line1=l1, line2=l2, catalog=catalog, rec=rec, epoch=epoch))
    if not out:
        raise TimeWarpError("no TLE records in that file")
    return out


def tle_dir() -> Path:
    env = os.environ.get("TIMEWARP_TLE_DIR")
    if env:
        return Path(env)
    xdg = os.environ.get("XDG_CACHE_HOME")
    root = Path(xdg) if xdg else Path.home() / ".cache"
    return root / "timewarp" / "tle"


def load_tle_file(path: Path) -> list[TleSat]:
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        raise TimeWarpError(f"could not read TLE {path}: {exc}") from exc
    return parse_tle_text(text)


def fetch_tle(query: str, *, timeout: float = 20.0) -> list[TleSat]:
    """Fetch a TLE from Celestrak. `query` is a name or catalog number."""
    _need_sgp4()
    q = query.strip()
    if q.isdigit():
        url = CELESTRAK_CATNR.format(id=q)
        cache_name = f"catnr-{q}.tle"
    else:
        url = CELESTRAK_NAME.format(name=urllib.parse.quote(q))
        cache_name = f"name-{q.lower().replace(' ', '_')}.tle"
    dest = tle_dir() / cache_name
    if dest.is_file():
        age = datetime.now(timezone.utc) - datetime.fromtimestamp(dest.stat().st_mtime, tz=timezone.utc)
        if age < timedelta(hours=24):
            return load_tle_file(dest)
    try:
        with urllib.request.urlopen(url, timeout=timeout) as resp:
            raw = resp.read()
    except (urllib.error.URLError, TimeoutError, OSError) as exc:
        if dest.is_file():
            return load_tle_file(dest)
        raise TimeWarpError(
            f"could not fetch TLE for {q!r} from Celestrak ({exc}). "
            f"Pass --tle FILE for offline use."
        ) from exc
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise TimeWarpError(f"Celestrak TLE for {q!r} was not text") from exc
    if "No GP data found" in text or not any(ln.startswith("1 ") for ln in text.splitlines()):
        raise TimeWarpError(f"Celestrak has no TLE for {q!r}")
    try:
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(text, encoding="utf-8")
    except OSError:
        pass
    return parse_tle_text(text)


def _sat_look(sat: TleSat, when: datetime, place: Place, obs: tuple[float, float, float]):
    utc = when.astimezone(timezone.utc)
    jd, fr = jday(
        utc.year,
        utc.month,
        utc.day,
        utc.hour,
        utc.minute,
        utc.second + utc.microsecond * 1e-6,
    )
    err, r, _v = sat.rec.sgp4(jd, fr)
    if err:
        return None
    ecef = _teme_to_ecef((r[0], r[1], r[2]), jd + fr)
    return _look(ecef, obs, place.lat, place.lon)


def _bisect_horizon(sat, t0, t1, place, obs, min_elev, want_up: bool) -> datetime:
    lo, hi = t0, t1
    for _ in range(32):
        if (hi - lo).total_seconds() < 0.5:
            break
        mid = lo + (hi - lo) / 2
        look = _sat_look(sat, mid, place, obs)
        alt = look[0] if look else -90.0
        above = alt >= min_elev
        if above == want_up:
            hi = mid
        else:
            lo = mid
    return (lo + (hi - lo) / 2).astimezone(ZoneInfo(place.tz)).replace(microsecond=0)


def twilight_label(sun_alt: float) -> str:
    if sun_alt >= -0.833:
        return "day"
    if sun_alt >= -6.0:
        return "civil"
    if sun_alt >= -12.0:
        return "nautical"
    if sun_alt >= -18.0:
        return "astronomical"
    return "night"


def _sky_at(when: datetime, place: Place, sat_alt: float, sat_az: float) -> tuple[str, float, float, float, float]:
    from timewarp.ephem import altitude_azimuth, position

    sun = position("sun", when)
    moon = position("moon", when)
    sun_alt, _sun_az = altitude_azimuth(sun, when, place.lat, place.lon)
    moon_alt, moon_az = altitude_azimuth(moon, when, place.lat, place.lon)
    illum = 0.0 if moon.phase is None else moon.phase
    sep = _sep_deg(sat_alt, sat_az, moon_alt, moon_az)
    return twilight_label(sun_alt), sun_alt, moon_alt, illum, sep


def select_sats(sats: list[TleSat], query: str | None, *, all_sats: bool) -> list[TleSat]:
    if all_sats:
        return sats
    if query is None or query.strip() == "":
        query = DEFAULT_SAT
    q = query.strip().lower()
    if q.isdigit():
        cat = int(q)
        picked = [s for s in sats if s.catalog == cat]
    else:
        picked = [s for s in sats if q in s.name.lower()]
        if not picked and q in {"iss", "zarya", "station"}:
            picked = [s for s in sats if s.catalog == 25544 or "iss" in s.name.lower()]
    if not picked:
        names = ", ".join(s.name for s in sats[:12])
        raise TimeWarpError(f"no satellite matching {query!r} in TLE set (have: {names})")
    return picked


def passes_for_day(
    sat: TleSat,
    day: Instant,
    place: Place,
    *,
    min_elev: float = DEFAULT_MIN_ELEV,
) -> list[Pass]:
    _need_sgp4()
    tz = ZoneInfo(place.tz)
    civil = as_date(day)
    start = datetime.combine(civil, time.min, tzinfo=tz)
    end = start + timedelta(days=1)
    obs = _geodetic_ecef(place.lat, place.lon, 0.0)

    samples: list[tuple[datetime, float, float]] = []
    t = start
    guard = 0
    while t <= end:
        look = _sat_look(sat, t, place, obs)
        if look:
            samples.append((t, look[0], look[1]))
        t += SCAN_STEP
        guard += 1
        if guard > 4000:
            raise TimeWarpError("pass sampler exceeded the local day (internal error)")

    passes: list[Pass] = []
    i = 0
    n = len(samples)
    while i < n:
        if samples[i][1] < min_elev:
            i += 1
            continue
        j = i
        while j + 1 < n and samples[j + 1][1] >= min_elev:
            j += 1
        window = samples[i : j + 1]
        peak = max(window, key=lambda row: row[1])
        t_before = samples[i - 1][0] if i > 0 else start
        t_after = samples[j + 1][0] if j + 1 < n else end
        aos = _bisect_horizon(sat, t_before, window[0][0], place, obs, min_elev, True)
        los = _bisect_horizon(sat, window[-1][0], t_after, place, obs, min_elev, False)
        tca = peak[0].astimezone(tz).replace(microsecond=0)
        look_aos = _sat_look(sat, aos, place, obs)
        look_tca = _sat_look(sat, tca, place, obs)
        look_los = _sat_look(sat, los, place, obs)
        if not (look_aos and look_tca and look_los):
            i = j + 1
            continue
        twilight, sun_alt, moon_alt, illum, sep = _sky_at(tca, place, look_tca[0], look_tca[1])
        if start <= tca < end:
            passes.append(
                Pass(
                    sat=sat,
                    place=place,
                    aos=aos,
                    tca=tca,
                    los=los,
                    max_alt_deg=look_tca[0],
                    az_aos=look_aos[1],
                    az_tca=look_tca[1],
                    az_los=look_los[1],
                    twilight=twilight,
                    sun_alt_deg=sun_alt,
                    moon_alt_deg=moon_alt,
                    moon_illum=illum,
                    moon_sep_deg=sep,
                )
            )
        i = j + 1
    return passes


def tle_freshness_note(sats: list[TleSat], day: Instant) -> str | None:
    civil = as_date(day)
    worst = 0
    for s in sats:
        worst = max(worst, abs((s.epoch.date() - civil).days))
    if worst > TLE_MAX_AGE_DAYS:
        return (
            f"TLE epoch is {worst} days from {civil.isoformat()}; "
            f"SGP4 accuracy falls off after about {TLE_MAX_AGE_DAYS} days"
        )
    return None
