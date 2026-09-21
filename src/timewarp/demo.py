"""Walk major TimeWarp commands for a human viewer."""

from __future__ import annotations

import os
import sys
import time
from pathlib import Path
from typing import Callable

from timewarp.errors import TimeWarpError

DEFAULT_PAUSE = 5.0
_CLEAR = "\033[2J\033[H"

# (title, argv tokens)
_SCENES: tuple[tuple[str, tuple[str, ...]], ...] = (
    ("Count days", ("count", "2026-07-04", "2026-12-25")),
    ("Add a duration", ("add", "2026-07-04", "P1Y1M")),
    ("Workdays", ("workdays", "2026-07-04", "2026-08-01", "--holidays", "US")),
    ("Weekday", ("weekday", "2026-07-04")),
    ("Calendar", ("calendar", "2026", "--country", "US")),
    ("Holidays", ("holidays", "2026", "--country", "US")),
    ("Countdown", ("countdown", "2026-12-31T00:00:00")),
    ("Today", ("today", "--city", "Indianapolis", "2026-07-04")),
    ("Sun", ("sun", "--city", "New York", "2026-07-04")),
    ("Moon", ("moon", "--city", "New York", "2026-07-04")),
    ("Seasons", ("seasons", "2026")),
    ("Rosicrucian cycle", ("cycle", "2026-08-29")),
    ("Chart", ("astro", "--city", "Indianapolis", "2026-07-04")),
    ("Rise / set", ("rise", "--city", "New York", "2026-07-04")),
    ("Eclipses", ("eclipse", "2026")),
    ("Month sheet", ("month", "2026-07", "--city", "Indianapolis")),
)


def _format_cmd(tokens: tuple[str, ...]) -> str:
    parts = ["timewarp"]
    for t in tokens:
        if any(ch.isspace() for ch in t):
            parts.append(f'"{t}"')
        else:
            parts.append(t)
    return " ".join(parts)


def _iss_tle() -> Path | None:
    here = Path(__file__).resolve()
    for root in (here.parents[2], Path.cwd()):
        cand = root / "tests" / "data" / "iss.tle"
        if cand.is_file():
            return cand
    return None


def scenes() -> list[tuple[str, tuple[str, ...]]]:
    rows = list(_SCENES)
    tle = _iss_tle()
    if tle is not None:
        rows.append(
            (
                "ISS passes",
                (
                    "passes",
                    "ISS",
                    "--city",
                    "New York",
                    "2019-12-10",
                    "--tle",
                    str(tle),
                ),
            )
        )
    return rows


def clear_screen() -> None:
    sys.stdout.write(_CLEAR)
    sys.stdout.flush()


def wait_for_key() -> None:
    """Block until one key (or Enter). EOF / non-TTY continues so tests do not hang."""
    sys.stdout.write("Press any key for the next screen…")
    sys.stdout.flush()
    try:
        if not sys.stdin.isatty():
            sys.stdin.read(1)
            print()
            return
        if os.name == "nt":
            import msvcrt

            msvcrt.getch()
        else:
            import termios
            import tty

            fd = sys.stdin.fileno()
            old = termios.tcgetattr(fd)
            try:
                tty.setraw(fd, when=termios.TCSANOW)
                sys.stdin.read(1)
            finally:
                termios.tcsetattr(fd, termios.TCSADRAIN, old)
    except (EOFError, OSError, ValueError):
        pass
    print()


def pause(seconds: float) -> None:
    if seconds > 0:
        # Human viewing delay; there is no viewer-done signal unless --pause 0.
        time.sleep(seconds)
        return
    wait_for_key()


def run_demo(*, pause_s: float = DEFAULT_PAUSE, invoke: Callable[[list[str]], int] | None = None) -> int:
    from timewarp.cli import main as cli_main

    call = invoke if invoke is not None else cli_main
    if pause_s < 0:
        raise TimeWarpError("--pause must be >= 0")
    rows = scenes()
    n = len(rows)
    for i, (title, tokens) in enumerate(rows, start=1):
        clear_screen()
        print(f"TimeWarp demo  {i}/{n}  {title}")
        print(f"$ {_format_cmd(tokens)}")
        print()
        try:
            call(list(tokens))
        except SystemExit:
            pass
        pause(pause_s)
    print()
    print("Demo finished.")
    return 0
