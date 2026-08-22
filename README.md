# TimeWarp

Command-line date calculators, including **negative** spans (end before start).

Every date you type and every date printed is **ISO 8601**. There is no `MM/DD` vs `DD/MM`. Time of day is optional.

```
timewarp add 2026-07-04 7 months 6 days
timewarp between 2026-05-31 2025-04-30
```

The second example is the reason this exists: the end date is earlier, so the duration is negative (`-P1Y1M`, −396 days).

`2025-04-31` is not a date (April has 30 days). TimeWarp refuses it instead of rolling the extra day into May.

## Install

```bash
python3 -m pip install -e /path/to/TimeWarp
timewarp --help
```

Or without installing:

```bash
PYTHONPATH=src python3 -m timewarp --help
```

## Phase 1 — dates and durations

| Tab | Command | What it does |
|---|---|---|
| Count Days | `timewarp count START END` | Signed calendar duration (alias: `between`) |
| Add Days | `timewarp add DATE OFFSET...` | Add years, months, weeks, days, and optional time |
| Add Days | `timewarp sub DATE OFFSET...` | Subtract the same offset |
| Workdays | `timewarp workdays START END` | Signed Mon–Fri count |
| Add Workdays | `timewarp add-workdays DATE N` | Move N business days |
| Weekday | `timewarp weekday DATE` | English weekday + ISO weekday 1–7 |
| Week № | `timewarp week DATE` | ISO week date `YYYY-Www-D` |

```bash
timewarp add 2026-07-04 7 months 6 days
# Result: 2027-02-10 Saturday

timewarp add 2026-07-04T09:00:00 P7M6DT3H
# Result: 2027-02-10T12:00:00

timewarp count 2026-05-31 2025-04-30
# Duration: -(1 year, 1 month)
# ISO 8601: -P1Y1M
# Total days: -396

timewarp workdays 2026-01-01 2026-01-31 --holidays US
timewarp add-workdays 2026-07-04 10 --holidays US
timewarp weekday 2026-07-04
timewarp week 2026-07-04
```

`--include-end` counts the end date (one extra calendar day in the same direction).

`--json` prints a machine-readable object. `-q` prints one ISO 8601 line.

Offsets may be words (`7 months 6 days`, `3 hours`) or ISO 8601 durations (`P7M6D`, `PT3H`, `-P10D`). Units are applied largest-first: years, months, weeks, days, hours, minutes, seconds. Month-end days clamp: `2026-01-31` + 1 month = `2026-02-28`.

Bare `m` is not a unit (months vs minutes). Use `mo` or `min`.

Workdays skip Saturday and Sunday by default. `--weekend Fri,Sat` changes that. `--holidays US` also skips observed US federal holidays.

## Phase 2 — calendar, sky, and countdowns

| Screen | Command | What you get now |
|---|---|---|
| Create calendar | `timewarp calendar [YEAR]` | Year grid, US holidays marked, ISO dates listed |
| Countdown to Any Date | `timewarp countdown DATE` | Signed remaining time (negative if the date is past) |
| Sunrise & Sunset | `timewarp sun --city "New York" [DATE]` | Rise, noon, set, day length |
| Moon phases | `timewarp moon [DATE]` | Phase name, illumination, next new/full |
| Rise / set | `timewarp rise moon --city "New York" [DATE]` | Moonrise, planet rise/set/transit |
| Eclipse lookup | `timewarp eclipse [YEAR]` | Solar and lunar eclipses 2021–2030 |

```bash
timewarp calendar 2026 --country US
timewarp calendar 2026 --iso          # Monday-first weeks
timewarp countdown 2026-12-31
timewarp sun --city "New York" 2026-07-04
timewarp sun --lat 40.7128 --lon -74.0060 --tz America/New_York
timewarp moon 2026-08-28
timewarp rise moon --city "New York" 2026-07-04
timewarp moonrise --city London 2026-08-28
timewarp rise venus --city "New York"
timewarp rise --all --city "New York" 2026-07-04
timewarp eclipse 2026
```

Eclipse rows are from Fred Espenak / NASA GSFC decade tables (2021–2030). `timewarp sun` uses the NOAA/USNO sunrise algorithm. `timewarp rise` uses Paul Schlyter’s low-precision ephemeris (about 1–2 arcminutes; rise/set typically within a few minutes of almanac times). Moon phase uses synodic age.

Bundled rise bodies: sun, moon, mercury, venus, mars, jupiter, saturn, uranus, neptune, pluto (Pluto only 1800–2100). Comets, asteroids, and planetary moons need extra orbital data and are deferred.

Other country holiday packs, live timers, and a 1900–2199 eclipse atlas are later work.

## Input formats

Accepted:

- `YYYY-MM-DD`
- `YYYY-MM-DDTHH:MM[:SS][Z|+HH:MM]` (space instead of `T` is also accepted)
- `YYYY-Www-D` (ISO week date)
- `YYYY-DDD` (ordinal date)
- `today`, `now`, `yesterday`, `tomorrow`

Rejected (on purpose): `04/31/2025`, `31-04-2025`, `April 31, 2025`.

## Tests

```bash
PYTHONPATH=src python3 -m unittest discover -s tests -v
```

Inspired by timeanddate.com.
