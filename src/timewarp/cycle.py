"""Rosicrucian year and Lewis cycle arithmetic.

Calendar rules (user + public AMORC):
  year = CE + 1353; new year = March equinox *date*; the RC day (and that
  new year) starts at **local sunrise**, not at the equinox instant and not
  at midnight. Stamp YEAR.DDD = sunrise-days since the equinox sunrise.
  1690-year cycles from sunrise on the 337 CE March-equinox date.

Lewis *Self-Mastery and Fate with the Cycles of Life*: period *lengths* only.
Daily A–G letters follow https://cycles.amorc.org/en/cycles but are counted
from sunrise (seven slices of the RC day). Place defaults to Greenwich.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date, datetime, timedelta, timezone
from zoneinfo import ZoneInfo

from timewarp.astro import seasons_for_year, sun_times
from timewarp.errors import TimeWarpError
from timewarp.iso import Instant, as_date, format_clock, format_instant
from timewarp.places import Place

# AMORC historically reckons the equinox at Greenwich.
GREENWICH = Place("Greenwich", 51.4779, -0.0015, "Europe/London")

CE_OFFSET = 1353
CYCLE_YEARS = 1690
EPOCH_CE = 337
# Soul-cycle civil grid (1947 Digest / Lewis ch. 14): seven ~52-day blocks
# beginning 22 March.
SOUL_START_MONTH = 3
SOUL_START_DAY = 22

# AMORC cycles.amorc.org colors for letters A–G (musical notes).
NOTE_COLOR = {
    "A": (241, 207, 103),
    "B": (146, 209, 129),
    "C": (232, 155, 69),
    "D": (204, 117, 199),
    "E": (143, 160, 201),
    "F": (123, 196, 223),
    "G": (238, 111, 111),
}
NOTE_HEX = {k: f"#{r:02x}{g:02x}{b:02x}" for k, (r, g, b) in NOTE_COLOR.items()}
# Names for the same hex values (cycles.amorc.org has no English labels).
NOTE_NAME = {
    "A": "gold",
    "B": "green",
    "C": "orange",
    "D": "orchid",
    "E": "periwinkle",
    "F": "sky",
    "G": "coral",
}

# Daily 24h / 7 (AMORC minute widths, counted from local sunrise).
DAILY_SLICES = (
    (1, 0, 205, "Midnight to 3:25 a.m."),
    (2, 205, 411, "3:25 a.m. to 6:51 a.m."),
    (3, 411, 617, "6:51 a.m. to 10:17 a.m."),
    (4, 617, 822, "10:17 a.m. to 1:42 p.m."),
    (5, 822, 1028, "1:42 p.m. to 5:08 p.m."),
    (6, 1028, 1234, "5:08 p.m. to 8:34 p.m."),
    (7, 1234, 1440, "8:34 p.m. to midnight"),
)

# weekday (Mon=0..Sun=6 Python) → AMORC sunday-first index
_PY_TO_SUNDAY = (1, 2, 3, 4, 5, 6, 0)

# period 1..7 × sunday-first weekday → letter (cycles.amorc.org)
DAILY_LETTERS = (
    # Sun Mon Tue Wed Thu Fri Sat
    ("G", "C", "F", "B", "E", "A", "D"),
    ("A", "D", "G", "C", "F", "B", "E"),
    ("B", "E", "A", "D", "G", "C", "F"),
    ("C", "F", "B", "E", "A", "D", "G"),
    ("D", "G", "C", "F", "B", "E", "A"),
    ("E", "A", "D", "G", "C", "F", "B"),
    ("F", "B", "E", "A", "D", "G", "C"),
)

DAILY_PLANET = (None, "sun", "venus", "mercury", "moon", "mars", "jupiter", "saturn")

# Yearly cycle 1–7 keywords (Hershenow / 1947 Digest public summary of Lewis).
YEARLY_KEY = (
    "promotional",
    "developmental",
    "energetical",
    "inspirational",
    "successful",
    "recreational",
    "transitional",
)

_EQ_CACHE: dict[int, datetime] = {}
_RISE_CACHE: dict[tuple[int, float, float], datetime | None] = {}


def march_equinox(year: int) -> datetime:
    if year not in _EQ_CACHE:
        if not 1 <= year <= 9999:
            raise TimeWarpError(f"year {year} is out of range 1..9999")
        _EQ_CACHE[year] = seasons_for_year(year)[0].time
    return _EQ_CACHE[year]


def _as_utc_dt(when: Instant) -> datetime:
    if isinstance(when, datetime):
        if when.tzinfo is None:
            return when.replace(tzinfo=timezone.utc)
        return when.astimezone(timezone.utc)
    return datetime(when.year, when.month, when.day, tzinfo=timezone.utc)


def _local_dt(when: Instant, tz: str) -> datetime:
    utc = _as_utc_dt(when)
    if isinstance(when, date) and not isinstance(when, datetime):
        # Date-only: noon local — after sunrise except polar night.
        return datetime(when.year, when.month, when.day, 12, 0, tzinfo=ZoneInfo(tz))
    return utc.astimezone(ZoneInfo(tz))


def sunrise_on(day: date, place: Place) -> datetime | None:
    key = (day.toordinal(), round(place.lat, 4), round(place.lon, 4))
    if key not in _RISE_CACHE:
        _RISE_CACHE[key] = sun_times(day, place).sunrise
    return _RISE_CACHE[key]


def _civil_midnight(day: date, place: Place) -> datetime:
    return datetime(day.year, day.month, day.day, tzinfo=ZoneInfo(place.tz))


def day_boundary(day: date, place: Place) -> tuple[datetime, str | None]:
    """Start of the RC day on this civil date: sunrise, or midnight if the sun does not rise."""
    rise = _civil_midnight(day, place)
    if rise is None:
        return _civil_midnight(day, place), "no sunrise; RC day from local midnight"
    return rise, None


def rc_day(when: Instant, place: Place) -> tuple[date, datetime, str | None]:
    """Civil date and instant of the last RC day-start (sunrise) at or before `when`."""
    local = _local_dt(when, place.tz)
    d = local.date()
    start, note = day_boundary(d, place)
    start_local = start.astimezone(ZoneInfo(place.tz))
    if local < start_local:
        d = d - timedelta(days=1)
        start, note = day_boundary(d, place)
    return d, start, note


@dataclass(frozen=True)
class RosicrucianStamp:
    rc_year: int
    day: int
    equinox: datetime
    next_equinox: datetime
    ce_year: int
    day_start: datetime
    note: str | None = None

    def stamp(self) -> str:
        return f"{self.rc_year}.{self.day:03d}"


def _equinox_sunrise(ce_year: int, place: Place) -> tuple[date, datetime]:
    eq = march_equinox(ce_year)
    eq_date = eq.astimezone(ZoneInfo(place.tz)).date()
    start, _note = day_boundary(eq_date, place)
    return eq_date, start


def rosicrucian_stamp(when: Instant, place: Place | None = None) -> RosicrucianStamp:
    place = place or GREENWICH
    d, start, note = rc_day(when, place)
    y = d.year
    if y < 1:
        raise TimeWarpError("Rosicrucian year is not defined before 1 CE")
    eq_date, eq_start = _equinox_sunrise(y, place)
    if start < eq_start:
        y -= 1
        if y < 1:
            raise TimeWarpError("Rosicrucian year is not defined before 1 CE")
        eq_date, eq_start = _equinox_sunrise(y, place)
    if y < 9999:
        _nxt_date, nxt_start = _equinox_sunrise(y + 1, place)
    else:
        nxt_start = eq_start + timedelta(days=365)
    day = (d - eq_date).days
    return RosicrucianStamp(
        rc_year=y + CE_OFFSET,
        day=day,
        equinox=eq_start,
        next_equinox=nxt_start,
        ce_year=y,
        day_start=start,
        note=note,
    )


@dataclass(frozen=True)
class Cycle1690:
    index: int
    start: datetime
    end: datetime
    elapsed_days: int
    remaining_days: int
    length_days: int


def cycle_1690(when: Instant, place: Place | None = None) -> Cycle1690:
    place = place or GREENWICH
    d, start, _note = rc_day(when, place)
    _eq0_date, start0 = _equinox_sunrise(EPOCH_CE, place)
    if start < start0:
        raise TimeWarpError("1690-year cycle epoch is sunrise on the March equinox date of 337 CE")
    ce = d.year
    _eq_date, eq_start = _equinox_sunrise(ce, place)
    if start < eq_start:
        ce -= 1
    n = (ce - EPOCH_CE) // CYCLE_YEARS
    start_ce = EPOCH_CE + n * CYCLE_YEARS
    end_ce = start_ce + CYCLE_YEARS
    _sdate, cstart = _equinox_sunrise(start_ce, place)
    _edate, cend = _equinox_sunrise(end_ce, place)
    elapsed = (d - _sdate).days
    length = (_edate - _sdate).days
    remaining = (_edate - d).days
    return Cycle1690(
        index=n,
        start=cstart,
        end=cend,
        elapsed_days=elapsed,
        remaining_days=max(0, remaining),
        length_days=length,
    )


def life_period(born: date, when: Instant) -> tuple[int, int, int, int]:
    """Return (1-based period, age_years, age_lo, age_hi) for 7-year life stages."""
    day = as_date(when)
    years = day.year - born.year
    if (day.month, day.day) < (born.month, born.day):
        years -= 1
    if years < 0:
        raise TimeWarpError("birth date is after the query date")
    idx = years // 7
    lo = idx * 7
    return idx + 1, years, lo, lo + 7


def _period_index(start: datetime, when: datetime, n: int = 7) -> int:
    span = (start.replace(year=start.year + 1) - start).total_seconds()
    # Birthday this year:
    try:
        this = start.replace(year=when.year)
    except ValueError:
        this = start.replace(year=when.year, day=28)
    if when < this:
        try:
            this = start.replace(year=when.year - 1)
        except ValueError:
            this = start.replace(year=when.year - 1, day=28)
    elapsed = (when - this).total_seconds()
    if elapsed < 0:
        elapsed = 0
    slot = span / n
    i = int(elapsed // slot)
    if i >= n:
        i = n - 1
    return i


def yearly_period(born: Instant, when: Instant, tz: str) -> tuple[int, str, str]:
    b = _local_dt(born, tz)
    w = _local_dt(when, tz)
    i = _period_index(b, w)
    letter = chr(ord("A") + i)
    return i + 1, letter, YEARLY_KEY[i]


def daily_period(when: Instant, place: Place | None = None) -> dict:
    place = place or GREENWICH
    d, start, note = rc_day(when, place)
    local = _local_dt(when, place.tz)
    elapsed_min = int((local - start.astimezone(ZoneInfo(place.tz))).total_seconds() // 60)
    if elapsed_min < 0:
        elapsed_min = 0
    slice_i = 6
    period = 7
    lo_m, hi_m = DAILY_SLICES[6][1], DAILY_SLICES[6][2]
    for p, lo, hi, _lab in DAILY_SLICES:
        if lo <= elapsed_min < hi:
            period, slice_i, lo_m, hi_m = p, p - 1, lo, hi
            break
    span_lo = start + timedelta(minutes=lo_m)
    span_hi = start + timedelta(minutes=hi_m)
    sun_i = _PY_TO_SUNDAY[d.weekday()]
    letter = DAILY_LETTERS[slice_i][sun_i]
    return {
        "period": period,
        "letter": letter,
        "time": f"{format_clock(span_lo)}–{format_clock(span_hi)}",
        "weekday": d.strftime("%A"),
        "planet": DAILY_PLANET[period],
        "color": NOTE_NAME[letter],
        "color_hex": NOTE_HEX[letter],
        "color_rgb": NOTE_COLOR[letter],
        "day_start": format_instant(start),
        "note": note,
    }


def soul_period(born: date) -> tuple[int, str]:
    """Which of the seven 22 March soul-cycle blocks the birth date falls in."""
    y = born.year
    starts = []
    try:
        origin = date(y, SOUL_START_MONTH, SOUL_START_DAY)
    except ValueError:
        origin = date(y, 3, 22)
    for k in range(7):
        starts.append(origin + timedelta(days=52 * k))
    # Period 7 wraps into next March 21.
    if born >= starts[0]:
        i = 0
        for k in range(6, -1, -1):
            if born >= starts[k]:
                i = k
                break
    else:
        p7 = date(y - 1, SOUL_START_MONTH, SOUL_START_DAY) + timedelta(days=52 * 6)
        i = 6 if born >= p7 else 5
    letter = chr(ord("A") + i)
    return i + 1, letter


def format_note(letter: str, *, color: bool) -> str:
    """A–G with optional tint; music emoji sits next to the letter, not the date."""
    if not color:
        return letter
    r, g, b = NOTE_COLOR[letter]
    tinted = f"\033[1;38;2;{r};{g};{b}m{letter}\033[0m"
    return f"🎵 {tinted}"


def format_color_period(letter: str, *, color: bool) -> str:
    """Color period that belongs to this note (AMORC hex, named)."""
    name = NOTE_NAME[letter]
    hex_s = NOTE_HEX[letter]
    r, g, b = NOTE_COLOR[letter]
    if not color:
        return f"{name}  {hex_s}"
    swatch = f"\033[48;2;{r};{g};{b}m      \033[0m"
    tinted = f"\033[1;38;2;{r};{g};{b}m{name}\033[0m"
    return f"{tinted}  {swatch}  {hex_s}"


def to_dict(
    when: Instant,
    *,
    place: Place | None = None,
    born: Instant | None = None,
) -> dict:
    place = place or GREENWICH
    stamp = rosicrucian_stamp(when, place)
    cyc = cycle_1690(when, place)
    daily = daily_period(when, place)
    payload = {
        "stamp": stamp.stamp(),
        "star_date": stamp.stamp(),
        "rc_year": stamp.rc_year,
        "day_of_year": stamp.day,
        "ce_year": stamp.ce_year,
        "place": place.name,
        "tz": place.tz,
        "day_start": format_instant(stamp.day_start),
        "equinox_sunrise": format_instant(stamp.equinox),
        "next_equinox_sunrise": format_instant(stamp.next_equinox),
        "cycle_1690": {
            "index": cyc.index,
            "start": format_instant(cyc.start),
            "end": format_instant(cyc.end),
            "elapsed_days": cyc.elapsed_days,
            "remaining_days": cyc.remaining_days,
            "length_days": cyc.length_days,
        },
        "daily": daily,
        "source": {
            "calendar": "AMORC: CE+1353; RC day and equinox year start at local sunrise",
            "lewis": "H. Spencer Lewis, Self-Mastery and Fate with the Cycles of Life (1929)",
            "clock": "https://cycles.amorc.org/en/cycles (A–G letters; slices from sunrise)",
        },
    }
    if stamp.note:
        payload["note"] = stamp.note
    if born is not None:
        bdate = as_date(born)
        life_n, age, lo, hi = life_period(bdate, when)
        y_n, y_letter, y_key = yearly_period(born, when, place.tz)
        soul_n, soul_letter = soul_period(bdate)
        payload["born"] = bdate.isoformat()
        payload["lewis"] = {
            "life_period": life_n,
            "age_years": age,
            "life_span": f"{lo}–{hi}",
            "yearly_period": y_n,
            "yearly_letter": y_letter,
            "yearly_key": y_key,
            "business_period": y_n,
            "health_period": y_n,
            "soul_period": soul_n,
            "soul_letter": soul_letter,
            "incarnation_interval_years": 144,
        }
    return payload
