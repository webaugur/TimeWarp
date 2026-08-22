"""timewarp command line."""

from __future__ import annotations

import argparse
import json
import sys
from datetime import date, datetime
from typing import Sequence

from timewarp import __version__
from timewarp.astro import moon_info, sun_times
from timewarp.ephem import BODIES
from timewarp.rise import events_for_day
from timewarp.calendar_view import year_calendar
from timewarp.duration import apply_offset, parse_offset, span
from timewarp.eclipses import eclipse_to_dict, iso_range, list_eclipses
from timewarp.errors import TimeWarpError
from timewarp.holidays import parse_weekend
from timewarp.iso import (
    as_date,
    format_instant,
    format_labeled,
    parse_instant,
    weekday_name,
)
from timewarp.places import Place, lookup_place
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
  countdown      signed time from now to a date (negative if past)
  sun            sunrise / solar noon / sunset
  moon           moon phase and illumination
  rise           rise/set/transit: moon, sun, mercury, venus, mars, jupiter, …
  moonrise       alias for: rise moon
  eclipse        solar/lunar eclipses 2021–2030 (NASA / Espenak)

Examples:
  {PROG} add 2026-07-04 7 months 6 days
  {PROG} add 2026-07-04T09:00:00 P7M6DT3H
  {PROG} count 2026-05-31 2025-04-30
  {PROG} workdays 2026-01-01 2026-01-31 --holidays US
  {PROG} add-workdays 2026-07-04 10 --holidays US
  {PROG} weekday 2026-07-04
  {PROG} week 2026-07-04
  {PROG} calendar 2026 --country US
  {PROG} countdown 2026-12-31T00:00:00
  {PROG} sun --city "New York" 2026-07-04
  {PROG} moon 2026-08-28
  {PROG} rise moon --city "New York" 2026-07-04
  {PROG} moonrise --city London 2026-08-28
  {PROG} rise venus --city "New York" 2026-07-04
  {PROG} rise --all --city "New York" 2026-07-04
  {PROG} eclipse 2026

Dates are ISO 8601 only: YYYY-MM-DD, YYYY-MM-DDTHH:MM[:SS][Z|+HH:MM],
YYYY-Www-D, YYYY-DDD. Optional words: today, now, yesterday, tomorrow.
Negative offsets after the date may need -- so they are not flags:
  {PROG} add 2026-07-04 -- -P7M
"""


def _print_json(payload: object) -> int:
    json.dump(payload, sys.stdout, indent=2, sort_keys=True)
    sys.stdout.write("\n")
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


def cmd_add(args: argparse.Namespace) -> int:
    start = parse_instant(args.date)
    tokens = _peel_flags(args, list(args.offset))
    offset = parse_offset(tokens)
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
    start = parse_instant(args.date)
    try:
        n = parse_workday_count(args.count)
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
    inst = parse_instant(args.date)
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
    inst = parse_instant(args.date)
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
    year = args.year
    if year is None:
        year = date.today().year
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


def cmd_countdown(args: argparse.Namespace) -> int:
    target = parse_instant(args.date)
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
        return Place(place.name, lat, lon, tz)
    if args.lat is None or args.lon is None:
        raise TimeWarpError("location required: --city NAME or both --lat and --lon (and usually --tz)")
    tz = args.tz or "UTC"
    return Place("custom", args.lat, args.lon, tz)


def cmd_sun(args: argparse.Namespace) -> int:
    inst = parse_instant(args.date) if args.date else date.today()
    place = _place_from_args(args)
    result = sun_times(inst, place)
    if args.json:
        return _print_json(result.to_dict())
    print(f"Date:  {result.date.isoformat()}")
    print(f"Place: {result.place.name} ({result.place.lat}, {result.place.lon}) {result.place.tz}")
    if result.note:
        print(result.note)
    if result.sunrise:
        print(f"Sunrise:    {format_instant(result.sunrise)}")
    if result.solar_noon:
        print(f"Solar noon: {format_instant(result.solar_noon)}")
    if result.sunset:
        print(f"Sunset:     {format_instant(result.sunset)}")
    if result.day_length_seconds is not None:
        print(f"Day length: {result.to_dict()['day_length_iso8601']}")
    return 0


def cmd_moon(args: argparse.Namespace) -> int:
    inst = parse_instant(args.date) if args.date else date.today()
    result = moon_info(inst)
    if args.json:
        return _print_json(result.to_dict())
    print(f"Date:          {result.date.isoformat()}")
    print(f"Phase:         {result.phase}")
    print(f"Illumination:  {result.illumination:.1%}")
    print(f"Age:           {result.age_days:.2f} days")
    print(f"Next new moon: {result.next_new.isoformat()}")
    print(f"Next full:     {result.next_full.isoformat()}")
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


def _print_rise(result) -> None:
    from timewarp.ephem import altitude_azimuth, position as sky_position

    print(f"Body:  {result.body}")
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
    for label, times in (("Rise", result.rises), ("Transit", result.transits), ("Set", result.sets)):
        if not times:
            print(f"{label}:    —")
            continue
        for when in times:
            event_pos = sky_position(result.body, when)
            alt, az = altitude_azimuth(event_pos, when, result.place.lat, result.place.lon)
            extra = _fmt_az(az) if label != "Transit" else f"alt {alt:.1f}°"
            print(f"{label}:    {format_instant(when)}  {extra}")


def cmd_rise(args: argparse.Namespace) -> int:
    from timewarp.ephem import normalize_body

    place = _place_from_args(args)
    body = args.body
    inst = parse_instant(args.date) if args.date else None
    if inst is None and body:
        try:
            normalize_body(body)
        except TimeWarpError:
            inst = parse_instant(body)
            body = None
    if inst is None:
        inst = date.today()
    if args.all:
        bodies = BODIES
    else:
        if not body:
            raise TimeWarpError("rise needs a body (moon, venus, …) or --all")
        bodies = [body]
    results = [events_for_day(b, inst, place) for b in bodies]
    if args.json:
        if len(results) == 1:
            return _print_json(results[0].to_dict())
        return _print_json({"events": [r.to_dict() for r in results]})
    if args.quiet:
        for r in results:
            iso = format_instant(r.rises[0]) if r.rises else "none"
            if len(results) == 1:
                print(iso)
            else:
                print(f"{r.body:8} {iso}")
        return 0
    for i, result in enumerate(results):
        if i:
            print()
        if len(results) > 1:
            print(f"== {result.body} ==")
        _print_rise(result)
    return 0


def cmd_eclipse(args: argparse.Namespace) -> int:
    year = args.year
    after = None
    limit = args.limit
    if year is None:
        after = date.today()
        if limit is None:
            limit = 8
    rows = list_eclipses(year=year, after=after, limit=limit)
    if args.json:
        return _print_json(
            {
                "source": "Eclipse Predictions by Fred Espenak, NASA GSFC",
                "coverage": "2021-01-01/2030-12-31",
                "eclipses": [eclipse_to_dict(e) for e in rows],
            }
        )
    if not rows:
        print("No eclipses in catalog for that query (coverage is 2021–2030).")
        return 0
    print("Eclipse Predictions by Fred Espenak, NASA GSFC")
    print("Catalog coverage: 2021-01-01/2030-12-31")
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


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog=PROG,
        description=HELP,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--version", action="version", version=f"{PROG} {__version__}")
    sub = parser.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("count", aliases=["between", "duration"], help="Count Days: signed duration")
    _add_common(p)
    _add_include_end(p)
    p.add_argument("start", help="ISO 8601 start")
    p.add_argument("end", help="ISO 8601 end")
    p.set_defaults(func=cmd_count)

    p = sub.add_parser("add", help="Add Days: add years/months/weeks/days/time")
    _add_common(p)
    p.add_argument("date", help="ISO 8601 start")
    p.add_argument("offset", nargs=argparse.REMAINDER, help="7 months 6 days  or  P7M6D")
    p.set_defaults(func=cmd_add, subtract=False)

    p = sub.add_parser("sub", aliases=["subtract"], help="Add Days: subtract an offset")
    _add_common(p)
    p.add_argument("date")
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
    p.add_argument("date")
    p.add_argument("count", help="signed integer or P10D / -P10D")
    p.set_defaults(func=cmd_add_workdays)

    p = sub.add_parser("weekday", help="Weekday for a date")
    _add_common(p)
    p.add_argument("date")
    p.set_defaults(func=cmd_weekday)

    p = sub.add_parser("week", aliases=["weekno", "week-number"], help="ISO week number")
    _add_common(p)
    p.add_argument("date")
    p.set_defaults(func=cmd_week)

    p = sub.add_parser("calendar", help="Year calendar")
    _add_common(p)
    p.add_argument("year", nargs="?", type=int)
    p.add_argument("--country", default="US", help="holiday calendar (US bundled)")
    p.add_argument("--iso", action="store_true", help="Monday-first weeks (ISO)")
    p.set_defaults(func=cmd_calendar)

    p = sub.add_parser("countdown", help="Signed time from now to a date")
    _add_common(p)
    p.add_argument("date")
    p.set_defaults(func=cmd_countdown)

    def _add_place(parser: argparse.ArgumentParser) -> None:
        parser.add_argument("--city", help="named city (New York, London, Tokyo, ...)")
        parser.add_argument("--lat", type=float)
        parser.add_argument("--lon", type=float)
        parser.add_argument("--tz", help="IANA time zone")

    p = sub.add_parser("sun", help="Sunrise and sunset")
    _add_common(p)
    p.add_argument("date", nargs="?", help="ISO 8601 date (default: today)")
    _add_place(p)
    p.set_defaults(func=cmd_sun)

    p = sub.add_parser("moon", help="Moon phase")
    _add_common(p)
    p.add_argument("date", nargs="?", help="ISO 8601 date (default: today)")
    p.set_defaults(func=cmd_moon)

    p = sub.add_parser(
        "rise",
        help="Rise/set/transit for moon, sun, and planets",
    )
    _add_common(p)
    p.add_argument("body", nargs="?", help="moon, sun, mercury, venus, mars, jupiter, saturn, uranus, neptune, pluto")
    p.add_argument("date", nargs="?", help="ISO 8601 date (default: today)")
    p.add_argument("--all", action="store_true", help="print every bundled body")
    _add_place(p)
    p.set_defaults(func=cmd_rise)

    p = sub.add_parser("moonrise", help="alias for: rise moon")
    _add_common(p)
    p.add_argument("date", nargs="?", help="ISO 8601 date (default: today)")
    _add_place(p)
    p.set_defaults(func=cmd_rise, body="moon", all=False)

    p = sub.add_parser("eclipse", help="Eclipse catalog 2021–2030")
    _add_common(p)
    p.add_argument("year", nargs="?", type=int)
    p.add_argument("--limit", type=int)
    p.set_defaults(func=cmd_eclipse)

    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    try:
        args = parser.parse_args(list(argv) if argv is not None else None)
        return args.func(args)
    except TimeWarpError as exc:
        print(f"{PROG}: {exc}", file=sys.stderr)
        return 2
    except BrokenPipeError:
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
