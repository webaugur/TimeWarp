"""One civil day on several calendars.

Masonic Anno Lucis / Inventionis / Ordinis are year offsets (Mackey).
Scottish Rite Anno Mundi is the Hebrew year. Hebrew, tabular Hijri, and Coptic
are arithmetic day counts. Panchanga and Rosicrucian reuse existing modules.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date, datetime
from zoneinfo import ZoneInfo

from timewarp.cycle import GREENWICH, daily_period, rosicrucian_stamp
from timewarp.errors import TimeWarpError
from timewarp.iso import Instant, as_date, format_instant, weekday_name
from timewarp.panchanga import Panchanga, compute_panchanga
from timewarp.panchanga import format_quiet as panchanga_line
from timewarp.places import Place

# Rata Die: day 1 = 1 Jan 1 CE (proleptic Gregorian).
_HEBREW_EPOCH = -1373427  # RD of 1 Tishri, AM 1
# Tabular Islamic epoch: Thursday 16 July 622 CE (Julian) = JD 1948439.5
_HIJRI_EPOCH_RD = 227015  # RD of 1 Muharram AH 1 (civil/tabular)
_COPTIC_EPOCH = 103605  # RD of 1 Thout, year 1 (29 Aug 284 CE Julian)

_HEBREW_MONTHS = (
    "",
    "Nisan",
    "Iyar",
    "Sivan",
    "Tammuz",
    "Av",
    "Elul",
    "Tishri",
    "Heshvan",
    "Kislev",
    "Tevet",
    "Shevat",
    "Adar",
    "Adar II",
)
_HIJRI_MONTHS = (
    "",
    "Muharram",
    "Safar",
    "Rabi I",
    "Rabi II",
    "Jumada I",
    "Jumada II",
    "Rajab",
    "Shaban",
    "Ramadan",
    "Shawwal",
    "Dhu al-Qidah",
    "Dhu al-Hijjah",
)
_COPTIC_MONTHS = (
    "",
    "Thout",
    "Paopi",
    "Hathor",
    "Koiak",
    "Tobi",
    "Meshir",
    "Paremhat",
    "Parmouti",
    "Pashons",
    "Paoni",
    "Epip",
    "Mesori",
    "Epagomenal",
)
_FRENCH_MONTHS = (
    "Vendémiaire",
    "Brumaire",
    "Frimaire",
    "Nivôse",
    "Pluviôse",
    "Ventôse",
    "Germinal",
    "Floréal",
    "Prairial",
    "Messidor",
    "Thermidor",
    "Fructidor",
)
_FRENCH_SANS = ("Vertu", "Génie", "Travail", "Opinion", "Récompenses", "Révolution")
_EGYPTIAN_MONTHS = (
    "Thoth",
    "Phaophi",
    "Athyr",
    "Choiak",
    "Tybi",
    "Mechir",
    "Phamenoth",
    "Pharmuthi",
    "Pachon",
    "Payni",
    "Epiphi",
    "Mesore",
    "Epagomenal",
)


def _aware(when: Instant, place: Place) -> datetime:
    tz = ZoneInfo(place.tz)
    if isinstance(when, datetime):
        if when.tzinfo is None:
            return when.replace(tzinfo=tz)
        return when
    return datetime(when.year, when.month, when.day, 12, 0, tzinfo=tz)


def gregorian_to_rd(year: int, month: int, day: int) -> int:
    y = year - 1
    leap = year % 4 == 0 and (year % 100 != 0 or year % 400 == 0)
    return (
        365 * y
        + y // 4
        - y // 100
        + y // 400
        + (367 * month - 362) // 12
        + (0 if month <= 2 else (-1 if leap else -2))
        + day
    )


def _hebrew_leap(year: int) -> bool:
    return (7 * year + 1) % 19 < 7


def _hebrew_elapsed_days(year: int) -> int:
    """Days from the Hebrew epoch to the molad-adjusted 1 Tishri of `year`."""
    months = (235 * year - 234) // 19
    parts = 12084 + 13753 * months
    days = 29 * months + parts // 25920
    if (3 * (days + 1)) % 7 < 3:
        days += 1
    return days


def _hebrew_year_length(year: int) -> int:
    return _hebrew_elapsed_days(year + 1) - _hebrew_elapsed_days(year)


def _long_heshvan(year: int) -> bool:
    return _hebrew_year_length(year) % 10 == 5


def _short_kislev(year: int) -> bool:
    return _hebrew_year_length(year) % 10 == 3


def _hebrew_month_days(year: int, month: int) -> int:
    if month in (2, 4, 6, 10, 13):
        return 29
    if month == 12 and not _hebrew_leap(year):
        return 29
    if month == 8 and not _long_heshvan(year):
        return 29
    if month == 9 and _short_kislev(year):
        return 29
    return 30


def _tishri_rd(year: int) -> int:
    return _HEBREW_EPOCH + _hebrew_elapsed_days(year)


def hebrew_from_gregorian(day: date) -> tuple[int, int, int, str]:
    """Return (year, month 1=Nisan … 13=Adar II, day, month name)."""
    rd = gregorian_to_rd(day.year, day.month, day.day)
    year = (rd - _HEBREW_EPOCH) // 366 + 1
    while _tishri_rd(year) > rd:
        year -= 1
    while _tishri_rd(year + 1) <= rd:
        year += 1
    offset = rd - _tishri_rd(year)
    # Civil order starts at Tishri (7).
    order = [7, 8, 9, 10, 11, 12] + ([13] if _hebrew_leap(year) else []) + [1, 2, 3, 4, 5, 6]
    for month in order:
        length = _hebrew_month_days(year, month)
        if offset < length:
            return year, month, offset + 1, _HEBREW_MONTHS[month]
        offset -= length
    raise TimeWarpError(f"Hebrew date failed for {day.isoformat()}")


def hijri_from_gregorian(day: date) -> tuple[int, int, int, str]:
    """Tabular (civil) Hijri. Not a moon-sighting date."""
    rd = gregorian_to_rd(day.year, day.month, day.day)
    days = rd - _HIJRI_EPOCH_RD
    if days < 0:
        raise TimeWarpError("tabular Hijri is not defined before 16 July 622 CE")
    # 30-year cycle: 19 common (354) + 11 leap (355) = 10631 days.
    cycle, rem = divmod(days, 10631)
    year_in = 0
    for y in range(1, 31):
        length = 355 if y in (2, 5, 7, 10, 13, 16, 18, 21, 24, 26, 29) else 354
        if rem < length:
            break
        rem -= length
        year_in = y
    year = cycle * 30 + year_in + 1
    month = 1
    while month <= 12:
        length = 30 if month % 2 == 1 else 29
        if month == 12 and year_in + 1 in (2, 5, 7, 10, 13, 16, 18, 21, 24, 26, 29):
            length = 30
        if rem < length:
            return year, month, rem + 1, _HIJRI_MONTHS[month]
        rem -= length
        month += 1
    raise TimeWarpError(f"Hijri date failed for {day.isoformat()}")


def julian_jdn(year: int, month: int, day: int) -> int:
    a = (14 - month) // 12
    y = year + 4800 - a
    m = month + 12 * a - 3
    return day + (153 * m + 2) // 5 + 365 * y + y // 4 - 32083


def gregorian_jdn(year: int, month: int, day: int) -> int:
    a = (14 - month) // 12
    y = year + 4800 - a
    m = month + 12 * a - 3
    return day + (153 * m + 2) // 5 + 365 * y + y // 4 - y // 100 + y // 400 - 32045


def gregorian_from_jdn(jdn: int) -> date:
    a = jdn + 32044
    b = (4 * a + 3) // 146097
    c = a - (146097 * b) // 4
    d = (4 * c + 3) // 1461
    e = c - (1461 * d) // 4
    m = (5 * e + 2) // 153
    day = e - (153 * m + 2) // 5 + 1
    month = m + 3 - 12 * (m // 10)
    year = 100 * b + d - 4800 + (m // 10)
    return date(year, month, day)


def julian_from_gregorian(day: date) -> tuple[int, int, int]:
    """Proleptic Julian civil date for a proleptic Gregorian day."""
    jdn = gregorian_jdn(day.year, day.month, day.day)
    c = jdn + 32082
    e = (4 * c + 3) // 1461
    f = c - (1461 * e) // 4
    g = (5 * f + 2) // 153
    d = f - (153 * g + 2) // 5 + 1
    month = g + 3 - 12 * (g // 10)
    year = e - 4800 + (g // 10)
    return year, month, d


def french_from_gregorian(day: date) -> tuple[int, int, int, str]:
    """Arithmetic French Republican date.

    Epoch 22 Sep 1792 = 1 Vendémiaire I. Leap years are III, VII, XI, …
    (year % 4 == 3). This is the 4-year rule extended past year XIV, not the
    Paris autumn-equinox rule.
    """
    epoch = date(1792, 9, 22)
    if day < epoch:
        raise TimeWarpError("French Republican date is not defined before 22 September 1792")
    left = (day - epoch).days
    year = 1
    while True:
        length = 366 if year % 4 == 3 else 365
        if left < length:
            break
        left -= length
        year += 1
        if year > 20000:
            raise TimeWarpError(f"French Republican date failed for {day.isoformat()}")
    if left >= 360:
        return year, 13, left - 359, _FRENCH_SANS[left - 360]
    month = left // 30 + 1
    return year, month, left % 30 + 1, _FRENCH_MONTHS[month - 1]


# 1 Thoth, Nabonassar year 1 = 26 February 747 BCE (Julian). No leap day: drifts.
_NABONASSAR_JDN = julian_jdn(-746, 2, 26)


def egyptian_from_gregorian(day: date) -> tuple[int, int, int, str]:
    """Wandering Egyptian civil year (365 days). Nabonassar epoch."""
    days = gregorian_jdn(day.year, day.month, day.day) - _NABONASSAR_JDN
    if days < 0:
        raise TimeWarpError("Egyptian civil date is before the Nabonassar epoch")
    year = days // 365 + 1
    doy = days % 365
    if doy >= 360:
        return year, 13, doy - 359, _EGYPTIAN_MONTHS[12]
    month = doy // 30 + 1
    return year, month, doy % 30 + 1, _EGYPTIAN_MONTHS[month - 1]


def coptic_from_gregorian(day: date) -> tuple[int, int, int, str]:
    """Coptic calendar (Egyptian civil 12×30 + 5). Year 1 = 29 Aug 284 CE."""
    rd = gregorian_to_rd(day.year, day.month, day.day)
    days = rd - _COPTIC_EPOCH
    if days < 0:
        raise TimeWarpError("Coptic date is not defined before 29 August 284 CE")
    year = days // 365 + 1
    # leap day every 4th Coptic year, at the end (year % 4 == 3 in some counts)
    # CC: coptic year y is leap if y % 4 == 3. Days before year:
    before = 365 * (year - 1) + (year // 4)
    if days < before:
        year -= 1
        before = 365 * (year - 1) + (year // 4)
    doy = days - before
    if doy >= 360:
        return year, 13, doy - 359, _COPTIC_MONTHS[13]
    month = doy // 30 + 1
    day_n = doy % 30 + 1
    return year, month, day_n, _COPTIC_MONTHS[month]


@dataclass(frozen=True)
class ErasDay:
    when: datetime
    place: Place
    civil: date
    weekday: str
    iso_week: str
    julian_year: int
    julian_month: int
    julian_day: int
    french_year: int
    french_month: str
    french_day: int
    egyptian_year: int
    egyptian_month: str
    egyptian_day: int
    rc_stamp: str
    rc_letter: str
    rc_color: str
    anno_lucis: int
    anno_mundi: int
    anno_inventionis: int
    anno_ordinis: int
    hebrew_year: int
    hebrew_month: str
    hebrew_day: int
    hijri_year: int
    hijri_month: str
    hijri_day: int
    coptic_year: int
    coptic_month: str
    coptic_day: int
    panchanga: Panchanga

    def to_dict(self) -> dict:
        return {
            "when": format_instant(self.when),
            "place": self.place.name,
            "tz": self.place.tz,
            "gregorian": self.civil.isoformat(),
            "weekday": self.weekday,
            "iso_week": self.iso_week,
            "julian": {
                "year": self.julian_year,
                "month": self.julian_month,
                "day": self.julian_day,
            },
            "french_republican": {
                "year": self.french_year,
                "month": self.french_month,
                "day": self.french_day,
            },
            "egyptian_nabonassar": {
                "year": self.egyptian_year,
                "month": self.egyptian_month,
                "day": self.egyptian_day,
            },
            "rosicrucian": {
                "stamp": self.rc_stamp,
                "letter": self.rc_letter,
                "color": self.rc_color,
            },
            "anno_lucis": self.anno_lucis,
            "anno_mundi": self.anno_mundi,
            "anno_inventionis": self.anno_inventionis,
            "anno_ordinis": self.anno_ordinis,
            "hebrew": {
                "year": self.hebrew_year,
                "month": self.hebrew_month,
                "day": self.hebrew_day,
            },
            "hijri_tabular": {
                "year": self.hijri_year,
                "month": self.hijri_month,
                "day": self.hijri_day,
            },
            "coptic": {
                "year": self.coptic_year,
                "month": self.coptic_month,
                "day": self.coptic_day,
            },
            "panchanga": self.panchanga.to_dict(),
        }


def compute_eras(when: Instant, place: Place | None = None) -> ErasDay:
    loc = place or GREENWICH
    inst = _aware(when, loc)
    civil = as_date(inst)
    iso = civil.isocalendar()
    stamp = rosicrucian_stamp(inst, loc)
    daily = daily_period(inst, loc)
    hy, hm, hd, hname = hebrew_from_gregorian(civil)
    iy, im, iday, iname = hijri_from_gregorian(civil)
    cy, cm, cd, cname = coptic_from_gregorian(civil)
    jy, jm, jd = julian_from_gregorian(civil)
    fy, fm, fd, fname = french_from_gregorian(civil)
    ey, em, ed, ename = egyptian_from_gregorian(civil)
    return ErasDay(
        when=inst,
        place=loc,
        civil=civil,
        weekday=weekday_name(civil),
        iso_week=f"{iso.year:04d}-W{iso.week:02d}-{iso.weekday}",
        julian_year=jy,
        julian_month=jm,
        julian_day=jd,
        french_year=fy,
        french_month=fname,
        french_day=fd,
        egyptian_year=ey,
        egyptian_month=ename,
        egyptian_day=ed,
        rc_stamp=stamp.stamp(),
        rc_letter=daily["letter"],
        rc_color=daily["color"],
        anno_lucis=civil.year + 4000,
        anno_mundi=hy,
        anno_inventionis=civil.year + 530,
        anno_ordinis=civil.year - 1118,
        hebrew_year=hy,
        hebrew_month=hname,
        hebrew_day=hd,
        hijri_year=iy,
        hijri_month=iname,
        hijri_day=iday,
        coptic_year=cy,
        coptic_month=cname,
        coptic_day=cd,
        panchanga=compute_panchanga(inst, loc),
    )


def format_quiet(day: ErasDay) -> str:
    from timewarp.cherokee import cherokee_line
    from timewarp.maya import maya_from_gregorian

    p = day.panchanga
    return (
        f"{day.civil.isoformat()}  RC {day.rc_stamp}  "
        f"AL {day.anno_lucis}  AM {day.anno_mundi}  "
        f"{day.hebrew_day} {day.hebrew_month} {day.hebrew_year}  "
        f"{day.hijri_day} {day.hijri_month} {day.hijri_year}  "
        f"Jul {day.julian_year:04d}-{day.julian_month:02d}-{day.julian_day:02d}  "
        f"FR {day.french_day} {day.french_month} {day.french_year}  "
        f"Eg {day.egyptian_day} {day.egyptian_month} {day.egyptian_year}  "
        f"{day.coptic_day} {day.coptic_month} {day.coptic_year}  "
        f"{panchanga_line(p)} {p.yoga}  "
        f"Maya {maya_from_gregorian(day.civil).long_count()}  "
        f"Cherokee {cherokee_line(day.civil)}"
    )
