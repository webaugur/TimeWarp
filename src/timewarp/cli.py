"""timewarp command line."""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import traceback
from datetime import date, datetime
from typing import Sequence

from timewarp import __version__
from timewarp.astro import moon_info, seasons_for_year, sun_times
from timewarp.ephem import BODIES, format_body
from timewarp.rise import events_for_period
from timewarp.calendar_view import year_calendar
from timewarp.month_view import format_month_sheet, parse_year_month, sheet_for_month
from timewarp.duration import OffsetError, apply_offset, parse_offset, span
from timewarp.eclipses import eclipse_to_dict, iso_range, list_eclipses
from timewarp.errors import TimeWarpError
from timewarp.holidays import parse_weekend
from timewarp.iso import (
    as_date,
    format_clock,
    format_instant,
    format_labeled,
    parse_instant,
    weekday_name,
)
from timewarp.cache import (
    CACHEABLE,
    cache_path,
    clear as cache_clear,
    data_as_pulled,
    flags_on_argv,
    format_pulled_cli,
    load as cache_load,
    quote_value,
    save as cache_save,
)
from timewarp.passes import (
    DEFAULT_MIN_ELEV,
    fetch_tle,
    load_tle_file,
    passes_for_day,
    select_sats,
    tle_freshness_note,
)
from timewarp.places import Place, lookup_place, place_names
from timewarp.rise import each_civil_day
from timewarp.workdays import add_workdays, count_workdays, parse_workday_count

PROG = "timewarp"

HELP = f"""\
{PROG} — local date calculators (ISO 8601 in, ISO 8601 out)

Phase 1 (dates and durations):
  count          Count Days     duration between two instants (signed)
  add            Add Days       add years/months/weeks/days/time
  sub            Add Days       subtract the same offset
  workdays       Workdays       count Mon–Fri (signed; optional US holidays)
  add-workdays   Add Workdays   add or subtract business days
  weekday        Weekday        ISO weekday for a date
  week           Week №         ISO 8601 week date (YYYY-Www-D)

Phase 2 (basic):
  calendar       year calendar with optional US holidays
  month          month sheet of sun/moon/twilight times
  countdown      signed time from now to a date (negative if past)
  sun            sunrise / sunset, twilight, azimuth
  moon           moon phase and next new/full/quarter times
  seasons        equinoxes and solstices
  passes         satellite passes (TLE / ISS) vs twilight and the moon
  rise           rise times for visible bodies (today, or a date / date range)
  set            set times for the same bodies and period
  moonrise       alias for: rise moon
  moonset        alias for: set moon
  cities         named places (capitals + IANA tz cities)
  save           store --city and similar flags
  load           print stored flags (scriptable)
  unload         drop stored flags
  cache          same as save/load/unload, nested: cache save|load|unload
  eclipse        solar/lunar eclipses 1900–2199 (Meeus)
  help           this overview, or help for one command (--help works too)

Examples:
  {PROG} add P7M6D
  {PROG} add 7 years 6 months
  {PROG} add 2026-07-04 7 months 6 days
  {PROG} add 2026-07-04T09:00:00 P7M6DT3H
  {PROG} count 2026-05-31 2025-04-30
  {PROG} workdays 2026-01-01 2026-01-31 --holidays US
  {PROG} add-workdays 2026-07-04 10 --holidays US
  {PROG} weekday 2026-07-04
  {PROG} week 2026-07-04
  {PROG} calendar 2026 --country US
  {PROG} month 2026-07 --city Indianapolis
  {PROG} month --city Indianapolis --twilight
  {PROG} countdown 2026-12-31T00:00:00
  {PROG} sun --city "New York" 2026-07-04
  {PROG} moon 2026-08-28 --city Indianapolis
  {PROG} seasons 2026
  {PROG} passes --city Indianapolis
  {PROG} passes ISS --city "New York" 2019-12-10 --tle path/to/iss.tle
  {PROG} rise --city "New York"
  {PROG} rise --city "New York" 2026-07-04
  {PROG} rise --city Indianapolis --13 --33
  {PROG} rise --city "New York" 2026-07-04 2026-07-10
  {PROG} rise moon --city "New York" 2026-07-04
  {PROG} moonrise --city London 2026-08-28
  {PROG} set --city "New York" 2026-07-04
  {PROG} set venus --city London
  {PROG} moonset --city London 2026-08-28
  {PROG} eclipse 2026
  {PROG} eclipse 1919
  {PROG} rise ceres --city London
  {PROG} rise io --city London
  {PROG} rise halley --city London
  {PROG} cities
  {PROG} save --city Indianapolis
  {PROG} load
  {PROG} unload --city
  {PROG} unload
  {PROG} help
  {PROG} help add

Dates are ISO 8601 only: YYYY-MM-DD, YYYY-MM-DDTHH:MM[:SS][Z|+HH:MM],
YYYY-Www-D, YYYY-DDD. Optional words: today, now, yesterday, tomorrow.
Omit a date to use today (yellow on the reconstructed command line).
Sky times print HH:MM plus a zone letter (17:52R); -q and --json stay ISO 8601.
`sun` includes civil/nautical/astronomical twilight. `passes` needs sgp4 and a TLE.
Negative offsets after the date may need -- so they are not flags:
  {PROG} add 2026-07-04 -- -P7M
"""


def _want_color(args: argparse.Namespace | None = None) -> bool:
    if args is not None and getattr(args, "no_color", False):
        return False
    if args is not None and getattr(args, "color", False):
        return True
    force = os.environ.get("FORCE_COLOR", "").strip().lower()
    if force in {"1", "true", "yes"}:
        return True
    return sys.stdout.isatty()


def _body_label(name: str, width: int = 0, *, color: bool | None = None) -> str:
    if color is None:
        color = _want_color()
    return format_body(name, color=color, width=width)


_PINK = "\033[38;2;255;128;192m"
_YELLOW = "\033[38;2;255;220;0m"
_WHITE = "\033[97m"
_RESET = "\033[0m"


def _stderr_color(args: argparse.Namespace | None = None) -> bool:
    if args is not None and getattr(args, "no_color", False):
        return False
    if args is not None and getattr(args, "color", False):
        return True
    return sys.stderr.isatty()


def _paint(text: str, on: str, *, enabled: bool) -> str:
    if not enabled or not text:
        return text
    return f"{on}{text}{_RESET}"


def _echo_cached_command(
    pulled: list, raw: list[str], args: argparse.Namespace, *, assumed: str | None = None
) -> None:
    color = _stderr_color(args)
    user = " ".join(quote_value(a) for a in raw)
    flags = format_pulled_cli(pulled, prog="")
    head = _paint("timewarp", _WHITE, enabled=color)
    mid = _paint(flags, _PINK, enabled=color)
    tail = _paint(user, _WHITE, enabled=color)
    guess = _paint(assumed, _YELLOW, enabled=color) if assumed else ""
    print(" ".join(p for p in (head, mid, tail, guess) if p), file=sys.stderr)


def _maybe_echo_command(args: argparse.Namespace, assumed: str | None) -> None:
    pulled = getattr(args, "cache_pulled", None) or []
    raw = getattr(args, "raw_argv", None) or []
    assumed_s = None if getattr(args, "json", False) else assumed
    if pulled or assumed_s:
        _echo_cached_command(pulled, raw, args, assumed=assumed_s)


def _print_json(payload: object) -> int:
    json.dump(payload, sys.stdout, indent=2, sort_keys=True)
    sys.stdout.write("\n")
    return 0


def cmd_help(args: argparse.Namespace) -> int:
    topic = getattr(args, "topic", None)
    commands = getattr(args, "help_commands", {}) or {}
    if not topic:
        text = HELP if HELP.endswith("\n") else HELP + "\n"
        sys.stdout.write(text)
        print(f"Command help: {PROG} help COMMAND")
        print(f"Also:         {PROG} --help    {PROG} COMMAND --help")
        return 0
    key = topic.lower()
    if key in {"help", "?", "h"}:
        print(f"usage: {PROG} help [COMMAND]")
        print()
        print("Show the overview, or the same text as COMMAND --help.")
        return 0
    target = commands.get(topic) or commands.get(key)
    if target is None:
        names = sorted({p.prog.rsplit(" ", 1)[-1] for p in commands.values()})
        raise TimeWarpError(f"no help for {topic!r}; commands: {', '.join(names)}")
    target.print_help()
    return 0


def cmd_count(args: argparse.Namespace) -> int:
    start = parse_instant(args.start)
    end = parse_instant(args.end)
    result = span(start, end, include_end=args.include_end)
    if args.json:
        return _print_json(result.to_dict())
    if args.quiet:
        print(result.iso())
        return 0
    weeks, week_days = result.weeks_and_days()
    direction = "forward" if result.sign > 0 else "backward" if result.sign < 0 else "zero"
    print(f"From  {format_labeled(result.start)}")
    print(f"To    {format_labeled(result.end)}")
    if result.include_end:
        print("Include end date: yes")
    print(f"Duration: {result.human()}")
    print(f"ISO 8601: {result.iso()}")
    print(f"Total days: {result.total_days}")
    print(f"Weeks+days: {weeks} weeks, {week_days} days")
    if result.hours or result.minutes or result.seconds or isinstance(result.start, datetime):
        print(f"Total seconds: {result.total_seconds}")
    print(f"Direction: {direction}")
    return 0


def _peel_flags(args: argparse.Namespace, tokens: list[str]) -> list[str]:
    """Allow -q/--json after remainder tokens (add DATE OFFSET... -q)."""
    kept = []
    for t in tokens:
        if t in ("-q", "--quiet"):
            args.quiet = True
        elif t in ("-j", "--json"):
            args.json = True
        elif t == "--":
            continue
        else:
            kept.append(t)
    return kept


def _looks_like_offset(text: str) -> bool:
    compact = text.strip().replace(" ", "")
    if re.match(r"^-?P", compact, re.IGNORECASE):
        return True
    if re.fullmatch(r"-?\d{1,3}-\d{1,2}(?:-\d{1,2})?", compact):
        return True
    return bool(
        re.search(
            r"\d\s*(years?|yrs?|y|months?|mons?|mo|weeks?|wks?|w|days?|d|"
            r"hours?|hrs?|h|minutes?|mins?|min|seconds?|secs?|s)\b",
            text,
            re.IGNORECASE,
        )
    )


def _split_start_offset(tokens: list[str]) -> tuple:
    """Return (start, offset_tokens, assumed_today). Date omitted → today."""
    if not tokens:
        raise OffsetError(
            "missing offset; example: 7 months 6 days or P7M6D (date defaults to today)"
        )
    first = tokens[0]
    if _looks_like_offset(first):
        return date.today(), tokens, True
    try:
        start = parse_instant(first)
    except TimeWarpError:
        return date.today(), tokens, True
    rest = tokens[1:]
    if not rest:
        raise OffsetError(
            "missing offset; example: 7 months 6 days or P7M6D "
            "(omit the date to add to today)"
        )
    return start, rest, False


def cmd_add(args: argparse.Namespace) -> int:
    tokens = []
    if getattr(args, "date", None):
        tokens.append(args.date)
    tokens.extend(_peel_flags(args, list(getattr(args, "offset", None) or [])))
    start, offset_tokens, assumed = _split_start_offset(tokens)
    _maybe_echo_command(args, as_date(start).isoformat() if assumed else None)
    offset = parse_offset(offset_tokens)
    if args.subtract:
        offset = offset.negated()
    result = apply_offset(start, offset)
    if args.json:
        return _print_json(
            {
                "start": format_instant(start),
                "offset": offset.human(),
                "result": format_instant(result),
                "weekday": weekday_name(as_date(result)),
                "iso_week": _iso_week_label(as_date(result)),
            }
        )
    if args.quiet:
        print(format_instant(result))
        return 0
    verb = "Subtract" if args.subtract else "Add"
    print(f"Start:  {format_labeled(start)}")
    print(f"{verb}:    {offset.human()}")
    print(f"Result: {format_labeled(result)}")
    print(f"ISO 8601: {format_instant(result)}")
    print(f"ISO week: {_iso_week_label(as_date(result))}")
    return 0


def _iso_week_label(d: date) -> str:
    iso = d.isocalendar()
    return f"{iso.year:04d}-W{iso.week:02d}-{iso.weekday}"


def cmd_workdays(args: argparse.Namespace) -> int:
    start = parse_instant(args.start)
    end = parse_instant(args.end)
    weekend = parse_weekend(args.weekend)
    result = count_workdays(
        start,
        end,
        include_end=args.include_end,
        weekend=weekend,
        holiday_country=args.holidays,
    )
    if args.json:
        return _print_json(result.to_dict())
    if args.quiet:
        print(result.iso())
        return 0
    print(f"From  {format_labeled(result.start)}")
    print(f"To    {format_labeled(result.end)}")
    print(f"Workdays: {result.workdays}")
    print(f"ISO 8601: {result.iso()}")
    print(f"Calendar days: {result.calendar_days}")
    print(f"Weekend days: {result.weekend_days}")
    if args.holidays:
        print(f"Holiday days skipped: {result.holiday_days} ({args.holidays})")
    return 0


def cmd_add_workdays(args: argparse.Namespace) -> int:
    if args.count is None:
        if not args.date:
            raise TimeWarpError("missing workday count; example: timewarp add-workdays 10")
        start = date.today()
        assumed = True
        count_tok = args.date
    else:
        start = parse_instant(args.date)
        assumed = False
        count_tok = args.count
    _maybe_echo_command(args, as_date(start).isoformat() if assumed else None)
    try:
        n = parse_workday_count(count_tok)
    except ValueError as exc:
        raise TimeWarpError(str(exc)) from exc
    result = add_workdays(start, n, holiday_country=args.holidays, weekend=parse_weekend(args.weekend))
    if args.json:
        return _print_json(
            {
                "start": format_instant(as_date(start)),
                "workdays": n,
                "result": result.isoformat(),
                "weekday": weekday_name(result),
            }
        )
    if args.quiet:
        print(result.isoformat())
        return 0
    print(f"Start:    {format_labeled(as_date(start))}")
    print(f"Workdays: {n}")
    print(f"Result:   {format_labeled(result)}")
    print(f"ISO 8601: {result.isoformat()}")
    return 0


def cmd_weekday(args: argparse.Namespace) -> int:
    assumed = not args.date
    inst = parse_instant(args.date) if args.date else date.today()
    _maybe_echo_command(args, as_date(inst).isoformat() if assumed else None)
    d = as_date(inst)
    iso = d.isoweekday()
    payload = {
        "date": d.isoformat(),
        "weekday": weekday_name(d),
        "iso_weekday": iso,
        "iso8601": _iso_week_label(d),
    }
    if args.json:
        return _print_json(payload)
    if args.quiet:
        print(f"{d.isoformat()} {payload['weekday']}")
        return 0
    print(f"Date:        {d.isoformat()}")
    print(f"Weekday:     {payload['weekday']}")
    print(f"ISO weekday: {iso} (Monday=1, Sunday=7)")
    print(f"ISO week:    {payload['iso8601']}")
    return 0


def cmd_week(args: argparse.Namespace) -> int:
    assumed = not args.date
    inst = parse_instant(args.date) if args.date else date.today()
    _maybe_echo_command(args, as_date(inst).isoformat() if assumed else None)
    d = as_date(inst)
    iso = d.isocalendar()
    label = _iso_week_label(d)
    if args.json:
        return _print_json(
            {
                "date": d.isoformat(),
                "iso_week_date": label,
                "week_year": iso.year,
                "week": iso.week,
                "iso_weekday": iso.weekday,
                "weekday": weekday_name(d),
            }
        )
    if args.quiet:
        print(label)
        return 0
    print(f"Date:          {d.isoformat()} {weekday_name(d)}")
    print(f"ISO week date: {label}")
    print(f"Week year:     {iso.year}")
    print(f"Week:          {iso.week}")
    print(f"ISO weekday:   {iso.weekday}")
    return 0


def cmd_calendar(args: argparse.Namespace) -> int:
    assumed = args.year is None
    year = args.year
    if year is None:
        year = date.today().year
    _maybe_echo_command(args, str(year) if assumed else None)
    if not 1 <= year <= 9999:
        raise TimeWarpError(f"year {year} is out of range 1..9999")
    text = year_calendar(year, country=args.country, iso_weeks=args.iso)
    if args.json:
        from timewarp.holidays import us_federal_holidays

        hols = []
        if args.country.strip().upper() in {"US", "USA", "UNITED STATES"}:
            hols = [{"date": d.isoformat(), "name": n} for d, n in us_federal_holidays(year)]
        return _print_json({"year": year, "country": args.country, "holidays": hols, "text": text})
    sys.stdout.write(text)
    return 0


def cmd_month(args: argparse.Namespace) -> int:
    year, month, assumed = parse_year_month(getattr(args, "when", None))
    place = _place_from_args(args)
    _maybe_echo_command(args, f"{year:04d}-{month:02d}" if assumed else None)
    rows = sheet_for_month(year, month, place)
    twilight = bool(getattr(args, "twilight", False))
    if args.json:
        return _print_json(
            {
                "year": year,
                "month": month,
                "place": place.name,
                "tz": place.tz,
                "twilight": twilight,
                "days": [r.to_dict() for r in rows],
            }
        )
    if args.quiet:
        for r in rows:
            rise = format_clock(r.sunrise) if r.sunrise else "none"
            sset = format_clock(r.sunset) if r.sunset else "none"
            print(f"{r.date.isoformat()} {rise} {sset}")
        return 0
    sys.stdout.write(format_month_sheet(rows, place, twilight=twilight))
    return 0


def cmd_countdown(args: argparse.Namespace) -> int:
    assumed = not args.date
    target = parse_instant(args.date) if args.date else date.today()
    _maybe_echo_command(args, as_date(target).isoformat() if assumed else None)
    if isinstance(target, datetime):
        start: datetime | date = datetime.now(tz=target.tzinfo).replace(microsecond=0)
    else:
        start = date.today()
    result = span(start, target)
    if args.json:
        payload = result.to_dict()
        payload["mode"] = "countdown"
        return _print_json(payload)
    if args.quiet:
        print(result.iso())
        return 0
    when = "until" if result.sign >= 0 else "since"
    print(f"Now:    {format_labeled(result.start)}")
    print(f"Target: {format_labeled(result.end)}")
    print(f"Time {when}: {result.human()}")
    print(f"ISO 8601: {result.iso()}")
    print(f"Total days: {result.total_days}")
    return 0


def _place_from_args(args: argparse.Namespace) -> Place:
    if args.city:
        place = lookup_place(args.city)
        lat = args.lat if args.lat is not None else place.lat
        lon = args.lon if args.lon is not None else place.lon
        tz = args.tz if args.tz else place.tz
        name = place.name
    else:
        if args.lat is None or args.lon is None:
            raise TimeWarpError("location required: --city NAME or both --lat and --lon (and usually --tz)")
        lat, lon = args.lat, args.lon
        tz = args.tz or "UTC"
        name = "custom"
    if not -90 <= lat <= 90:
        raise TimeWarpError(f"latitude {lat} is out of range -90..90")
    if not -180 <= lon <= 180:
        raise TimeWarpError(f"longitude {lon} is out of range -180..180")
    return Place(name, lat, lon, tz)


def _optional_place(args: argparse.Namespace):
    if getattr(args, "city", None) or (
        getattr(args, "lat", None) is not None and getattr(args, "lon", None) is not None
    ):
        return _place_from_args(args)
    return None


def _clock_at(when: datetime, tz: str | None) -> str:
    if tz:
        from zoneinfo import ZoneInfo

        when = when.astimezone(ZoneInfo(tz))
    return format_clock(when)


def cmd_sun(args: argparse.Namespace) -> int:
    assumed = not args.date
    inst = parse_instant(args.date) if args.date else date.today()
    place = _place_from_args(args)
    _maybe_echo_command(args, as_date(inst).isoformat() if assumed else None)
    result = sun_times(inst, place)
    if args.json:
        return _print_json(result.to_dict())
    print(f"Body:  {_body_label('sun', color=_want_color(args))}")
    print(f"Date:  {result.date.isoformat()}")
    print(f"Place: {result.place.name} ({result.place.lat}, {result.place.lon}) {result.place.tz}")
    if result.note:
        print(result.note)

    def line(label: str, when, az=None) -> None:
        head = f"{label}:"
        if not when:
            print(f"{head:20} —")
            return
        extra = f"  {_fmt_az(az)}" if az is not None else ""
        print(f"{head:20} {format_clock(when)}{extra}")

    line("Astronomical dawn", result.astronomical_dawn)
    line("Nautical dawn", result.nautical_dawn)
    line("Civil dawn", result.civil_dawn)
    line("Sunrise", result.sunrise, result.sunrise_az)
    line("Solar noon", result.solar_noon)
    line("Sunset", result.sunset, result.sunset_az)
    line("Civil dusk", result.civil_dusk)
    line("Nautical dusk", result.nautical_dusk)
    line("Astronomical dusk", result.astronomical_dusk)
    if result.day_length_seconds is not None:
        print(f"{'Day length:':20} {result.to_dict()['day_length_iso8601']}")
    return 0


def cmd_moon(args: argparse.Namespace) -> int:
    assumed = not args.date
    inst = parse_instant(args.date) if args.date else date.today()
    place = _optional_place(args)
    _maybe_echo_command(args, as_date(inst).isoformat() if assumed else None)
    result = moon_info(inst)
    tz = place.tz if place else "UTC"
    if args.json:
        payload = result.to_dict()
        if place:
            payload["place"] = place.name
            payload["tz"] = place.tz
        return _print_json(payload)
    print(f"Body:          {_body_label('moon', color=_want_color(args))}")
    print(f"Date:          {result.date.isoformat()}")
    if place:
        print(f"Place:         {place.name} ({place.tz})")
    print(f"Phase:         {result.phase}")
    print(f"Illumination:  {result.illumination:.1%}")
    print(f"Age:           {result.age_days:.2f} days")
    print(f"Next new:      {_clock_at(result.next_new, tz)}  {format_instant(result.next_new)}")
    print(f"Next first Q:  {_clock_at(result.next_first_quarter, tz)}  {format_instant(result.next_first_quarter)}")
    print(f"Next full:     {_clock_at(result.next_full, tz)}  {format_instant(result.next_full)}")
    print(f"Next last Q:   {_clock_at(result.next_last_quarter, tz)}  {format_instant(result.next_last_quarter)}")
    return 0


def cmd_seasons(args: argparse.Namespace) -> int:
    assumed = args.year is None
    year = date.today().year if args.year is None else args.year
    if not 1 <= year <= 9999:
        raise TimeWarpError(f"year {year} is out of range 1..9999")
    _maybe_echo_command(args, str(year) if assumed else None)
    rows = seasons_for_year(year)
    place = _optional_place(args)
    tz = place.tz if place else "UTC"
    if args.json:
        payload = {"year": year, "events": [e.to_dict() for e in rows]}
        if place:
            payload["place"] = place.name
            payload["tz"] = tz
        return _print_json(payload)
    print(f"Astronomical seasons {year}")
    if place:
        print(f"Place: {place.name} ({tz})")
    for e in rows:
        print(f"  {e.name:22} {_clock_at(e.time, tz)}  {format_instant(e.time)}")
    return 0


def cmd_passes(args: argparse.Namespace) -> int:
    from pathlib import Path

    place = _place_from_args(args)
    tokens = [t for t in (getattr(args, "sat", None), getattr(args, "date", None), getattr(args, "end", None)) if t]
    sat_q = None
    dates = []
    for tok in tokens:
        try:
            dates.append(parse_instant(tok))
        except TimeWarpError:
            if sat_q is not None:
                raise TimeWarpError("give at most one satellite name")
            sat_q = tok
    if len(dates) > 2:
        raise TimeWarpError("give at most a start date and an end date")
    assumed = not dates
    if not dates:
        start = end = date.today()
    elif len(dates) == 1:
        start = end = dates[0]
    else:
        start, end = dates[0], dates[1]
    _maybe_echo_command(args, as_date(start).isoformat() if assumed else None)
    min_elev = float(getattr(args, "min_elev", DEFAULT_MIN_ELEV))
    if not 0 <= min_elev <= 90:
        raise TimeWarpError("--min-elev must be in 0..90 degrees")
    tle_path = getattr(args, "tle", None)
    if tle_path:
        sats = load_tle_file(Path(tle_path))
    else:
        sats = fetch_tle(sat_q or "ISS")
    picked = select_sats(sats, sat_q, all_sats=bool(getattr(args, "all", False)))
    days = each_civil_day(start, end, place)
    note = tle_freshness_note(picked, start)
    rows = []
    for sat in picked:
        for day in days:
            rows.extend(passes_for_day(sat, day, place, min_elev=min_elev))
    rows.sort(key=lambda p: (p.tca, p.sat.name))
    if args.json:
        payload = {
            "start": as_date(start).isoformat(),
            "end": as_date(end).isoformat(),
            "min_elev_deg": min_elev,
            "note": note,
            "passes": [p.to_dict() for p in rows],
        }
        return _print_json(payload)
    if note:
        print(note)
    span = as_date(start).isoformat() if as_date(start) == as_date(end) else f"{as_date(start).isoformat()}/{as_date(end).isoformat()}"
    print(f"{place.name}  {span}  {place.tz}  min {min_elev:.0f}°")
    if not rows:
        print("No passes above the minimum elevation.")
        return 0
    if args.quiet:
        for p in rows:
            print(f"{p.sat.name:12} {format_instant(p.tca)}  {p.max_alt_deg:4.0f}°  {p.twilight}")
        return 0
    print(
        f"{'sat':12}  {'aos':8}  {'max':8}  {'alt':5}  {'az':12}  {'los':8}  "
        f"{'sky':13}  {'moon':6}  {'sep':5}"
    )
    for p in rows:
        moon = f"{p.moon_alt_deg:5.0f}°" if p.moon_alt_deg > -0.5 else "   —"
        az = _fmt_az(p.az_tca).strip()
        print(
            f"{p.sat.name[:12]:12}  {format_clock(p.aos):8}  {format_clock(p.tca):8}  "
            f"{p.max_alt_deg:4.0f}°  {az:12}  {format_clock(p.los):8}  "
            f"{p.twilight:13}  {moon:6}  {p.moon_sep_deg:4.0f}°"
        )
    return 0


def _fmt_az(deg: float | None) -> str:
    if deg is None:
        return ""
    dirs = (
        (0, "N"),
        (45, "NE"),
        (90, "E"),
        (135, "SE"),
        (180, "S"),
        (225, "SW"),
        (270, "W"),
        (315, "NW"),
        (360, "N"),
    )
    name = min(dirs, key=lambda item: abs((deg - item[0] + 180) % 360 - 180))[1]
    return f"{deg:6.1f}° {name}"


def _alt_rows(result, alt13: bool, alt33: bool) -> tuple[list[tuple[str, tuple]], list[tuple[str, tuple]]]:
    rows: list[tuple[str, tuple]] = []
    if alt13:
        rows.append(("+13°", result.after_rise_13))
    if alt33:
        rows.append(("+33°", result.after_rise_33))
    before: list[tuple[str, tuple]] = []
    if alt33:
        before.append(("33°", result.before_set_33))
    if alt13:
        before.append(("13°", result.before_set_13))
    return rows, before


def _print_sky_detail(
    result, *, primary: str, color: bool = False, alt13: bool = False, alt33: bool = False
) -> None:
    from timewarp.ephem import altitude_azimuth, position as sky_position

    print(f"Body:  {_body_label(result.body, color=color)}")
    print(f"Date:  {result.date.isoformat()}  ({result.place.tz})")
    print(f"Place: {result.place.name} ({result.place.lat}, {result.place.lon})")
    pos = result.position
    print(f"RA/Dec (noon): {pos.ra_deg:.3f}° / {pos.dec_deg:.3f}°")
    print(f"Distance: {pos.distance:.5g} {pos.distance_unit}")
    if pos.elongation_deg is not None:
        print(f"Elongation: {pos.elongation_deg:.1f}°")
    if pos.phase is not None and result.body != "sun":
        print(f"Illumination: {pos.phase:.1%}")
    if pos.magnitude is not None:
        print(f"Magnitude: {pos.magnitude:.2f}")
    if result.note:
        print(result.note)
    after, before = _alt_rows(result, alt13, alt33)
    if primary == "set":
        order = [
            ("Set", result.sets),
            *reversed(before),
            ("Transit", result.transits),
            ("Rise", result.rises),
            *after,
        ]
    else:
        order = [("Rise", result.rises), *after, ("Transit", result.transits), *before, ("Set", result.sets)]
    for label, times in order:
        if not times:
            print(f"{label}:    —")
            continue
        for when in times:
            event_pos = sky_position(result.body, when)
            alt, az = altitude_azimuth(event_pos, when, result.place.lat, result.place.lon)
            extra = _fmt_az(az) if label != "Transit" else f"alt {alt:.1f}°"
            print(f"{label}:    {format_clock(when)}  {extra}")


def _fmt_event_times(times) -> str:
    if not times:
        return "—"
    return ", ".join(format_clock(t) for t in times)


def _sky_table_cols(primary: str, alt13: bool, alt33: bool) -> list[tuple[str, str]]:
    after: list[tuple[str, str]] = []
    before: list[tuple[str, str]] = []
    if alt13:
        after.append(("+13°", "after_rise_13"))
    if alt33:
        after.append(("+33°", "after_rise_33"))
    if alt33:
        before.append(("33°", "before_set_33"))
    if alt13:
        before.append(("13°", "before_set_13"))
    if primary == "set":
        return [("set", "sets"), *reversed(before), ("rise", "rises"), *after]
    return [("rise", "rises"), *after, *before, ("set", "sets")]


def _print_sky_table(
    results, *, primary: str, color: bool = False, alt13: bool = False, alt33: bool = False
) -> None:
    if not results:
        print("No visible bodies in that period.")
        return
    place = results[0].place
    dates = sorted({r.date for r in results})
    if len(dates) == 1:
        print(f"{place.name}  {place.tz}")
    else:
        span = f"{dates[0].isoformat()}/{dates[-1].isoformat()}"
        print(f"{place.name}  {span}  {place.tz}")
    multi_day = len(dates) > 1
    cols = _sky_table_cols(primary, alt13, alt33)
    clock_w = 8
    parts = []
    if multi_day:
        parts.append(f"{'date':10}")
    parts.append(f"{'body':12}")
    parts.extend(f"{title:{clock_w}}" for title, _attr in cols)
    print("  ".join(parts))
    for r in results:
        label = _body_label(r.body, width=12, color=color)
        cells = []
        if multi_day:
            cells.append(f"{r.date.isoformat():10}")
        cells.append(label)
        for _title, attr in cols:
            cells.append(f"{_fmt_event_times(getattr(r, attr)):{clock_w}}")
        print("  ".join(cells))


def _parse_sky_when(args: argparse.Namespace):
    from timewarp.ephem import normalize_body

    tokens = [t for t in (getattr(args, "body", None), getattr(args, "date", None), getattr(args, "end", None)) if t]
    body_name = None
    dates = []
    for tok in tokens:
        try:
            name = normalize_body(tok)
        except TimeWarpError:
            dates.append(parse_instant(tok))
            continue
        if body_name is not None:
            raise TimeWarpError("give at most one body name")
        body_name = name
    if len(dates) > 2:
        raise TimeWarpError("give at most a start date and an end date")
    assumed = not dates
    if not dates:
        start = end = date.today()
    elif len(dates) == 1:
        start = end = dates[0]
    else:
        start, end = dates[0], dates[1]
    return body_name, start, end, assumed


def _primary_times(result, primary: str):
    return result.sets if primary == "set" else result.rises


def _event_sort_key(result, primary: str):
    """Local date, then first rise (or set), then name. Missing times go last."""
    times = _primary_times(result, primary)
    if times:
        return (result.date, 0, times[0].timestamp(), result.body)
    return (result.date, 1, 0.0, result.body)


def cmd_rise(args: argparse.Namespace) -> int:
    primary = getattr(args, "kind", "rise")
    place = _place_from_args(args)
    body_name, start, end, assumed = _parse_sky_when(args)
    _maybe_echo_command(args, as_date(start).isoformat() if assumed else None)
    if body_name:
        bodies = [body_name]
    else:
        bodies = list(BODIES)
    results = []
    for body in bodies:
        results.extend(events_for_period(body, start, end, place))
    if not getattr(args, "all", False) and body_name is None:
        results = [r for r in results if r.visible]
    results.sort(key=lambda r: _event_sort_key(r, primary))
    color = _want_color(args)
    alt13 = getattr(args, "alt13", False)
    alt33 = getattr(args, "alt33", False)

    if args.json:
        payload = {
            "kind": primary,
            "start": as_date(start).isoformat(),
            "end": as_date(end).isoformat(),
            "events": [r.to_dict() for r in results],
        }
        if len(results) == 1 and body_name:
            payload = results[0].to_dict()
            payload["kind"] = primary
        return _print_json(payload)
    if args.quiet:
        for r in results:
            times = _primary_times(r, primary)
            iso = format_instant(times[0]) if times else "none"
            if len(results) == 1:
                print(iso)
            elif len({x.date for x in results}) > 1:
                print(f"{r.date.isoformat()} {_body_label(r.body, width=12, color=color)} {iso}")
            else:
                print(f"{_body_label(r.body, width=12, color=color)} {iso}")
        return 0
    if body_name and len({r.date for r in results}) == 1:
        _print_sky_detail(results[0], primary=primary, color=color, alt13=alt13, alt33=alt33)
        return 0
    _print_sky_table(results, primary=primary, color=color, alt13=alt13, alt33=alt33)
    return 0


def cmd_cities(args: argparse.Namespace) -> int:
    names = place_names()
    if args.json:
        rows = []
        for n in names:
            p = lookup_place(n)
            rows.append({"name": p.name, "lat": p.lat, "lon": p.lon, "tz": p.tz})
        return _print_json({"cities": rows})
    for n in names:
        p = lookup_place(n)
        print(f"{p.name:24} {p.lat:10.5f} {p.lon:11.5f}  {p.tz}")
    return 0


def _apply_cache(args: argparse.Namespace, rest_argv: list[str]) -> list[tuple[str, str | bool]]:
    data = cache_load()
    present = flags_on_argv(rest_argv)
    pulled: list[tuple[str, str | bool]] = []

    def take(attr: str, flag: str) -> None:
        if not hasattr(args, attr) or attr in present:
            return
        if attr not in data:
            return
        setattr(args, attr, data[attr])
        val = data[attr]
        pulled.append((flag, True if val is True else val))

    take("city", "--city")
    take("lat", "--lat")
    take("lon", "--lon")
    take("tz", "--tz")
    take("holidays", "--holidays")
    take("weekend", "--weekend")
    take("country", "--country")
    if hasattr(args, "color") and "color" not in present and "no_color" not in present:
        if data.get("color") and not args.color and not getattr(args, "no_color", False):
            args.color = True
            pulled.append(("--color", True))
        elif data.get("no_color") and not args.color and not getattr(args, "no_color", False):
            args.no_color = True
            pulled.append(("--no-color", True))
    return pulled


def cmd_cache(args: argparse.Namespace) -> int:
    op = getattr(args, "cache_cmd", None) or "load"
    if op in {"load", "show"}:
        return cmd_cache_load(args)
    if op in {"save", "set"}:
        return cmd_cache_save(args)
    if op in {"unload", "clear"}:
        return cmd_cache_unload(args)
    raise TimeWarpError(f"unknown cache action {op!r}")


def cmd_cache_load(args: argparse.Namespace) -> int:
    data = cache_load()
    pulled = data_as_pulled(data)
    if args.json:
        return _print_json({"path": str(cache_path()), "settings": data})
    line = format_pulled_cli(pulled, prog="" if args.quiet else "timewarp")
    if line:
        print(line)
    elif not args.quiet:
        print("timewarp")
    return 0


def cmd_cache_save(args: argparse.Namespace) -> int:
    data = cache_load()
    wrote = False
    if getattr(args, "city", None):
        data["city"] = lookup_place(args.city).name
        wrote = True
    if getattr(args, "lat", None) is not None:
        data["lat"] = args.lat
        wrote = True
    if getattr(args, "lon", None) is not None:
        data["lon"] = args.lon
        wrote = True
    if getattr(args, "tz", None):
        data["tz"] = args.tz
        wrote = True
    if getattr(args, "holidays", None):
        data["holidays"] = args.holidays
        wrote = True
    if getattr(args, "weekend", None):
        data["weekend"] = args.weekend
        wrote = True
    if getattr(args, "country", None):
        data["country"] = args.country
        wrote = True
    if getattr(args, "color", False):
        data["color"] = True
        data.pop("no_color", None)
        wrote = True
    if getattr(args, "no_color", False):
        data["no_color"] = True
        data.pop("color", None)
        wrote = True
    if not wrote:
        raise TimeWarpError("cache save needs a setting, e.g. --city Indianapolis")
    cache_save(data)
    line = format_pulled_cli(data_as_pulled(data), prog="" if args.quiet else "timewarp")
    if line:
        print(line)
    return 0


def cmd_cache_unload(args: argparse.Namespace) -> int:
    keys: list[str] = []
    for raw in getattr(args, "keys", None) or []:
        name = raw[2:] if raw.startswith("--") else raw
        name = name.replace("_", "-")
        if name not in CACHEABLE:
            raise TimeWarpError(
                f"unknown cache key {raw!r}; known: {', '.join(sorted(CACHEABLE))}"
            )
        keys.append(CACHEABLE[name])
    for flag, key in CACHEABLE.items():
        if getattr(args, f"unload_{key}", False):
            keys.append(key)
    keys = list(dict.fromkeys(keys))
    if keys:
        cache_clear(keys)
        flags = " ".join(f"--{k.replace('_', '-')}" for k in keys)
        print(f"timewarp: unloaded {flags}", file=sys.stderr)
    else:
        cache_clear()
        print("timewarp: cache unloaded", file=sys.stderr)
    return 0


def cmd_eclipse(args: argparse.Namespace) -> int:
    year = args.year
    after = None
    limit = args.limit
    if year is not None and not 1 <= year <= 9999:
        raise TimeWarpError(f"year {year} is out of range 1..9999")
    if limit is not None and limit < 1:
        raise TimeWarpError("--limit must be a positive integer")
    if year is None:
        after = date.today()
        if limit is None:
            limit = 8
    rows = list_eclipses(year=year, after=after, limit=limit)
    if args.json:
        return _print_json(
            {
                "source": "Meeus, Astronomical Algorithms, ch. 54",
                "coverage": "1900-01-01/2199-12-31",
                "eclipses": [eclipse_to_dict(e) for e in rows],
            }
        )
    if not rows:
        print("No eclipses in catalog for that query (coverage is 1900–2199).")
        return 0
    print("Eclipses 1900–2199 (Meeus, Astronomical Algorithms ch. 54)")
    print("Greatest eclipse date is UTC. Types from γ and u.")
    for e in rows:
        print(f"  {iso_range(e):21}  {e.kind:6}  {e.type}")
    return 0


def _add_common(p: argparse.ArgumentParser) -> None:
    p.add_argument("-j", "--json", action="store_true", help="JSON output")
    p.add_argument("-q", "--quiet", action="store_true", help="one-line ISO 8601 result")


def _add_include_end(p: argparse.ArgumentParser) -> None:
    p.add_argument(
        "--include-end",
        action="store_true",
        help="count the end date (extends the span by one calendar day)",
    )


def _add_work_flags(p: argparse.ArgumentParser) -> None:
    p.add_argument(
        "--weekend",
        default="Sat,Sun",
        help="days treated as weekend (names or ISO 1-7). Default: Sat,Sun",
    )
    p.add_argument(
        "--holidays",
        metavar="COUNTRY",
        help="skip public holidays (US is the bundled calendar)",
    )


class _Parser(argparse.ArgumentParser):
    """Turn argparse failures into TimeWarpError so main() can print them."""

    def error(self, message: str) -> None:
        self.print_usage(sys.stderr)
        raise TimeWarpError(message)


def build_parser() -> argparse.ArgumentParser:
    parser = _Parser(
        prog=PROG,
        description=HELP,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--version", action="version", version=f"{PROG} {__version__}")
    sub = parser.add_subparsers(dest="cmd", required=False)
    parser.set_defaults(func=cmd_help, topic=None)

    p = sub.add_parser("help", aliases=["?"], help="Show help (also: --help, COMMAND --help)")
    p.add_argument("topic", nargs="?", help="command name (add, rise, count, …)")
    p.set_defaults(func=cmd_help)

    p = sub.add_parser("count", aliases=["between", "duration"], help="Count Days: signed duration")
    _add_common(p)
    _add_include_end(p)
    p.add_argument("start", help="ISO 8601 start")
    p.add_argument("end", help="ISO 8601 end")
    p.set_defaults(func=cmd_count)

    p = sub.add_parser("add", help="Add Days: add years/months/weeks/days/time")
    _add_common(p)
    p.add_argument("date", nargs="?", help="ISO 8601 start (default: today)")
    p.add_argument("offset", nargs=argparse.REMAINDER, help="7 months 6 days  or  P7M6D")
    p.set_defaults(func=cmd_add, subtract=False)

    p = sub.add_parser("sub", aliases=["subtract"], help="Add Days: subtract an offset")
    _add_common(p)
    p.add_argument("date", nargs="?", help="ISO 8601 start (default: today)")
    p.add_argument("offset", nargs=argparse.REMAINDER)
    p.set_defaults(func=cmd_add, subtract=True)

    p = sub.add_parser("workdays", aliases=["workday"], help="Workdays between two dates")
    _add_common(p)
    _add_include_end(p)
    _add_work_flags(p)
    p.add_argument("start")
    p.add_argument("end")
    p.set_defaults(func=cmd_workdays)

    p = sub.add_parser("add-workdays", aliases=["add-workday"], help="Add Workdays")
    _add_common(p)
    _add_work_flags(p)
    p.add_argument("date", nargs="?", help="ISO 8601 start (default: today)")
    p.add_argument("count", nargs="?", help="signed integer or P10D / -P10D")
    p.set_defaults(func=cmd_add_workdays)

    p = sub.add_parser("weekday", help="Weekday for a date")
    _add_common(p)
    p.add_argument("date", nargs="?", help="ISO 8601 date (default: today)")
    p.set_defaults(func=cmd_weekday)

    p = sub.add_parser("week", aliases=["weekno", "week-number"], help="ISO week number")
    _add_common(p)
    p.add_argument("date", nargs="?", help="ISO 8601 date (default: today)")
    p.set_defaults(func=cmd_week)

    p = sub.add_parser("calendar", help="Year calendar")
    _add_common(p)
    p.add_argument("year", nargs="?", type=int)
    p.add_argument("--country", default="US", help="holiday calendar (US bundled)")
    p.add_argument("--iso", action="store_true", help="Monday-first weeks (ISO)")
    p.set_defaults(func=cmd_calendar)

    p = sub.add_parser("countdown", help="Signed time from now to a date")
    _add_common(p)
    p.add_argument("date", nargs="?", help="ISO 8601 date (default: today)")
    p.set_defaults(func=cmd_countdown)

    def _add_place(parser: argparse.ArgumentParser) -> None:
        parser.add_argument("--city", help="named city (New York, London, Tokyo, ...)")
        parser.add_argument("--lat", type=float)
        parser.add_argument("--lon", type=float)
        parser.add_argument("--tz", help="IANA time zone")

    def _add_color_flags(parser: argparse.ArgumentParser) -> None:
        g = parser.add_mutually_exclusive_group()
        g.add_argument("--color", action="store_true", help="color body symbols (even when piped)")
        g.add_argument("--no-color", action="store_true", help="plain symbols, no ANSI color")

    def _add_cache_save_flags(parser: argparse.ArgumentParser) -> None:
        parser.add_argument("--city")
        parser.add_argument("--lat", type=float)
        parser.add_argument("--lon", type=float)
        parser.add_argument("--tz")
        parser.add_argument("--holidays")
        parser.add_argument("--weekend")
        parser.add_argument("--country")
        _add_color_flags(parser)

    def _add_cache_unload_flags(parser: argparse.ArgumentParser) -> None:
        parser.add_argument("keys", nargs="*", help="city, tz, color, …")
        for flag, key in CACHEABLE.items():
            parser.add_argument(
                f"--{flag}",
                dest=f"unload_{key}",
                action="store_true",
                help=f"unload --{flag}",
            )

    p = sub.add_parser("month", aliases=["almanac"], help="Month sheet of sun, moon, and twilight times")
    _add_common(p)
    p.add_argument("when", nargs="?", help="YYYY-MM (default: this month)")
    p.add_argument(
        "--twilight",
        action="store_true",
        help="also print nautical and astronomical dawn/dusk",
    )
    _add_place(p)
    _add_color_flags(p)
    p.set_defaults(func=cmd_month)

    p = sub.add_parser("sun", help="Sunrise, sunset, twilight, and azimuth")
    _add_common(p)
    p.add_argument("date", nargs="?", help="ISO 8601 date (default: today)")
    _add_place(p)
    _add_color_flags(p)
    p.set_defaults(func=cmd_sun)

    p = sub.add_parser("moon", help="Moon phase and next new/full/quarter times")
    _add_common(p)
    p.add_argument("date", nargs="?", help="ISO 8601 date (default: today)")
    _add_place(p)
    _add_color_flags(p)
    p.set_defaults(func=cmd_moon)

    p = sub.add_parser("seasons", help="Equinoxes and solstices")
    _add_common(p)
    p.add_argument("year", nargs="?", type=int, help="calendar year (default: this year)")
    _add_place(p)
    _add_color_flags(p)
    p.set_defaults(func=cmd_seasons)

    p = sub.add_parser("passes", help="Satellite passes vs twilight and the moon")
    _add_common(p)
    p.add_argument("sat", nargs="?", help="name or catalog number (default: ISS)")
    p.add_argument("date", nargs="?", help="ISO 8601 start date (default: today)")
    p.add_argument("end", nargs="?", help="ISO 8601 end date (inclusive)")
    p.add_argument("--tle", help="TLE file (skip Celestrak)")
    p.add_argument(
        "--min-elev",
        dest="min_elev",
        type=float,
        default=DEFAULT_MIN_ELEV,
        help="minimum max-elevation in degrees (default: 10)",
    )
    p.add_argument("--all", action="store_true", help="every satellite in the TLE set")
    _add_place(p)
    _add_color_flags(p)
    p.set_defaults(func=cmd_passes)

    def _add_sky_when(parser: argparse.ArgumentParser) -> None:
        parser.add_argument(
            "body",
            nargs="?",
            help="planet, moon, asteroid (ceres, vesta, …), comet (halley, 67p, …), or planetary moon (io, titan, …); default: visible planets",
        )
        parser.add_argument("date", nargs="?", help="ISO 8601 start date (default: today)")
        parser.add_argument("end", nargs="?", help="ISO 8601 end date (inclusive)")
        parser.add_argument(
            "--all",
            action="store_true",
            help="include bodies that stay below the horizon",
        )
        parser.add_argument(
            "--13",
            dest="alt13",
            action="store_true",
            help="times at +13° after rise and before set",
        )
        parser.add_argument(
            "--33",
            dest="alt33",
            action="store_true",
            help="times at +33° after rise and before set",
        )

    p = sub.add_parser("rise", help="Rise times for visible sun, moon, and planets")
    _add_common(p)
    _add_sky_when(p)
    _add_place(p)
    _add_color_flags(p)
    p.set_defaults(func=cmd_rise, kind="rise")

    p = sub.add_parser("set", help="Set times for visible sun, moon, and planets")
    _add_common(p)
    _add_sky_when(p)
    _add_place(p)
    _add_color_flags(p)
    p.set_defaults(func=cmd_rise, kind="set")

    p = sub.add_parser("moonrise", help="alias for: rise moon")
    _add_common(p)
    p.add_argument("date", nargs="?", help="ISO 8601 date (default: today)")
    p.add_argument("end", nargs="?", help="ISO 8601 end date (inclusive)")
    p.add_argument("--13", dest="alt13", action="store_true", help="times at +13° after rise and before set")
    p.add_argument("--33", dest="alt33", action="store_true", help="times at +33° after rise and before set")
    _add_place(p)
    _add_color_flags(p)
    p.set_defaults(func=cmd_rise, body="moon", all=False, kind="rise")

    p = sub.add_parser("moonset", help="alias for: set moon")
    _add_common(p)
    p.add_argument("date", nargs="?", help="ISO 8601 date (default: today)")
    p.add_argument("end", nargs="?", help="ISO 8601 end date (inclusive)")
    p.add_argument("--13", dest="alt13", action="store_true", help="times at +13° after rise and before set")
    p.add_argument("--33", dest="alt33", action="store_true", help="times at +33° after rise and before set")
    _add_place(p)
    _add_color_flags(p)
    p.set_defaults(func=cmd_rise, body="moon", all=False, kind="set")

    p = sub.add_parser("cities", help="List named places")
    _add_common(p)
    p.set_defaults(func=cmd_cities)

    cache_p = sub.add_parser("cache", help="Save, load, or unload remembered settings")
    cache_sub = cache_p.add_subparsers(dest="cache_cmd")
    cache_p.set_defaults(func=cmd_cache, cache_cmd="load")

    load_p = cache_sub.add_parser("load", aliases=["show"], help="print cached flags (scriptable)")
    _add_common(load_p)
    load_p.set_defaults(func=cmd_cache, cache_cmd="load")

    save_p = cache_sub.add_parser("save", aliases=["set"], help="store settings")
    _add_common(save_p)
    _add_cache_save_flags(save_p)
    save_p.set_defaults(func=cmd_cache, cache_cmd="save")

    unload_p = cache_sub.add_parser("unload", aliases=["clear"], help="drop cached settings")
    _add_common(unload_p)
    _add_cache_unload_flags(unload_p)
    unload_p.set_defaults(func=cmd_cache, cache_cmd="unload")

    p = sub.add_parser("save", help="store --city and similar flags")
    _add_common(p)
    _add_cache_save_flags(p)
    p.set_defaults(func=cmd_cache, cache_cmd="save")

    p = sub.add_parser("load", aliases=["show"], help="print stored flags")
    _add_common(p)
    p.set_defaults(func=cmd_cache, cache_cmd="load")

    p = sub.add_parser("unload", aliases=["clear"], help="drop stored flags")
    _add_common(p)
    _add_cache_unload_flags(p)
    p.set_defaults(func=cmd_cache, cache_cmd="unload")

    p = sub.add_parser("eclipse", help="Eclipse catalog 1900–2199")
    _add_common(p)
    p.add_argument("year", nargs="?", type=int)
    p.add_argument("--limit", type=int)
    p.set_defaults(func=cmd_eclipse)

    parser.tw_commands = dict(sub.choices)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    raw = list(sys.argv[1:] if argv is None else argv)
    parser = build_parser()
    try:
        args = parser.parse_args(raw)
        args.raw_argv = raw
        args.cache_pulled = []
        args.help_commands = getattr(parser, "tw_commands", {})
        if args.func is cmd_help:
            return cmd_help(args)
        echo_self = {
            cmd_rise,
            cmd_add,
            cmd_add_workdays,
            cmd_sun,
            cmd_moon,
            cmd_seasons,
            cmd_passes,
            cmd_weekday,
            cmd_week,
            cmd_countdown,
            cmd_calendar,
            cmd_month,
        }
        if args.func is not cmd_cache:
            pulled = _apply_cache(args, raw)
            args.cache_pulled = pulled
            if pulled and args.func not in echo_self:
                _echo_cached_command(pulled, raw, args)
        return args.func(args)
    except TimeWarpError as exc:
        print(f"{PROG}: {exc}", file=sys.stderr)
        return 2
    except BrokenPipeError:
        return 0
    except KeyboardInterrupt:
        print(f"{PROG}: interrupted", file=sys.stderr)
        return 130
    except SystemExit as exc:
        code = exc.code
        if code is None or code is True:
            return 0
        if code is False:
            return 1
        try:
            return int(code)
        except (TypeError, ValueError):
            return 2
    except Exception as exc:
        if os.environ.get("TIMEWARP_DEBUG", "").strip() in {"1", "true", "yes"}:
            traceback.print_exc()
        print(f"{PROG}: {type(exc).__name__}: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
