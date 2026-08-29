"""Human terminal output: ASCII columns, emoji only as trailing marks.

-q / --json / NO_COLOR / --no-color / non-TTY stay plain text (IAU symbols, no emoji).
Emoji in the same cell as a label or clock is what made ☀️/🌞 shove the rest of the
line over; terminals disagree with East Asian Width on VS16 sequences. Measure
columns with len() on ASCII and park the glyph after the aligned text.
"""

from __future__ import annotations

import os
import sys
from typing import TextIO

from timewarp.ephem import SYMBOL_RGB, SYMBOLS, format_body as _format_body_iau

# Prefer emoji over IAU miscellaneous-symbols; most modern terminals draw these.
EMOJI = {
    "sun": "🌞",
    "moon": "🌙",
    "mercury": "☿️",
    "venus": "♀️",
    "mars": "♂️",
    "jupiter": "🟠",
    "saturn": "🪐",
    "uranus": "🟢",
    "neptune": "🔵",
    "pluto": "🟣",
    "ceres": "🪨",
    "pallas": "🪨",
    "juno": "🪨",
    "vesta": "🪨",
    "hygiea": "🪨",
    "eros": "💎",
    "halley": "☄️",
    "encke": "☄️",
    "tempel1": "☄️",
    "67p": "☄️",
    "io": "🌕",
    "europa": "🌕",
    "ganymede": "🌕",
    "callisto": "🌕",
    "titan": "🟠",
    "triton": "🔵",
    "phobos": "🥔",
    "deimos": "🥔",
}

SKY_BIN = {
    "day": "🌞",
    "civil": "🏙️",
    "nautical": "🌆",
    "astronomical": "🌌",
    "night": "🌑",
}

ICON = {
    "rise": "⬆️",
    "set": "⬇️",
    "noon": "🕛",
    "dawn": "🌅",
    "dusk": "🌇",
    "night": "🌃",
    "calendar": "📅",
    "holiday": "🎉",
    "warn": "⚠️",
    "error": "❌",
    "pin": "📌",
    "clock": "⏳",
    "pass": "🛰️",
    "solar": "🌞",
    "lunar": "🌙",
}

_RESET = "\033[0m"


def want_color(args=None, *, stream: TextIO | None = None) -> bool:
    if args is not None and getattr(args, "no_color", False):
        return False
    if args is not None and getattr(args, "color", False):
        return True
    if os.environ.get("NO_COLOR", "").strip():
        return False
    force = os.environ.get("FORCE_COLOR", "").strip().lower()
    if force in {"1", "true", "yes"}:
        return True
    out = stream if stream is not None else sys.stdout
    return bool(getattr(out, "isatty", lambda: False)())


def want_emoji(args=None, *, stream: TextIO | None = None) -> bool:
    return want_color(args, stream=stream)


def icon(name: str, *, emoji: bool) -> str:
    if not emoji:
        return ""
    return ICON.get(name, "")


def marked(kind: str, label: str, *, emoji: bool) -> str:
    """Title line: 'emoji label', or just label. Not for table cells."""
    mark = icon(kind, emoji=emoji)
    return f"{mark} {label}" if mark else label


def sky_bin_label(name: str, *, emoji: bool) -> str:
    if not emoji:
        return name
    mark = SKY_BIN.get(name, "")
    return f"{mark} {name}" if mark else name


def _cell_len(text: str) -> int:
    try:
        from rich.cells import cell_len

        return cell_len(text)
    except ImportError:
        return len(text)


def _color_symbol(symbol: str, rgb: tuple[int, int, int]) -> str:
    r, g, b = rgb
    return f"\033[1;38;2;{r};{g};{b}m{symbol}{_RESET}"


def format_body(name: str, *, color: bool = False, width: int = 0, emoji: bool | None = None) -> str:
    """Label a body. color/emoji True → emoji glyph + tint; else IAU symbol."""
    if emoji is None:
        emoji = color
    key = name.strip().lower()
    if emoji:
        symbol = EMOJI.get(key) or SYMBOLS.get(key)
        plain = f"{symbol} {key}" if symbol else key
        visual = _cell_len(plain)
        if width > 0 and visual < width:
            plain = plain + " " * (width - visual)
        if not color or not symbol or key not in SYMBOL_RGB:
            return plain
        return plain.replace(symbol, _color_symbol(symbol, SYMBOL_RGB[key]), 1)
    return _format_body_iau(name, color=color, width=width)


def body_mark(name: str, *, color: bool) -> str:
    """Trailing body emoji, tinted. Empty when color is off."""
    if not color:
        return ""
    key = name.strip().lower()
    symbol = EMOJI.get(key) or ""
    if not symbol:
        return ""
    rgb = SYMBOL_RGB.get(key)
    if rgb:
        return _color_symbol(symbol, rgb)
    return symbol


def sky_mark(name: str, *, color: bool) -> str:
    if not color:
        return ""
    return SKY_BIN.get(name, "")


def _plain(cell: object) -> str:
    if cell is None:
        return ""
    plain = getattr(cell, "plain", None)
    if isinstance(plain, str):
        return plain
    return str(cell)


def _kv_parts(row: tuple) -> tuple[str, str, str, str]:
    """(mark, key, val, extra). 2-tuple = no icon; 4-tuple = icon first."""
    if len(row) >= 4:
        mark, key, val, extra = row[0], row[1], row[2], row[3]
    elif len(row) == 3:
        mark, key, val, extra = "", row[0], row[1], row[2]
    elif len(row) == 2:
        mark, key, val, extra = "", row[0], row[1], ""
    else:
        mark, key, val, extra = "", row[0] if row else "", "", ""
    return str(mark or ""), str(key), _plain(val), _plain(extra)


def _emit(line: str, mark: str = "", *, file: TextIO | None) -> None:
    out = file or sys.stdout
    if mark:
        # Keep value/extra padding so trailing glyphs share a column.
        print(f"{line}  {mark}", file=out)
        return
    print(line.rstrip(), file=out)


def print_kv(
    rows: list[tuple],
    *,
    color: bool,
    file: TextIO | None = None,
    key_width: int | None = None,
) -> int:
    """Right-aligned keys, left-aligned values/extras. Emoji trail the line.

    Extra (azimuth, ISO timestamp) is a third column sized only from rows that
    have one, so a long Place line cannot shove Next-full dates across the
    screen. Pass key_width to line colons up across several blocks.
    """
    parsed = [_kv_parts(row) for row in rows]
    return _print_kv_parsed(parsed, color=color, file=file, key_width=key_width)


def print_kv_blocks(
    blocks: list[list[tuple]],
    *,
    color: bool,
    file: TextIO | None = None,
) -> None:
    """Several kv blocks that share one key column (colons line up)."""
    parsed_blocks = [[_kv_parts(row) for row in block] for block in blocks if block]
    key_w = 0
    for parsed in parsed_blocks:
        key_w = max(key_w, max((len(k) for _m, k, _v, _x in parsed), default=0))
    for parsed in parsed_blocks:
        _print_kv_parsed(parsed, color=color, file=file, key_width=key_w)


def _print_kv_parsed(
    parsed: list[tuple[str, str, str, str]],
    *,
    color: bool,
    file: TextIO | None,
    key_width: int | None,
) -> int:
    key_w = max(key_width or 0, max((len(k) for _m, k, _v, _x in parsed), default=0))
    extra_rows = [(v, x) for _m, _k, v, x in parsed if x]
    val_w = max((len(v) for v, _x in extra_rows), default=0)
    extra_w = max((len(x) for _v, x in extra_rows), default=0)
    align_marks = extra_w > 0 and sum(1 for m, _k, _v, _x in parsed if m) >= 2
    for mark, key, val, extra in parsed:
        if extra_w:
            core = f"{key:>{key_w}}  {val:<{val_w}}  {extra:<{extra_w}}"
        else:
            core = f"{key:>{key_w}}  {val}"
        glyph = mark if color else ""
        if glyph and not align_marks:
            core = core.rstrip()
        _emit(core, glyph, file=file)
    return key_w


def print_grid(
    headers: list[str],
    rows: list[list],
    *,
    color: bool,
    file: TextIO | None = None,
    justify: dict[int, str] | None = None,
    widths: dict[int, int] | None = None,
    marks: list[str] | None = None,
) -> None:
    """ASCII columns (len() == cells) plus optional trailing emoji per row."""
    just = justify or {}
    wmap = widths or {}
    n = len(headers)
    plain_rows = []
    for row in rows:
        cells = [_plain(c) for c in row]
        if len(cells) < n:
            cells.extend([""] * (n - len(cells)))
        plain_rows.append(cells[:n])
    col_w = []
    for i in range(n):
        w = len(headers[i])
        for row in plain_rows:
            w = max(w, len(row[i]))
        if i in wmap:
            w = max(w, wmap[i])
        col_w.append(w)

    def fmt_row(cells: list[str]) -> str:
        parts = []
        for i, cell in enumerate(cells):
            if just.get(i) == "right":
                parts.append(cell.rjust(col_w[i]))
            else:
                parts.append(cell.ljust(col_w[i]))
        return "  ".join(parts)

    header_line = fmt_row(headers)
    _emit(header_line, file=file)
    _emit("─" * len(header_line), file=file)
    for i, row in enumerate(plain_rows):
        mark = ""
        if color and marks and i < len(marks):
            mark = marks[i]
        _emit(fmt_row(row), mark, file=file)
