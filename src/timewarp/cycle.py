"""Rosicrucian year and Lewis cycle arithmetic.

Calendar rules (user + public AMORC):
  year = CE + 1353; new year = March equinox *date*; the RC day (and that
  new year) starts at **local midnight**, not at the equinox instant.
  Star date YEAR.DDD = midnights since the equinox-date midnight.
  1690-year cycles from midnight on the 337 CE March-equinox date.

Daily periods follow H. Spencer Lewis, *Self-Mastery and Fate with the
Cycles of Life* (1929), Chart E: seven exact parts of the local standard
day (86400/7 seconds). Printed boundaries (03:25, 06:51, …) are that
division rounded to the minute and are only for display. Daylight saving
is removed before the period is chosen. The letter is the color's key.
Planets are the Chaldean rulers Lewis built the cycle on (Nordhaus-Bike
2022): A Sun, B Venus, C Mercury, D Moon, E Saturn, F Jupiter, G Mars.
Lewis prints no pigments. The colors are Heindel's planetary colors
(*The Message of the Stars*, 1918, p. 64). https://cycles.amorc.org/en/cycles
is only a letter-grid reference. Place defaults to Greenwich.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date, datetime, timedelta, timezone
from zoneinfo import ZoneInfo

from timewarp.astro import seasons_for_year
from timewarp.errors import TimeWarpError
from timewarp.iso import Instant, as_date, format_instant
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

# One seventh of a civil day. Lewis's "3 hours, 25 minutes, and 43 seconds."
PERIOD_SECS = 86400.0 / 7.0

# Heindel pigments, keyed by Lewis's Chart E letter (not by period number).
NOTE_COLOR = {
    "A": (0xFF, 0x8C, 0x00),  # Sun, orange
    "B": (0xE6, 0xC2, 0x00),  # Venus, yellow
    "C": (0x7B, 0x2D, 0xB0),  # Mercury, violet
    "D": (0x2E, 0x8B, 0x57),  # Moon, green
    "E": (0x32, 0x2A, 0x6E),  # Saturn, indigo
    "F": (0x1E, 0x5A, 0xB0),  # Jupiter, blue
    "G": (0xD0, 0x22, 0x22),  # Mars, red
}
NOTE_HEX = {k: f"#{r:02x}{g:02x}{b:02x}" for k, (r, g, b) in NOTE_COLOR.items()}
NOTE_NAME = {
    "A": "orange",
    "B": "yellow",
    "C": "violet",
    "D": "green",
    "E": "indigo",
    "F": "blue",
    "G": "red",
}
# Chaldean day ruler on the letter, not on the period index.
NOTE_PLANET = {
    "A": "sun",
    "B": "venus",
    "C": "mercury",
    "D": "moon",
    "E": "saturn",
    "F": "jupiter",
    "G": "mars",
}
# Kept so a period-indexed lookup cannot drift from the letter again.
DAILY_PLANET = {letter: NOTE_PLANET[letter] for letter in "ABCDEFG"}

# Lewis's printed boundaries, minutes from standard midnight. Display only.
LEWIS_MINUTES = (0, 205, 411, 617, 822, 1028, 1234, 1440)

# weekday (Mon=0..Sun=6 Python) → Sunday-first index
_PY_TO_SUNDAY = (1, 2, 3, 4, 5, 6, 0)
_LETTERS = "ABCDEFG"

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
        # Date-only: noon local (after midnight on that civil date).
        return datetime(when.year, when.month, when.day, 12, 0, tzinfo=ZoneInfo(tz))
    return utc.astimezone(ZoneInfo(tz))


def _civil_midnight(day: date, place: Place) -> datetime:
    return datetime(day.year, day.month, day.day, tzinfo=ZoneInfo(place.tz))


def day_boundary(day: date, place: Place) -> tuple[datetime, str | None]:
    """Start of the RC day: local midnight."""
    return _civil_midnight(day, place), None


def rc_day(when: Instant, place: Place) -> tuple[date, datetime, str | None]:
    """Civil date and local midnight of the RC day containing `when`."""
    local = _local_dt(when, place.tz)
    d = local.date()
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


def _equinox_day_start(ce_year: int, place: Place) -> tuple[date, datetime]:
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
    eq_date, eq_start = _equinox_day_start(y, place)
    if start < eq_start:
        y -= 1
        if y < 1:
            raise TimeWarpError("Rosicrucian year is not defined before 1 CE")
        eq_date, eq_start = _equinox_day_start(y, place)
    if y < 9999:
        _nxt_date, nxt_start = _equinox_day_start(y + 1, place)
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
    _eq0_date, start0 = _equinox_day_start(EPOCH_CE, place)
    if start < start0:
        raise TimeWarpError("1690-year cycle epoch is midnight on the March equinox date of 337 CE")
    ce = d.year
    _eq_date, eq_start = _equinox_day_start(ce, place)
    if start < eq_start:
        ce -= 1
    n = (ce - EPOCH_CE) // CYCLE_YEARS
    start_ce = EPOCH_CE + n * CYCLE_YEARS
    end_ce = start_ce + CYCLE_YEARS
    _sdate, cstart = _equinox_day_start(start_ce, place)
    _edate, cend = _equinox_day_start(end_ce, place)
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


def dst_advance_seconds(local: datetime) -> int:
    """Seconds the civil clock is ahead of local standard time.

    The standard offset is the smaller of the 15 January and 15 July offsets
    in this zone. A zone with no daylight saving contributes 0. Indianapolis
    has observed daylight saving since 2006, so a July afternoon there is
    3600 seconds ahead of Eastern standard time.
    """
    if local.tzinfo is None or local.utcoffset() is None:
        return 0
    zone = local.tzinfo
    year = local.year
    january = datetime(year, 1, 15, 12, tzinfo=zone)
    july = datetime(year, 7, 15, 12, tzinfo=zone)
    jan_off = january.utcoffset()
    jul_off = july.utcoffset()
    now_off = local.utcoffset()
    if jan_off is None or jul_off is None or now_off is None:
        return 0
    standard = min(int(jan_off.total_seconds()), int(jul_off.total_seconds()))
    return int(now_off.total_seconds()) - standard


def standard_seconds_from_midnight(local: datetime) -> int:
    """Local standard seconds since midnight, in 0..86399."""
    civil = local.hour * 3600 + local.minute * 60 + local.second
    return (civil - dst_advance_seconds(local)) % 86400


def period_index(standard_secs: float) -> int:
    """0 is midnight–03:25, 2 is 06:51–10:17, 6 is 20:34–midnight."""
    index = int(standard_secs // PERIOD_SECS)
    if index >= 7:
        return 6
    if index < 0:
        return 0
    return index


def chart_e_letter(sunday_index: int, period: int) -> str:
    """Chart E. Sunday's first period is G. Each later weekday starts three letters later."""
    start = (6 + 3 * (sunday_index % 7)) % 7
    return _LETTERS[(start + period) % 7]


def _hhmm(minutes: int) -> str:
    if minutes >= 1440:
        return "24:00"
    hour, minute = divmod(minutes, 60)
    return f"{hour:02d}:{minute:02d}"


def _lewis_span(index: int) -> str:
    """Printed Chart E bounds for this period. Classification does not use these."""
    return f"{_hhmm(LEWIS_MINUTES[index])}–{_hhmm(LEWIS_MINUTES[index + 1])} standard"


def period_row(sunday_index: int, index: int) -> dict:
    letter = chart_e_letter(sunday_index, index)
    start = index * PERIOD_SECS
    end = 86400.0 if index == 6 else (index + 1) * PERIOD_SECS
    return {
        "period": index + 1,
        "letter": letter,
        "planet": NOTE_PLANET[letter],
        "color": NOTE_NAME[letter],
        "color_hex": NOTE_HEX[letter],
        "color_rgb": list(NOTE_COLOR[letter]),
        "start_sec": start,
        "end_sec": end,
        "time": _lewis_span(index),
    }


def period_day(day: date, tz: str) -> dict:
    """One civil day's Chart E rows, in local standard time, plus that day's noon DST lead."""
    noon = datetime(day.year, day.month, day.day, 12, 0, tzinfo=ZoneInfo(tz))
    sunday = _PY_TO_SUNDAY[day.weekday()]
    return {
        "date": day.isoformat(),
        "weekday": day.strftime("%A"),
        "dst_advance_seconds": dst_advance_seconds(noon),
        "periods": [period_row(sunday, index) for index in range(7)],
    }


def period_days(day: date, tz: str) -> list[dict]:
    """Yesterday, today, and tomorrow, so a DST shift across midnight still has a letter."""
    return [period_day(day + timedelta(days=offset), tz) for offset in (-1, 0, 1)]


def daily_period(when: Instant, place: Place | None = None) -> dict:
    place = place or GREENWICH
    d, start, note = rc_day(when, place)
    local = _local_dt(when, place.tz)
    secs = standard_seconds_from_midnight(local)
    index = period_index(secs)
    sunday = _PY_TO_SUNDAY[d.weekday()]
    letter = chart_e_letter(sunday, index)
    return {
        "period": index + 1,
        "letter": letter,
        "time": _lewis_span(index),
        "weekday": d.strftime("%A"),
        "planet": NOTE_PLANET[letter],
        "color": NOTE_NAME[letter],
        "color_hex": NOTE_HEX[letter],
        "color_rgb": NOTE_COLOR[letter],
        "standard_seconds": secs,
        "dst_advance_seconds": dst_advance_seconds(local),
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
    """Heindel pigment for this Chart E letter, named, with its hex."""
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
        "equinox_start": format_instant(stamp.equinox),
        "next_equinox_start": format_instant(stamp.next_equinox),
        "cycle_1690": {
            "index": cyc.index,
            "start": format_instant(cyc.start),
            "end": format_instant(cyc.end),
            "elapsed_days": cyc.elapsed_days,
            "remaining_days": cyc.remaining_days,
            "length_days": cyc.length_days,
        },
        "daily": daily,
        "period_days": period_days(stamp.day_start.astimezone(ZoneInfo(place.tz)).date(), place.tz),
        "source": {
            "calendar": "AMORC: CE+1353; RC day and equinox year start at local midnight",
            "lewis": "H. Spencer Lewis, Self-Mastery and Fate with the Cycles of Life (1929), Chart E",
            "planets": "Julie Nordhaus-Bike, Rose+Croix Journal 16 (2022): A Sun through G Mars",
            "pigments": "Max Heindel, The Message of the Stars (1918), p. 64",
            "letters": "https://cycles.amorc.org/en/cycles (letter grid only)",
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
