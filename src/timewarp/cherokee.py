"""Cherokee (Kituwah) month names mapped onto the Gregorian month.

This is the published month-name list, not a computed new moon.
September and December have no secure English gloss in the sources we use;
those rows are transliteration and syllabary only.
"""

from __future__ import annotations

from datetime import date

# month, gloss or "", transliteration, syllabary
_MONTHS: tuple[tuple[str, str, str], ...] = (
    ("", "", ""),
    ("cold moon", "unolvtanv", "ᎤᏃᎸᏔᎾ"),
    ("bony moon", "kagali", "ᎧᎦᎵ"),
    ("strawberry moon", "anvyi", "ᎠᏅᏱ"),
    ("duck moon", "kawoni", "ᎧᏬᏂ"),
    ("planting moon", "anisgvti", "ᎠᏂᏍᎨᏘ"),
    ("green-corn moon", "dehaluyi", "ᏕᎭᎷᏱ"),
    ("ripe-corn moon", "kuyegwona", "ᎫᏰᏉᎾ"),
    ("drying-up moon", "galonee", "ᎦᎶᏁᎡ"),
    ("", "dulisdi", "ᏚᎵᏍᏗ"),
    ("harvest moon", "duninhdi", "ᏚᏂᏂᏗ"),
    ("big moon", "nvdadegwa", "ᏅᏓᏕᏆ"),
    ("", "uskiya", "ᎤᏍᎩᏯ"),
)


def cherokee_month(day: date) -> tuple[str, str, str]:
    """Return (English gloss or '', transliteration, syllabary) for the Gregorian month."""
    return _MONTHS[day.month]


def cherokee_line(day: date) -> str:
    gloss, name, syll = cherokee_month(day)
    if gloss:
        return f"{gloss}  {name}  {syll}"
    return f"{name}  {syll}"
