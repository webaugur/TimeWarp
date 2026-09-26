"""English name beside a native-script spelling.

The native span sits in a first-strong isolate so Hebrew and Arabic
do not reorder the rest of an LTR line. JSON stores the raw spelling.
"""

from __future__ import annotations

from collections.abc import Mapping

from timewarp.errors import TimeWarpError

# First Strong Isolate / Pop Directional Isolate.
_FSI = "\u2068"
_PDI = "\u2069"


def isolate(native: str) -> str:
    """Wrap native text so it cannot reorder surrounding Latin text."""
    return f"{_FSI}{native}{_PDI}"


def beside(english: str, native: str) -> str:
    """English, two spaces, then the isolated native spelling."""
    if not native:
        raise TimeWarpError(f"empty native spelling for {english}")
    return f"{english}  {isolate(native)}"


def named_date(day: int, month: str, year: int, native: str) -> str:
    """Western day, English month, native spelling, Western year."""
    return f"{day} {beside(month, native)} {year}"


def spelling(table: Mapping[str, str], name: str, script: str) -> str:
    """Look up a native spelling. A missing name is a table bug."""
    try:
        native = table[name]
    except KeyError as exc:
        raise TimeWarpError(f"no {script} spelling for {name}") from exc
    if not native:
        raise TimeWarpError(f"empty {script} spelling for {name}")
    return native
