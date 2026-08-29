"""JPL Small-Body Database osculating elements for named asteroids and comets.

GET https://ssd-api.jpl.nasa.gov/sbdb.api?sstr=…&full-prec=1
Cache: ~/.cache/timewarp/sbdb/{name}.json (7 days). TIMEWARP_SBDB_DIR overrides.
"""

from __future__ import annotations

import json
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from pathlib import Path

from timewarp.errors import TimeWarpError
from timewarp.paths import cache_subdir

SBDB_URL = "https://ssd-api.jpl.nasa.gov/sbdb.api?sstr={sstr}&full-prec=1"
CACHE_TTL = timedelta(days=7)
# Schlyter d = JD − 2451543.5 (2000 Jan 0.0). SBDB epoch is Julian day TDB.
_SCHLYTER_JD0 = 2451543.5
# Gaussian gravitational constant, degrees per day (n = k / a^{3/2}).
_K_DEG_PER_DAY = 0.9856076686
_TIMEOUT = 20.0

# Process-local parse cache; keyed by cache dir + body so tests can isolate.
_MEMO: dict[str, KeplerElements | None] = {}

# Internal body name → SBDB search string. Only the named extras in ephem.
SBDB_QUERY = {
    "ceres": "Ceres",
    "pallas": "Pallas",
    "juno": "Juno",
    "vesta": "Vesta",
    "hygiea": "Hygiea",
    "eros": "Eros",
    "halley": "1P",
    "encke": "2P",
    "tempel1": "9P",
    "67p": "67P",
}


@dataclass(frozen=True)
class KeplerElements:
    """Osculating Keplerian elements at `epoch_jd` (TDB Julian day)."""

    name: str
    a: float
    e: float
    i: float
    N: float
    w: float
    M0: float
    n: float
    epoch_jd: float
    d_epoch: float
    designation: str | None = None


def sbdb_cache_dir() -> Path:
    return cache_subdir("TIMEWARP_SBDB_DIR", "sbdb")


def _rev(deg: float) -> float:
    x = deg % 360.0
    return x + 360.0 if x < 0.0 else x


def _as_float(value: object, *, field: str) -> float:
    if isinstance(value, bool) or value is None:
        raise TimeWarpError(f"SBDB {field} is missing")
    if isinstance(value, (int, float)):
        return float(value)
    if isinstance(value, str):
        text = value.strip()
        if not text:
            raise TimeWarpError(f"SBDB {field} is empty")
        try:
            return float(text)
        except ValueError as exc:
            raise TimeWarpError(f"SBDB {field} is not a number: {value!r}") from exc
    raise TimeWarpError(f"SBDB {field} is not a number")


def _element_map(orbit: dict) -> dict[str, float]:
    raw = orbit.get("elements")
    if not isinstance(raw, list):
        raise TimeWarpError("SBDB orbit has no elements list")
    out: dict[str, float] = {}
    for row in raw:
        if not isinstance(row, dict):
            continue
        key = row.get("name")
        if not isinstance(key, str) or not key:
            continue
        out[key] = _as_float(row.get("value"), field=key)
    return out


def parse_sbdb(data: object, *, name: str) -> KeplerElements:
    """Parse an SBDB API object into Keplerian elements."""
    if not isinstance(data, dict):
        raise TimeWarpError("SBDB response is not a JSON object")
    code = data.get("code")
    if code is not None and "orbit" not in data:
        msg = data.get("message") or f"SBDB error {code}"
        extra = data.get("list")
        if isinstance(extra, list) and extra:
            labels = []
            for row in extra[:8]:
                if isinstance(row, dict):
                    labels.append(str(row.get("pdes") or row.get("name") or row.get("fullname") or "?"))
            if labels:
                msg += "; matches: " + ", ".join(labels)
        raise TimeWarpError(f"SBDB {name}: {msg}")
    orbit = data.get("orbit")
    if not isinstance(orbit, dict):
        raise TimeWarpError(f"SBDB {name} has no orbit")
    els = _element_map(orbit)
    epoch_jd = _as_float(orbit.get("epoch"), field="epoch")
    for req in ("a", "e", "i", "om", "w", "ma"):
        if req not in els:
            raise TimeWarpError(f"SBDB {name} is missing element {req}")
    a = els["a"]
    if a <= 0.0:
        raise TimeWarpError(f"SBDB {name} semi-major axis is not positive")
    e = els["e"]
    if e >= 1.0:
        raise TimeWarpError(f"SBDB {name} is not an elliptical orbit (e={e})")
    n = els.get("n")
    if n is None or n <= 0.0:
        n = _K_DEG_PER_DAY / (a**1.5)
    obj = data.get("object") if isinstance(data.get("object"), dict) else {}
    des = obj.get("fullname") or obj.get("des") or obj.get("shortname")
    designation = des if isinstance(des, str) else None
    return KeplerElements(
        name=name,
        a=a,
        e=e,
        i=els["i"],
        N=els["om"],
        w=els["w"],
        M0=_rev(els["ma"]),
        n=n,
        epoch_jd=epoch_jd,
        d_epoch=epoch_jd - _SCHLYTER_JD0,
        designation=designation,
    )


def mean_anomaly(el: KeplerElements, d: float) -> float:
    """Mean anomaly (deg) at Schlyter day number `d`."""
    return _rev(el.M0 + el.n * (d - el.d_epoch))


def _read_json(path: Path) -> dict:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise TimeWarpError(f"could not read SBDB cache {path}: {exc}") from exc
    if not isinstance(data, dict):
        raise TimeWarpError(f"SBDB cache {path} is not a JSON object")
    return data


def fetch_sbdb(sstr: str, *, timeout: float = _TIMEOUT) -> dict:
    url = SBDB_URL.format(sstr=urllib.parse.quote(sstr, safe=""))
    req = urllib.request.Request(
        url,
        headers={"User-Agent": "TimeWarp (https://github.com/webaugur/TimeWarp)"},
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            raw = resp.read()
    except urllib.error.HTTPError as exc:
        raise TimeWarpError(f"could not fetch SBDB {sstr!r}: HTTP {exc.code}") from exc
    except (urllib.error.URLError, TimeoutError, OSError) as exc:
        raise TimeWarpError(
            f"could not fetch SBDB {sstr!r} ({exc}). "
            f"Cached files live in {sbdb_cache_dir()}"
        ) from exc
    try:
        data = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise TimeWarpError(f"SBDB response for {sstr!r} was not JSON") from exc
    if not isinstance(data, dict):
        raise TimeWarpError(f"SBDB response for {sstr!r} is not a JSON object")
    return data


def query_slug(sstr: str) -> str:
    raw = sstr.strip().lower().replace(" ", "")
    chars: list[str] = []
    for ch in raw:
        if ch.isalnum() or ch in "-_.":
            chars.append(ch)
        elif ch == "/":
            chars.append("-")
    return "".join(chars)[:80] or "query"


def load_elements(
    name: str,
    *,
    sstr: str | None = None,
    refresh: bool = False,
    required: bool = False,
) -> KeplerElements | None:
    """Osculating elements, or None to use the fallback table.

    `required=True` raises if SBDB cannot be fetched and there is no cache
    (used for arbitrary designations). Named extras stay optional.
    """
    query = sstr if sstr is not None else SBDB_QUERY.get(name)
    if query is None:
        if required:
            raise TimeWarpError(f"no SBDB query for {name}")
        return None
    slug = query_slug(name)
    key = f"{sbdb_cache_dir()}::{slug}"
    if not refresh and key in _MEMO:
        return _MEMO[key]
    path = sbdb_cache_dir() / f"{slug}.json"
    stale: KeplerElements | None = None
    if path.is_file() and not refresh:
        try:
            stale = parse_sbdb(_read_json(path), name=name)
        except TimeWarpError:
            stale = None
        else:
            age = datetime.now(timezone.utc) - datetime.fromtimestamp(
                path.stat().st_mtime, tz=timezone.utc
            )
            if age <= CACHE_TTL:
                _MEMO[key] = stale
                return stale
    try:
        data = fetch_sbdb(query)
        el = parse_sbdb(data, name=name)
    except TimeWarpError:
        if stale is not None:
            _MEMO[key] = stale
            return stale
        if required:
            raise
        _MEMO[key] = None
        return None
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    except OSError:
        pass
    _MEMO[key] = el
    return el


def load_query(sstr: str, *, refresh: bool = False) -> KeplerElements:
    """SBDB elements for an arbitrary search string (number, packed des, name)."""
    q = sstr.strip()
    if not q:
        raise TimeWarpError("empty SBDB query")
    slug = query_slug(q)
    el = load_elements(slug, sstr=q, refresh=refresh, required=True)
    if el is None:
        raise TimeWarpError(f"SBDB has no elliptical orbit for {q!r}")
    return el
