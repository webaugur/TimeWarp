"""US holidays via python-holidays; other countries via Nager.Date JSON cache."""

from __future__ import annotations

import json
import os
import urllib.error
import urllib.request
from datetime import date, datetime, timedelta, timezone
from pathlib import Path

from timewarp.errors import TimeWarpError

NAGER_URL = "https://date.nager.at/api/v3/PublicHolidays/{year}/{cc}"
CACHE_TTL = timedelta(days=30)
_US_ALIASES = {"US", "USA", "UNITED STATES", "UNITEDSTATES"}
_GB_ALIASES = {"GB", "UK", "GBR", "UNITED KINGDOM", "UNITEDKINGDOM"}
# Without --region, GB uses England so Easter Monday / summer bank holiday apply.
_DEFAULT_REGION = {"GB": "GB-ENG"}

# ISO 3166-2 US / USPS. No city or county calendars in Nager or python-holidays.
_USPS = {
    "AL": "Alabama",
    "AK": "Alaska",
    "AZ": "Arizona",
    "AR": "Arkansas",
    "CA": "California",
    "CO": "Colorado",
    "CT": "Connecticut",
    "DE": "Delaware",
    "DC": "District of Columbia",
    "FL": "Florida",
    "GA": "Georgia",
    "HI": "Hawaii",
    "ID": "Idaho",
    "IL": "Illinois",
    "IN": "Indiana",
    "IA": "Iowa",
    "KS": "Kansas",
    "KY": "Kentucky",
    "LA": "Louisiana",
    "ME": "Maine",
    "MD": "Maryland",
    "MA": "Massachusetts",
    "MI": "Michigan",
    "MN": "Minnesota",
    "MS": "Mississippi",
    "MO": "Missouri",
    "MT": "Montana",
    "NE": "Nebraska",
    "NV": "Nevada",
    "NH": "New Hampshire",
    "NJ": "New Jersey",
    "NM": "New Mexico",
    "NY": "New York",
    "NC": "North Carolina",
    "ND": "North Dakota",
    "OH": "Ohio",
    "OK": "Oklahoma",
    "OR": "Oregon",
    "PA": "Pennsylvania",
    "RI": "Rhode Island",
    "SC": "South Carolina",
    "SD": "South Dakota",
    "TN": "Tennessee",
    "TX": "Texas",
    "UT": "Utah",
    "VT": "Vermont",
    "VA": "Virginia",
    "WA": "Washington",
    "WV": "West Virginia",
    "WI": "Wisconsin",
    "WY": "Wyoming",
    "PR": "Puerto Rico",
    "VI": "Virgin Islands",
    "GU": "Guam",
    "AS": "American Samoa",
    "MP": "Northern Mariana Islands",
}
_USPS_BY_NAME = {name.upper().replace(" ", "").replace(".", ""): code for code, name in _USPS.items()}
_USPS_BY_NAME["WASHINGTONDC"] = "DC"
_USPS_BY_NAME["DISTRICTOFCOLUMBIA"] = "DC"

WEEKDAY_INDEX = {
    "mon": 0,
    "monday": 0,
    "tue": 1,
    "tues": 1,
    "tuesday": 1,
    "wed": 2,
    "wednesday": 2,
    "thu": 3,
    "thur": 3,
    "thurs": 3,
    "thursday": 3,
    "fri": 4,
    "friday": 4,
    "sat": 5,
    "saturday": 5,
    "sun": 6,
    "sunday": 6,
}


def _python_holidays(year: int, *, subdiv: str | None = None) -> list[tuple[date, str]]:
    """Public holidays from python-holidays (offline). subdiv=None is US federal."""
    try:
        import holidays as pyholidays
    except ImportError as exc:
        raise TimeWarpError(
            "US holidays need the holidays package; from this tree: "
            ".venv/bin/pip install holidays   (or: pip install holidays)"
        ) from exc
    cal = pyholidays.country_holidays("US", subdiv=subdiv, years=year)
    out: list[tuple[date, str]] = []
    for d, name in sorted(cal.items()):
        if d.year != year:
            continue
        label = name if isinstance(name, str) else "; ".join(str(p) for p in name)
        out.append((d, label))
    return out


def us_federal_holidays(year: int) -> list[tuple[date, str]]:
    """US federal public holidays for a year (python-holidays, including observed dates)."""
    return _python_holidays(year)


def us_holiday_set(year: int) -> set[date]:
    return {d for d, _ in us_federal_holidays(year)}


def _country_code(country: str) -> str:
    raw = country.strip().upper().replace(" ", "")
    if raw in _US_ALIASES:
        return "US"
    if raw in _GB_ALIASES:
        return "GB"
    if len(raw) == 2 and raw.isalpha():
        return raw
    raise TimeWarpError(
        f"holiday country {country!r} is not an ISO 3166-1 alpha-2 code (e.g. US, GB, DE, CA)"
    )


def holiday_cache_dir() -> Path:
    env = os.environ.get("TIMEWARP_HOLIDAY_DIR")
    if env:
        return Path(env)
    xdg = os.environ.get("XDG_CACHE_HOME")
    root = Path(xdg) if xdg else Path.home() / ".cache"
    return root / "timewarp" / "holidays"


def _cache_path(cc: str, year: int) -> Path:
    return holiday_cache_dir() / f"{cc}-{year}.json"


def _read_json(path: Path) -> list:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise TimeWarpError(f"could not read holiday cache {path}: {exc}") from exc
    if not isinstance(data, list):
        raise TimeWarpError(f"holiday cache {path} is not a JSON array")
    return data


def _fetch_nager(cc: str, year: int, *, timeout: float = 20.0) -> list:
    url = NAGER_URL.format(year=year, cc=cc)
    try:
        with urllib.request.urlopen(url, timeout=timeout) as resp:
            raw = resp.read()
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            raise TimeWarpError(
                f"Nager.Date has no public-holiday calendar for {cc} {year}"
            ) from exc
        raise TimeWarpError(f"could not fetch holidays for {cc} {year}: HTTP {exc.code}") from exc
    except (urllib.error.URLError, TimeoutError, OSError) as exc:
        raise TimeWarpError(
            f"could not fetch holidays for {cc} {year} ({exc}). "
            f"Use --holidays US or a cached file in {holiday_cache_dir()}"
        ) from exc
    try:
        data = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise TimeWarpError(f"Nager.Date response for {cc} {year} was not JSON") from exc
    if not isinstance(data, list):
        raise TimeWarpError(f"Nager.Date response for {cc} {year} is not a JSON array")
    return data


def load_nager_year(cc: str, year: int, *, refresh: bool = False) -> tuple[list, str | None]:
    """Return (raw Nager rows, optional note). Uses disk cache."""
    path = _cache_path(cc, year)
    note = None
    if path.is_file() and not refresh:
        age = datetime.now(timezone.utc) - datetime.fromtimestamp(path.stat().st_mtime, tz=timezone.utc)
        if age <= CACHE_TTL:
            return _read_json(path), None
        note = f"holiday cache for {cc} {year} is {age.days} days old; refreshing"
    try:
        data = _fetch_nager(cc, year)
    except TimeWarpError:
        if path.is_file():
            return _read_json(path), f"using stale holiday cache {path.name} (fetch failed)"
        raise
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    except OSError:
        pass
    return data, note


def _row_applies(row: dict, region: str | None) -> bool:
    types = row.get("types") or []
    if "Public" not in types:
        return False
    counties = row.get("counties")
    if not counties:
        return True
    if region:
        want = region.strip().upper()
        return any(str(c).upper() == want for c in counties)
    return bool(row.get("global"))


def nager_holidays(year: int, cc: str, *, refresh: bool = False, region: str | None = None) -> tuple[list[tuple[date, str]], str | None]:
    rows, note = load_nager_year(cc, year, refresh=refresh)
    out: list[tuple[date, str]] = []
    for row in rows:
        if not isinstance(row, dict):
            continue
        if not _row_applies(row, region):
            continue
        raw = row.get("date")
        name = row.get("name") or row.get("localName") or "Holiday"
        try:
            d = date.fromisoformat(str(raw)[:10])
        except ValueError:
            continue
        if d.year == year:
            out.append((d, str(name)))
    out.sort(key=lambda item: item[0])
    # unique dates: first name wins
    seen: dict[date, str] = {}
    for d, name in out:
        seen.setdefault(d, name)
    return [(d, seen[d]) for d in sorted(seen)], note


def normalize_us_region(region: str) -> str:
    """IN, US-IN, or Indiana → IN."""
    raw = region.strip().upper().replace(" ", "").replace(".", "")
    if raw.startswith("US-"):
        raw = raw[3:]
    if len(raw) == 2 and raw in _USPS:
        return raw
    if raw in _USPS_BY_NAME:
        return _USPS_BY_NAME[raw]
    raise TimeWarpError(
        f"unknown US region {region!r}; use a state code (IN, US-CA) or name (Indiana)"
    )


def us_state_holidays(year: int, region: str) -> list[tuple[date, str]]:
    """Federal + state public holidays from python-holidays (offline)."""
    return _python_holidays(year, subdiv=normalize_us_region(region))


def holidays_for_year(
    year: int,
    country: str = "US",
    *,
    refresh: bool = False,
    region: str | None = None,
) -> tuple[list[tuple[date, str]], str | None]:
    cc = _country_code(country)
    if cc == "US":
        if region:
            return us_state_holidays(year, region), None
        return us_federal_holidays(year), None
    use_region = region
    if use_region is None and cc in _DEFAULT_REGION:
        use_region = _DEFAULT_REGION[cc]
    return nager_holidays(year, cc, refresh=refresh, region=use_region)


def holidays_in_range(
    start: date,
    end: date,
    country: str = "US",
    *,
    refresh: bool = False,
    region: str | None = None,
) -> set[date]:
    lo, hi = (start, end) if start <= end else (end, start)
    days: set[date] = set()
    for year in range(lo.year, hi.year + 1):
        rows, _note = holidays_for_year(year, country, refresh=refresh, region=region)
        for d, _name in rows:
            if lo <= d <= hi:
                days.add(d)
    return days


def parse_weekend(spec: str | None) -> frozenset[int]:
    """Return datetime.weekday() indices treated as weekend. Default Saturday+Sunday."""
    if spec is None or spec.strip() == "":
        return frozenset({5, 6})
    days = set()
    for part in spec.replace(" ", "").split(","):
        if not part:
            continue
        key = part.lower()
        if key.isdigit():
            n = int(key)
            # ISO 1-7 Monday-Sunday
            if 1 <= n <= 7:
                days.add(n - 1 if n < 7 else 6)
                continue
        if key not in WEEKDAY_INDEX:
            raise TimeWarpError(
                f"unknown weekday {part!r} in --weekend; use Mon,Tue,... or ISO 1-7"
            )
        days.add(WEEKDAY_INDEX[key])
    if not days:
        return frozenset({5, 6})
    return frozenset(days)


def country_label(country: str | None) -> str:
    if not country:
        return "none"
    return country.strip().upper()
