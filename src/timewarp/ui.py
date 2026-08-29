"""Human terminal output: Rich console, emoji when color is on.

-q / --json / NO_COLOR / --no-color / non-TTY stay plain text (IAU symbols, no emoji).
"""

from __future__ import annotations

import os
import sys
from typing import TextIO

from timewarp.ephem import SYMBOL_RGB, SYMBOLS, format_body as _format_body_iau

# Prefer emoji over IAU miscellaneous-symbols; most modern terminals draw these.
EMOJI = {
    "sun": "☀️",
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
    "day": "☀️",
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
    "solar": "☀️",
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


def console(*, color: bool, file: TextIO | None = None):
    """Rich Console; falls back to a tiny printer if rich is missing."""
    stream = file if file is not None else sys.stdout
    try:
        from rich.console import Console

        return Console(
            file=stream,
            force_terminal=color,
            no_color=not color,
            highlight=False,
            emoji=False,
            color_system="truecolor" if color else None,
        )
    except ImportError:
        return _PlainConsole(stream)


class _PlainConsole:
    def __init__(self, file: TextIO) -> None:
        self.file = file

    def print(self, *args, **kwargs) -> None:
        print(*args, file=self.file)
