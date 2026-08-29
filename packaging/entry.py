"""PyInstaller console entry (so the freeze does not import tests)."""

from timewarp.cli import main

if __name__ == "__main__":
    raise SystemExit(main())
