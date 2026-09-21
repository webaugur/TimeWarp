"""Portable double-click: interactive TimeWarp prompt in the onedir folder."""

from __future__ import annotations

import os
import shlex
import shutil
import stat
import subprocess
import sys
from pathlib import Path
from typing import Iterable

from timewarp import __version__
from timewarp.paths import is_frozen

_IN_REPL = False


def exe_dir() -> Path:
    if is_frozen():
        return Path(sys.executable).resolve().parent
    return Path.cwd()


def windows_owns_console() -> bool:
    """True when this process is the only one on the console (Explorer double-click)."""
    if os.name != "nt":
        return False
    try:
        buf = (ctypes_uint8())()
        n = _console_process_count(buf)
        return n == 1
    except (AttributeError, OSError, ValueError):
        return False


def ctypes_uint8():
    import ctypes

    return ctypes.c_uint * 8


def _console_process_count(buf: object) -> int:
    import ctypes

    return int(ctypes.windll.kernel32.GetConsoleProcessList(buf, 8))


_STREAM_BLOCKED = frozenset({"shell", "demo", "-", "--stdin"})


def stdin_is_command_stream() -> bool:
    """True when stdin is a pipe or file (not a TTY, not /dev/null)."""
    try:
        if sys.stdin.isatty():
            return False
        mode = os.fstat(sys.stdin.fileno()).st_mode
    except (OSError, ValueError, AttributeError):
        return False
    return bool(stat.S_ISFIFO(mode) or stat.S_ISREG(mode))


def tokenize_line(text: str) -> list[str] | None:
    """Split one command line. None = skip (blank or comment)."""
    stripped = text.strip()
    if not stripped or stripped.startswith("#"):
        return None
    posix = os.name != "nt"
    tokens = shlex.split(stripped, posix=posix)
    if tokens and tokens[0].lower() in {"timewarp", "timewarp.exe"}:
        tokens = tokens[1:]
    return tokens


def run_command_lines(lines: Iterable[str], *, invoke=None) -> int:
    """Run TimeWarp commands from an iterable of lines. Continue on errors."""
    from timewarp.cli import main as cli_main

    call = invoke if invoke is not None else cli_main
    last_err = 0
    for line in lines:
        try:
            tokens = tokenize_line(line)
        except ValueError as exc:
            print(f"timewarp: {exc}", file=sys.stderr)
            last_err = 2
            continue
        if not tokens:
            continue
        if tokens[0] in _STREAM_BLOCKED:
            print(f"timewarp: {tokens[0]!r} is not allowed in a command stream", file=sys.stderr)
            last_err = 2
            continue
        try:
            code = call(tokens)
        except SystemExit as exc:
            code = exc.code
            if code is None or code is True:
                code = 0
            elif code is False:
                code = 1
            else:
                try:
                    code = int(code)
                except (TypeError, ValueError):
                    code = 1
        except KeyboardInterrupt:
            print("timewarp: interrupted", file=sys.stderr)
            return 130
        if code not in (0, None):
            last_err = int(code)
    return last_err


def should_auto_repl(argv_was_none: bool, raw: list[str]) -> bool:
    if not argv_was_none or raw or _IN_REPL:
        return False
    if not is_frozen():
        return False
    if os.name == "nt":
        return windows_owns_console()
    try:
        return not sys.stdin.isatty()
    except (OSError, ValueError, AttributeError):
        return True


def _unix_terminal_argv(workdir: Path, inner: list[str]) -> list[str] | None:
    custom = os.environ.get("TERMINAL", "").strip()
    if custom:
        path = shutil.which(custom) or (custom if os.path.isfile(custom) else None)
        if path:
            return [path, "-e", *inner]
    probes: list[tuple[str, list[str]]] = [
        ("xdg-terminal-exec", [f"--dir={workdir}", "--", *inner]),
        ("gnome-terminal", [f"--working-directory={workdir}", "--", *inner]),
        ("konsole", ["--workdir", str(workdir), "-e", *inner]),
        (
            "xfce4-terminal",
            [f"--working-directory={workdir}", "-e", " ".join(shlex.quote(p) for p in inner)],
        ),
        ("kitty", ["--directory", str(workdir), *inner]),
        ("wezterm", ["start", "--cwd", str(workdir), "--", *inner]),
        ("xterm", ["-e", *inner]),
    ]
    for name, extra in probes:
        path = shutil.which(name)
        if path:
            return [path, *extra]
    return None


def spawn_unix_terminal(workdir: Path) -> int:
    """Open a terminal emulator running `timewarp shell` in `workdir`."""
    exe = str(Path(sys.executable).resolve())
    inner = [exe, "shell"]
    env = os.environ.copy()
    env["PATH"] = str(workdir) + os.pathsep + env.get("PATH", "")

    if sys.platform == "darwin":
        script = f"cd {shlex.quote(str(workdir))} && exec {shlex.quote(exe)} shell"
        quoted = script.replace("\\", "\\\\").replace('"', '\\"')
        osa = ["osascript", "-e", f'tell application "Terminal" to do script "{quoted}"']
        try:
            subprocess.Popen(osa, env=env)
            return 0
        except OSError as exc:
            print(f"timewarp: could not open Terminal ({exc})", file=sys.stderr)
            return 2

    argv = _unix_terminal_argv(workdir, inner)
    if argv is None:
        print(
            "timewarp: no terminal emulator found; run from a terminal: timewarp shell",
            file=sys.stderr,
        )
        return 2
    try:
        subprocess.Popen(argv, cwd=str(workdir), env=env)
    except OSError as exc:
        print(f"timewarp: could not open a terminal ({exc})", file=sys.stderr)
        return 2
    return 0


def maybe_launch_from_double_click(argv_was_none: bool, raw: list[str]) -> int | None:
    """Return an exit code if we handled a double-click; otherwise None."""
    if not should_auto_repl(argv_was_none, raw):
        return None
    workdir = exe_dir()
    try:
        os.chdir(workdir)
    except OSError as exc:
        print(f"timewarp: could not use {workdir}: {exc}", file=sys.stderr)
        return 2
    if os.name != "nt":
        try:
            if not sys.stdin.isatty():
                return spawn_unix_terminal(workdir)
        except (OSError, ValueError, AttributeError):
            return spawn_unix_terminal(workdir)
    return run_repl()


def run_repl(*, invoke=None) -> int:
    """Read TimeWarp commands until quit. `invoke` is main(tokens) for tests."""
    global _IN_REPL
    if _IN_REPL:
        print("timewarp: already in the interactive shell", file=sys.stderr)
        return 2
    from timewarp.cli import main as cli_main

    call = invoke if invoke is not None else cli_main
    workdir = Path.cwd()
    if is_frozen():
        workdir = exe_dir()
        try:
            os.chdir(workdir)
        except OSError:
            pass
    _IN_REPL = True
    try:
        print(f"TimeWarp {__version__}")
        print(f"Working directory: {workdir}")
        print("Type a command (sun --city Indianapolis), help, or quit.")
        while True:
            try:
                line = input("timewarp> ")
            except EOFError:
                print()
                return 0
            except KeyboardInterrupt:
                print()
                continue
            text = line.strip()
            if not text:
                continue
            low = text.lower()
            if low in {"quit", "exit", "q"}:
                return 0
            try:
                tokens = tokenize_line(line)
            except ValueError as exc:
                print(f"timewarp: {exc}", file=sys.stderr)
                continue
            if not tokens:
                continue
            if tokens[0] == "shell":
                print("timewarp: already in the interactive shell", file=sys.stderr)
                continue
            try:
                call(tokens)
            except SystemExit as exc:
                code = exc.code
                if code not in (None, 0, True, False):
                    pass
            except KeyboardInterrupt:
                print()
                continue
        return 0
    finally:
        _IN_REPL = False
