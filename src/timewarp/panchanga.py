"""Lunisolar daily date (tithi, paksha, masa, nakshatra).

The Mahabharata dates events with these Vedic elements. This module converts a
**modern** civil instant using Schlyter sun/moon and Lahiri ayanamsa. It is not
a war chronology and is not valid near 3100 BCE.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timedelta, timezone

from timewarp.chart import ayanamsa_deg, apply_frame
from timewarp.cycle import GREENWICH
from timewarp.ephem import julian_day, position, rev
from timewarp.iso import Instant, as_date, format_instant, weekday_name
from timewarp.native import beside, spelling
from timewarp.places import Place

# Convention only (not “the” Mahabharata epoch): 18 Feb 3102 BCE 00:00 TT ≈ JD 588465.5
KALI_JD = 588465.5
_DEG_PER_TITHI = 12.0
_NAK_SPAN = 360.0 / 27.0
_ELONG_RATE = 12.1907  # deg/day, mean

TITHI_NAMES = (
    "Pratipada",
    "Dwitiya",
    "Tritiya",
    "Chaturthi",
    "Panchami",
    "Shashthi",
    "Saptami",
    "Ashtami",
    "Navami",
    "Dashami",
    "Ekadashi",
    "Dwadashi",
    "Trayodashi",
    "Chaturdashi",
    "Purnima",
)

NAKSHATRAS = (
    "Ashvini",
    "Bharani",
    "Krittika",
    "Rohini",
    "Mrigashirsha",
    "Ardra",
    "Punarvasu",
    "Pushya",
    "Ashlesha",
    "Magha",
    "Purva Phalguni",
    "Uttara Phalguni",
    "Hasta",
    "Chitra",
    "Svati",
    "Vishakha",
    "Anuradha",
    "Jyeshtha",
    "Mula",
    "Purva Ashadha",
    "Uttara Ashadha",
    "Shravana",
    "Dhanishtha",
    "Shatabhisha",
    "Purva Bhadrapada",
    "Uttara Bhadrapada",
    "Revati",
)

# Lunar month named from the nakshatra of the full moon in that month.
NAK_TO_MASA = (
    "Ashvina",
    "Ashvina",
    "Kartika",
    "Kartika",
    "Margashirsha",
    "Margashirsha",
    "Pausha",
    "Pausha",
    "Magha",
    "Magha",
    "Phalguna",
    "Phalguna",
    "Chaitra",
    "Chaitra",
    "Vaishakha",
    "Vaishakha",
    "Jyeshtha",
    "Jyeshtha",
    "Ashadha",
    "Ashadha",
    "Ashadha",
    "Shravana",
    "Shravana",
    "Bhadrapada",
    "Bhadrapada",
    "Bhadrapada",
    "Ashvina",
)


def _aware(when: Instant, place: Place) -> datetime:
    from zoneinfo import ZoneInfo

    tz = ZoneInfo(place.tz)
    if isinstance(when, datetime):
        if when.tzinfo is None:
            return when.replace(tzinfo=tz)
        return when
    return datetime(when.year, when.month, when.day, 12, 0, tzinfo=tz)


def _sidereal_lons(when: datetime) -> tuple[float, float, float]:
    jd = julian_day(when)
    ayan = ayanamsa_deg(jd, "lahiri")
    sun = apply_frame(position("sun", when).ecl_lon, ayan)
    moon = apply_frame(position("moon", when).ecl_lon, ayan)
    return sun, moon, ayan


def elongation(when: datetime) -> float:
    sun, moon, _ = _sidereal_lons(when)
    return rev(moon - sun)


def tithi_from_elong(elong_deg: float) -> tuple[int, float, str, str]:
    e = rev(elong_deg)
    raw = e / _DEG_PER_TITHI
    num = int(raw) + 1
    if num > 30:
        num = 30
    frac = raw - int(raw)
    if num <= 15:
        paksha = "Shukla"
        name = TITHI_NAMES[num - 1] if num < 15 else "Purnima"
    else:
        paksha = "Krishna"
        k = num - 15
        name = TITHI_NAMES[k - 1] if k < 15 else "Amavasya"
    return num, frac, paksha, name


def nakshatra_from_lon(moon_sid: float) -> tuple[int, str, float]:
    x = rev(moon_sid)
    i = min(int(x / _NAK_SPAN), 26)
    frac = (x / _NAK_SPAN) - i
    return i + 1, NAKSHATRAS[i], frac


YOGAS = (
    "Vishkambha",
    "Priti",
    "Ayushman",
    "Saubhagya",
    "Shobhana",
    "Atiganda",
    "Sukarma",
    "Dhriti",
    "Shula",
    "Ganda",
    "Vriddhi",
    "Dhruva",
    "Vyaghata",
    "Harshana",
    "Vajra",
    "Siddhi",
    "Vyatipata",
    "Variyan",
    "Parigha",
    "Shiva",
    "Siddha",
    "Sadhya",
    "Shubha",
    "Shukla",
    "Brahma",
    "Indra",
    "Vaidhriti",
)

# Devanagari beside the English tokens above.
# Tithi मूल नाम: https://hi.wikipedia.org/wiki/तिथि (पंचमी is that page's spelling).
# Paksha: शुक्ल / कृष्ण on the same page.
# Nakshatra names: https://en.wikipedia.org/wiki/Nakshatra
# Masa: Marathi column of https://en.wikipedia.org/wiki/Hindu_calendar
# Vara: Hindi column of that page's weekday table.
# Yoga: Dharmawiki विष्कम्भादि table. Siddha is सिद्ध (the later name for the
# second सिद्धि). Indra is the table's ऐन्द्र.
_TITHI_NATIVE = {
    "Pratipada": "प्रतिपदा",
    "Dwitiya": "द्वितीया",
    "Tritiya": "तृतीया",
    "Chaturthi": "चतुर्थी",
    "Panchami": "पंचमी",
    "Shashthi": "षष्ठी",
    "Saptami": "सप्तमी",
    "Ashtami": "अष्टमी",
    "Navami": "नवमी",
    "Dashami": "दशमी",
    "Ekadashi": "एकादशी",
    "Dwadashi": "द्वादशी",
    "Trayodashi": "त्रयोदशी",
    "Chaturdashi": "चतुर्दशी",
    "Purnima": "पूर्णिमा",
    "Amavasya": "अमावस्या",
}
_PAKSHA_NATIVE = {
    "Shukla": "शुक्ल",
    "Krishna": "कृष्ण",
}
_NAKSHATRA_NATIVE = {
    "Ashvini": "अश्विनी",
    "Bharani": "भरणी",
    "Krittika": "कृत्तिका",
    "Rohini": "रोहिणी",
    "Mrigashirsha": "मृगशीर्षा",
    "Ardra": "आर्द्रा",
    "Punarvasu": "पुनर्वसु",
    "Pushya": "पुष्य",
    "Ashlesha": "आश्लेषा",
    "Magha": "मघा",
    "Purva Phalguni": "पूर्व फाल्गुनी",
    "Uttara Phalguni": "उत्तर फाल्गुनी",
    "Hasta": "हस्त",
    "Chitra": "चित्रा",
    "Svati": "स्वाति",
    "Vishakha": "विशाखा",
    "Anuradha": "अनुराधा",
    "Jyeshtha": "ज्येष्ठा",
    "Mula": "मूल",
    "Purva Ashadha": "पूर्वाषाढ़ा",
    "Uttara Ashadha": "उत्तराषाढ़ा",
    "Shravana": "श्रवण",
    "Dhanishtha": "धनिष्ठा",
    "Shatabhisha": "शतभिषा",
    "Purva Bhadrapada": "पूर्वभाद्रपदा",
    "Uttara Bhadrapada": "उत्तरभाद्रपदा",
    "Revati": "रेवती",
}
_MASA_NATIVE = {
    "Chaitra": "चैत्र",
    "Vaishakha": "वैशाख",
    "Jyeshtha": "ज्येष्ठ",
    "Ashadha": "आषाढ",
    "Shravana": "श्रावण",
    "Bhadrapada": "भाद्रपद",
    "Ashvina": "आश्विन",
    "Kartika": "कार्तिक",
    "Margashirsha": "मार्गशीर्ष",
    "Pausha": "पौष",
    "Magha": "माघ",
    "Phalguna": "फाल्गुण",
}
_YOGA_NATIVE = {
    "Vishkambha": "विष्कम्भ",
    "Priti": "प्रीति",
    "Ayushman": "आयुष्मान्",
    "Saubhagya": "सौभाग्य",
    "Shobhana": "शोभन",
    "Atiganda": "अतिगण्ड",
    "Sukarma": "सुकर्मा",
    "Dhriti": "धृति",
    "Shula": "शूल",
    "Ganda": "गण्ड",
    "Vriddhi": "वृद्धि",
    "Dhruva": "ध्रुव",
    "Vyaghata": "व्याघात",
    "Harshana": "हर्षण",
    "Vajra": "वज्र",
    "Siddhi": "सिद्धि",
    "Vyatipata": "व्यतीपात",
    "Variyan": "वरीयान्",
    "Parigha": "परिघ",
    "Shiva": "शिव",
    "Siddha": "सिद्ध",
    "Sadhya": "साध्य",
    "Shubha": "शुभ",
    "Shukla": "शुक्ल",
    "Brahma": "ब्रह्मा",
    "Indra": "ऐन्द्र",
    "Vaidhriti": "वैधृति",
}
_VARA_NATIVE = {
    "Sunday": "रविवार",
    "Monday": "सोमवार",
    "Tuesday": "मंगलवार",
    "Wednesday": "बुधवार",
    "Thursday": "गुरुवार",
    "Friday": "शुक्रवार",
    "Saturday": "शनिवार",
}


def tithi_native(name: str) -> str:
    return spelling(_TITHI_NATIVE, name, "Devanagari")


def paksha_native(name: str) -> str:
    return spelling(_PAKSHA_NATIVE, name, "Devanagari")


def nakshatra_native(name: str) -> str:
    return spelling(_NAKSHATRA_NATIVE, name, "Devanagari")


def masa_native(name: str) -> str:
    return spelling(_MASA_NATIVE, name, "Devanagari")


def yoga_native(name: str) -> str:
    return spelling(_YOGA_NATIVE, name, "Devanagari")


def weekday_native(name: str) -> str:
    return spelling(_VARA_NATIVE, name, "Devanagari")


def yoga_from_lons(sun_sid: float, moon_sid: float) -> tuple[int, str, float]:
    """Nitya yoga: (sidereal Sun + Moon) / (360°/27)."""
    x = rev(sun_sid + moon_sid)
    i = min(int(x / _NAK_SPAN), 26)
    frac = (x / _NAK_SPAN) - i
    return i + 1, YOGAS[i], frac


def _when_elong(start: datetime, target: float, *, backward: bool) -> datetime:
    t = start
    for _ in range(12):
        e = elongation(t)
        if backward:
            de = (e - target) % 360.0
            if de < 0.02 or de > 359.98:
                break
            t = t - timedelta(days=de / _ELONG_RATE)
        else:
            de = (target - e) % 360.0
            if de < 0.02 or de > 359.98:
                break
            t = t + timedelta(days=de / _ELONG_RATE)
    return t


def masa_name(when: datetime, *, purnimanta: bool) -> str:
    e = elongation(when)
    shukla = e < 180.0
    if shukla or purnimanta:
        t_purnima = _when_elong(when, 180.0, backward=False)
    else:
        t_purnima = _when_elong(when, 180.0, backward=True)
    _sun, moon, _ayan = _sidereal_lons(t_purnima)
    idx, _name, _f = nakshatra_from_lon(moon)
    return NAK_TO_MASA[idx - 1]


def kali_year(when: datetime) -> tuple[int, float]:
    jd = julian_day(when)
    ahargana = jd - KALI_JD
    year = int(ahargana / 365.2425) + 1
    return year, ahargana


@dataclass(frozen=True)
class Panchanga:
    when: datetime
    place: Place
    elong: float
    tithi: int
    tithi_name: str
    tithi_frac: float
    paksha: str
    nakshatra: str
    nakshatra_n: int
    nakshatra_frac: float
    yoga: str
    yoga_n: int
    yoga_frac: float
    masa: str
    purnimanta: bool
    weekday: str
    kali_year: int
    kali_ahargana: float
    ayanamsa: float
    sun_sid: float
    moon_sid: float

    def to_dict(self) -> dict:
        return {
            "when": format_instant(self.when),
            "place": self.place.name,
            "tz": self.place.tz,
            "elongation_deg": round(self.elong, 4),
            "tithi": self.tithi,
            "tithi_name": self.tithi_name,
            "tithi_name_native": tithi_native(self.tithi_name),
            "tithi_elapsed": round(self.tithi_frac, 4),
            "paksha": self.paksha,
            "paksha_native": paksha_native(self.paksha),
            "nakshatra": self.nakshatra,
            "nakshatra_native": nakshatra_native(self.nakshatra),
            "nakshatra_n": self.nakshatra_n,
            "nakshatra_elapsed": round(self.nakshatra_frac, 4),
            "yoga": self.yoga,
            "yoga_native": yoga_native(self.yoga),
            "yoga_n": self.yoga_n,
            "yoga_elapsed": round(self.yoga_frac, 4),
            "masa": self.masa,
            "masa_native": masa_native(self.masa),
            "month_system": "purnimanta" if self.purnimanta else "amanta",
            "weekday": self.weekday,
            "weekday_native": weekday_native(self.weekday),
            "kali_year": self.kali_year,
            "kali_ahargana": round(self.kali_ahargana, 4),
            "ayanamsa_lahiri": round(self.ayanamsa, 4),
            "sun_sidereal": round(self.sun_sid, 4),
            "moon_sidereal": round(self.moon_sid, 4),
            "note": "Schlyter + Lahiri; daily use in modern centuries, not Mahabharata-era sky",
        }


def compute_panchanga(
    when: Instant,
    place: Place | None = None,
    *,
    purnimanta: bool = False,
) -> Panchanga:
    loc = place or GREENWICH
    inst = _aware(when, loc)
    sun, moon, ayan = _sidereal_lons(inst)
    e = rev(moon - sun)
    tithi, frac, paksha, tname = tithi_from_elong(e)
    n_i, n_name, n_frac = nakshatra_from_lon(moon)
    y_i, y_name, y_frac = yoga_from_lons(sun, moon)
    masa = masa_name(inst, purnimanta=purnimanta)
    ky, ah = kali_year(inst)
    return Panchanga(
        when=inst,
        place=loc,
        elong=e,
        tithi=tithi,
        tithi_name=tname,
        tithi_frac=frac,
        paksha=paksha,
        nakshatra=n_name,
        nakshatra_n=n_i,
        nakshatra_frac=n_frac,
        yoga=y_name,
        yoga_n=y_i,
        yoga_frac=y_frac,
        masa=masa,
        purnimanta=purnimanta,
        weekday=weekday_name(as_date(inst)),
        kali_year=ky,
        kali_ahargana=ah,
        ayanamsa=ayan,
        sun_sid=sun,
        moon_sid=moon,
    )


def format_quiet(p: Panchanga) -> str:
    return (
        f"{beside(p.masa, masa_native(p.masa))} "
        f"{beside(p.paksha, paksha_native(p.paksha))} "
        f"{beside(p.tithi_name, tithi_native(p.tithi_name))} "
        f"{beside(p.nakshatra, nakshatra_native(p.nakshatra))}"
    )


def format_with_yoga(p: Panchanga) -> str:
    """Quiet panchanga line plus the nitya yoga, English and Devanagari."""
    return f"{format_quiet(p)}  yoga {beside(p.yoga, yoga_native(p.yoga))}"


def explain(p: Panchanga) -> list[str]:
    return [
        f"Sidereal (Lahiri {p.ayanamsa:.2f}°) Sun {p.sun_sid:.2f}°, Moon {p.moon_sid:.2f}°.",
        f"Moon−Sun elongation {p.elong:.2f}° → tithi {p.tithi} {p.paksha} {p.tithi_name} "
        f"({p.tithi_frac:.0%} elapsed).",
        f"Moon in {p.nakshatra} ({p.nakshatra_n}/27, {p.nakshatra_frac:.0%} elapsed).",
        f"Yoga {p.yoga_n} {p.yoga} ({p.yoga_frac:.0%} elapsed; Sun+Moon).",
        f"Lunar month {p.masa} ({'purnimanta' if p.purnimanta else 'amanta'}, named from full-moon nakshatra).",
        "Schlyter planets; not Drik Panchang and not a Mahabharata war date.",
    ]
