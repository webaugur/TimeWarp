"""Western (Gregorian) liturgical day and the ancient Roman day-count.

Computus is the Anonymous Gregorian algorithm. Ascension is the Thursday
(+39). This is not the 1962 calendar, not the full sanctoral cycle, and not
Orthodox Easter.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date, datetime, timedelta

from timewarp.errors import TimeWarpError
from timewarp.iso import Instant, as_date, weekday_name

_ROMAN_NUM = (
    "",
    "I",
    "II",
    "III",
    "IV",
    "V",
    "VI",
    "VII",
    "VIII",
    "IX",
    "X",
    "XI",
    "XII",
    "XIII",
    "XIV",
    "XV",
    "XVI",
    "XVII",
    "XVIII",
    "XIX",
)
_MONTHS = (
    "",
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
)
_ABBR = (
    "",
    "Ian.",
    "Feb.",
    "Mar.",
    "Apr.",
    "Mai.",
    "Iun.",
    "Iul.",
    "Aug.",
    "Sep.",
    "Oct.",
    "Nov.",
    "Dec.",
)
# March, May, July, October: Nones on the 7th, Ides on the 15th.
_LONG = frozenset({3, 5, 7, 10})

_SOLEMNITIES = {
    (12, 25): "Christmas",
    (1, 6): "Epiphany",
    (3, 25): "Annunciation",
    (8, 15): "Assumption",
    (11, 1): "All Saints",
    (12, 8): "Immaculate Conception",
}


def easter_sunday(year: int) -> date:
    """Gregorian Easter Sunday (Anonymous Gregorian computus)."""
    a = year % 19
    b = year // 100
    c = year % 100
    d = b // 4
    e = b % 4
    f = (b + 8) // 25
    g = (b - f + 1) // 3
    h = (19 * a + b - d - g + 15) % 30
    i = c // 4
    k = c % 4
    ell = (32 + 2 * e + 2 * i - h - k) % 7
    m = (a + 11 * h + 22 * ell) // 451
    month = (h + ell - 7 * m + 114) // 31
    day = ((h + ell - 7 * m + 114) % 31) + 1
    return date(year, month, day)


def _advent_start(year: int) -> date:
    """Sunday on or after 27 November."""
    nov27 = date(year, 11, 27)
    return nov27 + timedelta(days=(6 - nov27.weekday()) % 7)


def _baptism(year: int) -> date:
    """Sunday after 6 January, or 6 January when that day is Sunday."""
    epiphany = date(year, 1, 6)
    if epiphany.weekday() == 6:
        return epiphany
    return epiphany + timedelta(days=(6 - epiphany.weekday()) % 7)


def julian_easter(year: int) -> tuple[date, date]:
    """Orthodox Easter: (Julian date, Gregorian date of that same Sunday).

    Meeus Julian computus. The Julian month/day is then shifted onto the
    Gregorian civil calendar. It is not mixed into the Western season.
    """
    from timewarp.eras import gregorian_from_jdn, julian_jdn

    a = year % 4
    b = year % 7
    c = year % 19
    d = (19 * c + 15) % 30
    e = (2 * a + 4 * b - d + 34) % 7
    month = (d + e + 114) // 31
    day = ((d + e + 114) % 31) + 1
    greg = gregorian_from_jdn(julian_jdn(year, month, day))
    return date(year, month, day), greg


def liturgical_season(day: date, *, calendar: str = "1970") -> str:
    easter = easter_sunday(day.year)
    ash = easter - timedelta(days=46)
    holy_thu = easter - timedelta(days=3)
    pentecost = easter + timedelta(days=49)
    advent = _advent_start(day.year)
    baptism = _baptism(day.year)
    christmas = date(day.year, 12, 25)
    if ash <= day < holy_thu:
        return "Lent"
    if holy_thu <= day < easter:
        return "Paschal Triduum"
    if easter <= day <= pentecost:
        return "Easter"
    if day >= christmas:
        return "Christmas"
    if day >= advent:
        return "Advent"
    if day <= baptism:
        return "Christmas"
    if calendar == "1962":
        septuagesima = easter - timedelta(days=63)
        if septuagesima <= day < ash:
            return "Septuagesima"
        if day < septuagesima:
            return "After Epiphany"
        return "After Pentecost"
    return "Ordinary Time"


def solemnity(day: date) -> str | None:
    return _SOLEMNITIES.get((day.month, day.day))


# Principal days of the General Roman Calendar: (month, day, en, la, rank, when).
# when is both, 1970, or 1962. Not every optional memorial in the martyrology.
_SANCTORAL: tuple[tuple[int, int, str, str, str, str], ...] = (
    (1, 1, "Mary, Mother of God", "Sancta Maria Dei Genetrix", "solemnity", "1970"),
    (1, 1, "Circumcision of the Lord", "Circumcisio Domini", "solemnity", "1962"),
    (1, 2, "Basil and Gregory Nazianzen", "Basilii et Gregorii Nazianzeni", "memorial", "both"),
    (1, 6, "Epiphany", "Epiphania Domini", "solemnity", "both"),
    (1, 21, "Agnes", "Agnetis", "memorial", "both"),
    (1, 24, "Francis de Sales", "Francisci de Sales", "memorial", "both"),
    (1, 25, "Conversion of Paul", "Conversionis Pauli", "feast", "both"),
    (1, 26, "Timothy and Titus", "Timothei et Titi", "memorial", "both"),
    (1, 28, "Thomas Aquinas", "Thomae de Aquino", "memorial", "both"),
    (2, 2, "Presentation of the Lord", "Praesentationis Domini", "feast", "both"),
    (2, 22, "Chair of Peter", "Cathedrae Petri", "feast", "both"),
    (3, 7, "Perpetua and Felicity", "Perpetuae et Felicitatis", "memorial", "both"),
    (3, 19, "Joseph", "Ioseph", "solemnity", "both"),
    (3, 25, "Annunciation", "Annuntiatio Domini", "solemnity", "both"),
    (4, 25, "Mark", "Marci", "feast", "both"),
    (4, 29, "Catherine of Siena", "Catharinae Senensis", "memorial", "both"),
    (5, 1, "Joseph the Worker", "Ioseph Opificis", "feast", "1962"),
    (5, 1, "Joseph the Worker", "Ioseph Opificis", "optional", "1970"),
    (5, 3, "Philip and James", "Philippi et Iacobi", "feast", "both"),
    (5, 14, "Matthias", "Matthiae", "feast", "both"),
    (5, 31, "Visitation", "Visitationis", "feast", "both"),
    (6, 1, "Justin", "Iustini", "memorial", "both"),
    (6, 11, "Barnabas", "Barnabae", "memorial", "both"),
    (6, 13, "Anthony of Padua", "Antonii de Padova", "memorial", "both"),
    (6, 24, "Nativity of John the Baptist", "Nativitas Ioannis Baptistae", "solemnity", "both"),
    (6, 29, "Peter and Paul", "Petri et Pauli", "solemnity", "both"),
    (7, 1, "Precious Blood", "Pretiosissimi Sanguinis", "feast", "1962"),
    (7, 3, "Thomas", "Thomae", "feast", "both"),
    (7, 11, "Benedict", "Benedicti", "memorial", "both"),
    (7, 22, "Mary Magdalene", "Mariae Magdalenae", "feast", "both"),
    (7, 26, "Joachim and Anne", "Ioachim et Annae", "memorial", "both"),
    (8, 6, "Transfiguration", "Transfiguratio Domini", "feast", "both"),
    (8, 10, "Lawrence", "Laurentii", "feast", "both"),
    (8, 11, "Clare", "Clarae", "memorial", "both"),
    (8, 14, "Maximilian Kolbe", "Maximiliani Kolbe", "memorial", "1970"),
    (8, 15, "Assumption", "Assumptio Mariae", "solemnity", "both"),
    (8, 24, "Bartholomew", "Bartholomaei", "feast", "both"),
    (8, 27, "Monica", "Monicae", "memorial", "both"),
    (8, 28, "Augustine", "Augustini", "memorial", "both"),
    (8, 29, "Passion of John the Baptist", "Passio Ioannis Baptistae", "memorial", "both"),
    (9, 3, "Gregory the Great", "Gregorii Magni", "memorial", "both"),
    (9, 8, "Nativity of Mary", "Nativitas Mariae", "feast", "both"),
    (9, 13, "John Chrysostom", "Ioannis Chrysostomi", "memorial", "both"),
    (9, 14, "Exaltation of the Cross", "Exaltatio Crucis", "feast", "both"),
    (9, 16, "Cornelius and Cyprian", "Cornelii et Cypriani", "memorial", "both"),
    (9, 21, "Matthew", "Matthaei", "feast", "both"),
    (9, 29, "Michael, Gabriel, and Raphael", "Michaelis, Gabrielis et Raphaelis", "feast", "both"),
    (9, 30, "Jerome", "Hieronymi", "memorial", "both"),
    (10, 4, "Francis of Assisi", "Francisci Assisiensis", "memorial", "both"),
    (10, 15, "Teresa of Avila", "Teresiae de Avila", "memorial", "both"),
    (10, 17, "Ignatius of Antioch", "Ignatii Antiocheni", "memorial", "both"),
    (10, 18, "Luke", "Lucae", "feast", "both"),
    (10, 28, "Simon and Jude", "Simonis et Iudae", "feast", "both"),
    (11, 1, "All Saints", "Omnium Sanctorum", "solemnity", "both"),
    (11, 2, "All Souls", "Omnium Fidelium Defunctorum", "solemnity", "both"),
    (11, 9, "Dedication of the Lateran", "Dedicatio Basilicae Lateranensis", "feast", "both"),
    (11, 10, "Leo the Great", "Leonis Magni", "memorial", "both"),
    (11, 11, "Martin of Tours", "Martini", "memorial", "both"),
    (11, 30, "Andrew", "Andreae", "feast", "both"),
    (12, 7, "Ambrose", "Ambrosii", "memorial", "both"),
    (12, 8, "Immaculate Conception", "Immaculata Conceptio", "solemnity", "both"),
    (12, 13, "Lucy", "Luciae", "memorial", "both"),
    (12, 14, "John of the Cross", "Ioannis a Cruce", "memorial", "both"),
    (12, 25, "Christmas", "Nativitas Domini", "solemnity", "both"),
    (12, 26, "Stephen", "Stephani", "feast", "both"),
    (12, 27, "John", "Ioannis", "feast", "both"),
    (12, 28, "Holy Innocents", "Sanctorum Innocentium", "feast", "both"),
)

_RANK_ORDER = {"solemnity": 0, "feast": 1, "memorial": 2, "optional": 3}


def _following_weekday(day: date, weekday: int) -> date:
    """Next date with Monday=0 … Sunday=6, strictly after `day`."""
    delta = (weekday - day.weekday()) % 7
    if delta == 0:
        delta = 7
    return day + timedelta(days=delta)


def ember_dates(year: int) -> set[date]:
    """Wednesday, Friday, and Saturday of the four traditional Ember weeks."""
    easter = easter_sunday(year)
    out: set[date] = set()
    for anchor in (
        date(year, 12, 13),
        easter - timedelta(days=42),
        date(year, 9, 14),
    ):
        wed = _following_weekday(anchor, 2)
        out.update({wed, wed + timedelta(days=2), wed + timedelta(days=3)})
    pentecost = easter + timedelta(days=49)
    wed = pentecost + timedelta(days=3)
    out.update({wed, wed + timedelta(days=2), wed + timedelta(days=3)})
    return out


def christ_the_king(year: int, *, calendar: str) -> date:
    if calendar == "1962":
        oct31 = date(year, 10, 31)
        return oct31 - timedelta(days=(oct31.weekday() + 1) % 7)
    nov20 = date(year, 11, 20)
    return nov20 + timedelta(days=(6 - nov20.weekday()) % 7)


def _holy_family(year: int) -> date:
    christmas = date(year, 12, 25)
    if christmas.weekday() == 6:
        return date(year, 12, 30)
    nxt = christmas + timedelta(days=(6 - christmas.weekday()) % 7)
    if nxt == christmas:
        return date(year, 12, 30)
    return nxt


@dataclass(frozen=True)
class Observance:
    en: str
    la: str
    rank: str


def observances_on(day: date, *, calendar: str = "1970") -> list[Observance]:
    found: list[Observance] = []
    easter = easter_sunday(day.year)
    movable = {
        easter: ("Easter", "Dominica Resurrectionis", "solemnity"),
        easter - timedelta(days=46): ("Ash Wednesday", "Feria IV Cinerum", "solemnity"),
        easter - timedelta(days=7): ("Palm Sunday", "Dominica in Palmis", "solemnity"),
        easter - timedelta(days=3): ("Holy Thursday", "Feria V in Cena Domini", "solemnity"),
        easter - timedelta(days=2): ("Good Friday", "Feria VI in Passione Domini", "solemnity"),
        easter - timedelta(days=1): ("Holy Saturday", "Sabbato Sancto", "solemnity"),
        easter + timedelta(days=39): ("Ascension", "Ascensio Domini", "solemnity"),
        easter + timedelta(days=49): ("Pentecost", "Pentecoste", "solemnity"),
        easter + timedelta(days=56): ("Trinity Sunday", "Sanctissimae Trinitatis", "solemnity"),
        easter + timedelta(days=60): ("Corpus Christi", "Corpus Christi", "solemnity"),
        easter + timedelta(days=68): ("Sacred Heart", "Sacratissimi Cordis Iesu", "solemnity"),
        christ_the_king(day.year, calendar=calendar): ("Christ the King", "Christi Regis", "solemnity"),
        _holy_family(day.year): ("Holy Family", "Sanctae Familiae", "feast"),
        _baptism(day.year) if calendar != "1962" else date(day.year, 1, 13): (
            "Baptism of the Lord",
            "Baptisma Domini",
            "feast",
        ),
    }
    hit = movable.get(day)
    if hit:
        found.append(Observance(*hit))
    for month, dom, en, la, rank, when in _SANCTORAL:
        if month == day.month and dom == day.day and when in (calendar, "both"):
            found.append(Observance(en, la, rank))
    return found


def select_observance(day: date, *, calendar: str = "1970") -> tuple[Observance | None, list[Observance]]:
    """Highest-ranking observance. Privileged Sundays outrank saint feasts and memorials."""
    cands = observances_on(day, calendar=calendar)
    season = liturgical_season(day, calendar=calendar)
    sunday = day.weekday() == 6
    privileged = sunday and season in ("Advent", "Lent", "Easter", "Septuagesima")
    ordinary_sunday = sunday and season in ("Ordinary Time", "After Epiphany", "After Pentecost", "Christmas")
    if privileged:
        cands = [c for c in cands if c.rank == "solemnity"]
    elif ordinary_sunday:
        kept = [c for c in cands if c.rank == "solemnity"]
        cands = kept
    cands.sort(key=lambda c: _RANK_ORDER.get(c.rank, 9))
    if not cands:
        return None, []
    return cands[0], cands[1:]


def _nones(month: int) -> int:
    return 7 if month in _LONG else 5


def _ides(month: int) -> int:
    return 15 if month in _LONG else 13


def _next_month(month: int) -> int:
    return 1 if month == 12 else month + 1


def roman_date(day: date) -> tuple[str, str]:
    """Traditional count and a plain English line. Inclusive Roman numbering."""
    month = day.month
    n = day.day
    nones = _nones(month)
    ides = _ides(month)
    if n == 1:
        return f"Kal. {_ABBR[month]}", f"Kalends of {_MONTHS[month]}"
    if n == nones:
        return f"Non. {_ABBR[month]}", f"Nones of {_MONTHS[month]}"
    if n == ides:
        return f"Id. {_ABBR[month]}", f"Ides of {_MONTHS[month]}"
    if n < nones:
        count = nones - n + 1
        kind, name, mon = "Non.", "Nones", month
    elif n < ides:
        count = ides - n + 1
        kind, name, mon = "Id.", "Ides", month
    else:
        if month == 12:
            last = 31
        else:
            last = (date(day.year, month + 1, 1) - timedelta(days=1)).day
        count = (last - n + 1) + 1
        kind, name, mon = "Kal.", "Kalends", _next_month(month)
    if count == 2:
        abbr = f"prid. {kind} {_ABBR[mon]}"
        plain = f"the day before the {name} of {_MONTHS[mon]}"
    else:
        abbr = f"a.d. {_ROMAN_NUM[count]} {kind} {_ABBR[mon]}"
        plain = f"{count}th day before the {name} of {_MONTHS[mon]} (counted inclusively)"
    return abbr, plain


@dataclass(frozen=True)
class LiturgyDay:
    day: date
    weekday: str
    season: str
    feast: str | None
    rank: str | None
    also: tuple[str, ...]
    ember: bool
    calendar: str
    easter: date
    orthodox_easter: date | None
    orthodox_easter_julian: str | None
    ash_wednesday: date
    palm_sunday: date
    holy_thursday: date
    good_friday: date
    holy_saturday: date
    ascension: date
    pentecost: date
    trinity: date
    corpus_christi: date
    roman_abbr: str
    roman_plain: str
    auc: int

    def to_dict(self) -> dict:
        def iso(d: date) -> str:
            return d.isoformat()

        return {
            "date": iso(self.day),
            "weekday": self.weekday,
            "season": self.season,
            "feast": self.feast,
            "rank": self.rank,
            "also": list(self.also),
            "ember": self.ember,
            "calendar": self.calendar,
            "easter": iso(self.easter),
            "orthodox_easter": iso(self.orthodox_easter) if self.orthodox_easter else None,
            "orthodox_easter_julian": self.orthodox_easter_julian,
            "ash_wednesday": iso(self.ash_wednesday),
            "palm_sunday": iso(self.palm_sunday),
            "holy_thursday": iso(self.holy_thursday),
            "good_friday": iso(self.good_friday),
            "holy_saturday": iso(self.holy_saturday),
            "ascension": iso(self.ascension),
            "pentecost": iso(self.pentecost),
            "trinity": iso(self.trinity),
            "corpus_christi": iso(self.corpus_christi),
            "roman": self.roman_abbr,
            "roman_plain": self.roman_plain,
            "auc": self.auc,
            "note": "Gregorian season. --orthodox adds Julian computus beside it. --calendar 1962 renames the seasons.",
        }


def compute_liturgy(
    when: Instant,
    *,
    calendar: str = "1970",
    lang: str = "en",
    orthodox: bool = False,
) -> LiturgyDay:
    if calendar not in ("1970", "1962"):
        raise TimeWarpError(f"unknown calendar {calendar!r}; use 1970 or 1962")
    if lang not in ("en", "la"):
        raise TimeWarpError(f"unknown language {lang!r}; use en or la")
    day = as_date(when)
    easter = easter_sunday(day.year)
    abbr, plain = roman_date(day)
    primary, rest = select_observance(day, calendar=calendar)
    if primary is None:
        feast = None
        rank = None
    else:
        feast = primary.la if lang == "la" else primary.en
        rank = primary.rank
    also = tuple((item.la if lang == "la" else item.en) for item in rest)
    orth_g = orth_j = None
    if orthodox:
        j_date, g_date = julian_easter(day.year)
        orth_g = g_date
        orth_j = f"{j_date.year:04d}-{j_date.month:02d}-{j_date.day:02d} Julian"
    return LiturgyDay(
        day=day,
        weekday=weekday_name(day),
        season=liturgical_season(day, calendar=calendar),
        feast=feast,
        rank=rank,
        also=also,
        ember=day in ember_dates(day.year),
        calendar=calendar,
        easter=easter,
        orthodox_easter=orth_g,
        orthodox_easter_julian=orth_j,
        ash_wednesday=easter - timedelta(days=46),
        palm_sunday=easter - timedelta(days=7),
        holy_thursday=easter - timedelta(days=3),
        good_friday=easter - timedelta(days=2),
        holy_saturday=easter - timedelta(days=1),
        ascension=easter + timedelta(days=39),
        pentecost=easter + timedelta(days=49),
        trinity=easter + timedelta(days=56),
        corpus_christi=easter + timedelta(days=60),
        roman_abbr=abbr,
        roman_plain=plain,
        auc=day.year + 753,
    )


def format_quiet(day: LiturgyDay) -> str:
    feast = f"  {day.feast}" if day.feast else ""
    ember = "  Ember" if day.ember else ""
    orth = f"  Orthodox Easter {day.orthodox_easter.isoformat()}" if day.orthodox_easter else ""
    return (
        f"{day.day.isoformat()}  {day.calendar}  {day.season}{feast}{ember}  "
        f"Easter {day.easter.isoformat()}{orth}  {day.roman_abbr}"
    )
