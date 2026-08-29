"""JPL Horizons osculating elements for named planetary moons.

GET https://ssd.jpl.nasa.gov/api/horizons.api (EPHEM_TYPE=ELEMENTS).
Cache: ~/.cache/timewarp/horizons/{name}.json (7 days). TIMEWARP_HORIZONS_DIR overrides.
Circular _MOON table in ephem is the offline fallback.
"""

from __future__ import annotations

import csv
import io
import json
import os
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timedelta, timezone
from pathlib import Path

from timewarp.errors import TimeWarpError
from timewarp.jpl import KeplerElements, _SCHLYTER_JD0, _as_float, _rev

HORIZONS_URL = "https://ssd.jpl.nasa.gov/api/horizons.api"
CACHE_TTL = timedelta(days=7)
_TIMEOUT = 40.0

# Horizons major-body IDs; CENTER is the parent body center (500@NAIF).
HORIZONS_MOONS = {
    "io": {"command": "501", "center": "500@599", "parent": "jupiter"},
    "europa": {"command": "502", "center": "500@599", "parent": "jupiter"},
    "ganymede": {"command": "503", "center": "500@599", "parent": "jupiter"},
    "callisto": {"command": "504", "center": "500@599", "parent": "jupiter"},
    "titan": {"command": "606", "center": "500@699", "parent": "saturn"},
    "triton": {"command": "801", "center": "500@899", "parent": "neptune"},
    "phobos": {"command": "401", "center": "500@499", "parent": "mars"},
    "deimos": {"command": "402", "center": "500@499", "parent": "mars"},
}

_MEMO: dict[str, KeplerElements | None] = {}


def horizons_cache_dir() -> Path:
    env = os.environ.get("TIMEWARP_HORIZONS_DIR")
    if env:
        return Path(env)
    xdg = os.environ.get("XDG_CACHE_HOME")
    root = Path(xdg) if xdg else Path.home() / ".cache"
    return root / "timewarp" / "horizons"


def parent_of(name: str) -> str | None:
    row = HORIZONS_MOONS.get(name)
    return None if row is None else str(row["parent"])


def parse_horizons_elements(text: str, *, name: str) -> KeplerElements:
    """Parse Horizons ELEMENTS CSV ($$SOE … $$EOE) into Keplerian elements."""
    if "$$SOE" not in text or "$$EOE" not in text:
        raise TimeWarpError(f"Horizons {name} result has no element table")
    header_line = None
    for raw in text.splitlines():
        if "JDTDB" in raw and "EC" in raw:
            header_line = raw.strip().strip(",")
            break
    if not header_line:
        raise TimeWarpError(f"Horizons {name} result has no element header")
    body = text.split("$$SOE", 1)[1].split("$$EOE", 1)[0].strip()
    if not body:
        raise TimeWarpError(f"Horizons {name} element table is empty")
    first = body.splitlines()[0].strip()
    reader = csv.reader(io.StringIO(header_line + "\n" + first))
    rows = list(reader)
    if len(rows) < 2:
        raise TimeWarpError(f"Horizons {name} CSV did not parse")
    header = [h.strip() for h in rows[0]]
    values = [v.strip() for v in rows[1]]
    rec = {header[i]: values[i] if i < len(values) else "" for i in range(len(header))}

    def col(*names: str) -> str:
        for n in names:
            if n in rec and rec[n] != "":
                return rec[n]
        raise TimeWarpError(f"Horizons {name} is missing {names[0]}")

    epoch_jd = _as_float(col("JDTDB"), field="epoch")
    e = _as_float(col("EC"), field="e")
    a = _as_float(col("A"), field="a")
    if a <= 0.0:
        raise TimeWarpError(f"Horizons {name} semi-major axis is not positive")
    if e >= 1.0:
        raise TimeWarpError(f"Horizons {name} is not an elliptical orbit (e={e})")
    n = _as_float(col("N"), field="n")
    return KeplerElements(
        name=name,
        a=a,
        e=e,
        i=_as_float(col("IN"), field="i"),
        N=_as_float(col("OM"), field="om"),
        w=_as_float(col("W"), field="w"),
        M0=_rev(_as_float(col("MA"), field="ma")),
        n=n,
        epoch_jd=epoch_jd,
        d_epoch=epoch_jd - _SCHLYTER_JD0,
        designation=name,
    )


def elements_to_payload(el: KeplerElements) -> dict:
    return {
        "name": el.name,
        "a": el.a,
        "e": el.e,
        "i": el.i,
        "om": el.N,
        "w": el.w,
        "ma": el.M0,
        "n": el.n,
        "epoch_jd": el.epoch_jd,
        "designation": el.designation,
    }


def payload_to_elements(data: dict, *, name: str) -> KeplerElements:
    epoch_jd = _as_float(data.get("epoch_jd"), field="epoch")
    a = _as_float(data.get("a"), field="a")
    e = _as_float(data.get("e"), field="e")
    if a <= 0.0:
        raise TimeWarpError(f"Horizons cache {name} semi-major axis is not positive")
    if e >= 1.0:
        raise TimeWarpError(f"Horizons cache {name} is not an elliptical orbit (e={e})")
    return KeplerElements(
        name=name,
        a=a,
        e=e,
        i=_as_float(data.get("i"), field="i"),
        N=_as_float(data.get("om"), field="om"),
        w=_as_float(data.get("w"), field="w"),
        M0=_rev(_as_float(data.get("ma"), field="ma")),
        n=_as_float(data.get("n"), field="n"),
        epoch_jd=epoch_jd,
        d_epoch=epoch_jd - _SCHLYTER_JD0,
        designation=data.get("designation") if isinstance(data.get("designation"), str) else name,
    )


def fetch_horizons(name: str, *, timeout: float = _TIMEOUT) -> KeplerElements:
    meta = HORIZONS_MOONS.get(name)
    if meta is None:
        raise TimeWarpError(f"no Horizons mapping for {name}")
    start_dt = datetime.now(timezone.utc)
    start = start_dt.strftime("%Y-%m-%d")
    stop = (start_dt + timedelta(days=1)).strftime("%Y-%m-%d")
    params = {
        "format": "json",
        "COMMAND": f"'{meta['command']}'",
        "OBJ_DATA": "NO",
        "MAKE_EPHEM": "YES",
        "EPHEM_TYPE": "ELEMENTS",
        "CENTER": f"'{meta['center']}'",
        "REF_PLANE": "ECLIPTIC",
        "START_TIME": f"'{start}'",
        "STOP_TIME": f"'{stop}'",
        "STEP_SIZE": "'1 d'",
        "OUT_UNITS": "AU-D",
        "CSV_FORMAT": "YES",
        "ELM_LABELS": "YES",
    }
    url = HORIZONS_URL + "?" + urllib.parse.urlencode(params)
    req = urllib.request.Request(
        url,
        headers={"User-Agent": "TimeWarp (https://github.com/webaugur/TimeWarp)"},
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            raw = resp.read()
    except urllib.error.HTTPError as exc:
        raise TimeWarpError(f"could not fetch Horizons {name}: HTTP {exc.code}") from exc
    except (urllib.error.URLError, TimeoutError, OSError) as exc:
        raise TimeWarpError(
            f"could not fetch Horizons {name} ({exc}). "
            f"Cached files live in {horizons_cache_dir()}"
        ) from exc
    try:
        data = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise TimeWarpError(f"Horizons response for {name} was not JSON") from exc
    if not isinstance(data, dict):
        raise TimeWarpError(f"Horizons response for {name} is not a JSON object")
    if data.get("error"):
        raise TimeWarpError(f"Horizons {name}: {data['error']}")
    result = data.get("result")
    if not isinstance(result, str) or not result.strip():
        raise TimeWarpError(f"Horizons {name} has no result text")
    return parse_horizons_elements(result, name=name)


def load_moon_elements(name: str, *, refresh: bool = False) -> KeplerElements | None:
    """Osculating planetocentric elements, or None to use the circular fallback."""
    if name not in HORIZONS_MOONS:
        return None
    key = f"{horizons_cache_dir()}::{name}"
    if not refresh and key in _MEMO:
        return _MEMO[key]
    path = horizons_cache_dir() / f"{name}.json"
    stale: KeplerElements | None = None
    if path.is_file() and not refresh:
        try:
            raw = json.loads(path.read_text(encoding="utf-8"))
            if not isinstance(raw, dict):
                raise TimeWarpError("not an object")
            stale = payload_to_elements(raw, name=name)
        except (OSError, json.JSONDecodeError, TimeWarpError):
            stale = None
        else:
            age = datetime.now(timezone.utc) - datetime.fromtimestamp(
                path.stat().st_mtime, tz=timezone.utc
            )
            if age <= CACHE_TTL:
                _MEMO[key] = stale
                return stale
    try:
        el = fetch_horizons(name)
    except TimeWarpError:
        _MEMO[key] = stale
        return stale
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(elements_to_payload(el), indent=2) + "\n", encoding="utf-8")
    except OSError:
        pass
    _MEMO[key] = el
    return el
