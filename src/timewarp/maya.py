"""Maya civil day-count. GMT correlation 584283.

11 August 3114 BCE (proleptic Gregorian) = Long Count 0.0.0.0.0.
That correlation is a convention, not the only one ever proposed.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date

from timewarp.eras import gregorian_jdn
from timewarp.errors import TimeWarpError
from timewarp.native import beside

# JDN = long-count days + GMT.
GMT = 584283

_TZOLKIN = (
    "Imix",
    "Ik",
    "Akbal",
    "Kan",
    "Chicchan",
    "Kimi",
    "Manik",
    "Lamat",
    "Muluk",
    "Ok",
    "Chuwen",
    "Eb",
    "Ben",
    "Ix",
    "Men",
    "Kib",
    "Kaban",
    "Etznab",
    "Kawak",
    "Ajaw",
)
_HAAB = (
    "Pop",
    "Wo",
    "Sip",
    "Sotz",
    "Sek",
    "Xul",
    "Yaxkin",
    "Mol",
    "Chen",
    "Yax",
    "Sak",
    "Keh",
    "Mak",
    "Kankin",
    "Muwan",
    "Pax",
    "Kayab",
    "Kumku",
    "Wayeb",
)

# Unicode Mayan Numerals are U+1D2E0..U+1D2F3 (0..19). Day-sign glyphs
# are not encoded. A coefficient outside that range stays decimal digits.


def mayan_numeral(n: int) -> str:
    """One Mayan numeral, or decimal digits when n is outside 0..19."""
    if 0 <= n <= 19:
        return chr(0x1D2E0 + n)
    return str(n)


def maya_days(day: date) -> int:
    """Days since 0.0.0.0.0 under GMT 584283."""
    return gregorian_jdn(day.year, day.month, day.day) - GMT


@dataclass(frozen=True)
class MayaDate:
    days: int
    baktun: int
    katun: int
    tun: int
    winal: int
    kin: int
    tzolkin_number: int
    tzolkin_name: str
    haab_day: int
    haab_month: str

    def long_count(self) -> str:
        return f"{self.baktun}.{self.katun}.{self.tun}.{self.winal}.{self.kin}"

    def long_count_mayan(self) -> str:
        parts = (self.baktun, self.katun, self.tun, self.winal, self.kin)
        return ".".join(mayan_numeral(n) for n in parts)

    def tzolkin_text(self) -> str:
        return (
            f"{self.tzolkin_number} "
            f"{beside(self.tzolkin_name, mayan_numeral(self.tzolkin_number))}"
        )

    def haab_text(self) -> str:
        return (
            f"{self.haab_day} "
            f"{beside(self.haab_month, mayan_numeral(self.haab_day))}"
        )

    def long_count_text(self) -> str:
        return beside(self.long_count(), self.long_count_mayan())

    def line(self) -> str:
        return (
            f"{self.tzolkin_text()}  "
            f"{self.haab_text()}  "
            f"{self.long_count_text()} (GMT {GMT})"
        )

    def to_dict(self) -> dict:
        return {
            "correlation": "GMT",
            "correlation_constant": GMT,
            "days": self.days,
            "long_count": self.long_count(),
            "long_count_mayan": self.long_count_mayan(),
            "tzolkin": f"{self.tzolkin_number} {self.tzolkin_name}",
            "tzolkin_numeral": mayan_numeral(self.tzolkin_number),
            "haab": f"{self.haab_day} {self.haab_month}",
            "haab_day_numeral": mayan_numeral(self.haab_day),
        }


def maya_from_gregorian(day: date) -> MayaDate:
    days = maya_days(day)
    if days < 0:
        raise TimeWarpError("Maya Long Count is before the GMT epoch (11 Aug 3114 BCE)")
    baktun, rem = divmod(days, 144000)
    katun, rem = divmod(rem, 7200)
    tun, rem = divmod(rem, 360)
    winal, kin = divmod(rem, 20)
    # 0.0.0.0.0 = 4 Ajaw.
    number = (days + 3) % 13 + 1
    name = _TZOLKIN[(days + 19) % 20]
    # 0.0.0.0.0 = 8 Kumku.
    haab_index = (days + 348) % 365
    if haab_index >= 360:
        haab_day = haab_index - 360
        haab_month = "Wayeb"
    else:
        haab_day = haab_index % 20
        haab_month = _HAAB[haab_index // 20]
    return MayaDate(
        days=days,
        baktun=baktun,
        katun=katun,
        tun=tun,
        winal=winal,
        kin=kin,
        tzolkin_number=number,
        tzolkin_name=name,
        haab_day=haab_day,
        haab_month=haab_month,
    )
