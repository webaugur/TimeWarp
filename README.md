# TimeWarp

Command-line date calculators, including **negative** spans (end before start).

Dates you **type** are **ISO 8601** only. There is no `MM/DD` vs `DD/MM`. Time of day is optional. `-q` and `--json` also print ISO 8601. Rise/set tables use a short clock (`17:52R`) instead of a full timestamp.

```
timewarp add P7M6D
timewarp add 2026-07-04 7 months 6 days
timewarp between 2026-05-31 2025-04-30
```

The last example is the reason this exists: the end date is earlier, so the duration is negative (`-P1Y1M`, −396 days).

If you omit a date, TimeWarp uses **today** and writes that date in **yellow** on the reconstructed command line (stderr). Cached flags (`--city`, …) are **pink**; what you typed is white.

`2025-04-31` is not a date (April has 30 days). TimeWarp refuses it instead of rolling the extra day into May. Errors print `timewarp: …` on stderr and exit 2 (no traceback unless `TIMEWARP_DEBUG=1`).

```bash
timewarp help
timewarp help add
timewarp --help
```

## Install

Launcher in this tree, linked onto PATH:

```bash
ln -sfn /path/to/TimeWarp/bin/timewarp ~/.local/bin/timewarp
timewarp --help
```

`~/.local/bin` must be on your `PATH`. The script uses `.venv/bin/python` if that environment exists, otherwise `PYTHONPATH=src python3 -m timewarp`.

Editable install is still available:

```bash
python3 -m pip install -e /path/to/TimeWarp
```

### Remembered settings

`--city`, `--tz`, `--lat`/`--lon`, `--color`/`--no-color`, `--holidays`, `--weekend`, and `--country` are stored with the `cache` command (scriptable load/unload):

```bash
timewarp save --city Indianapolis
timewarp load
# stdout: timewarp --city Indianapolis

timewarp rise
# stderr: timewarp --city Indianapolis rise 2026-08-22
# (city pink if cached; date yellow if assumed)

eval "timewarp rise $(timewarp load -q)"
```

```bash
timewarp unload --city     # drop only the city
timewarp unload city       # same, positional (scripts)
timewarp unload            # drop every stored setting
```

`timewarp cache save|load|unload` is the same command, nested.

Python's `locale` module does not list cities. Named places come from IANA tzdata (`zoneinfo` / `zone1970.tab`) plus US state capitals, Canadian provincial/territorial capitals, Mexican state capitals, and extras such as San Jose, CA. `timewarp cities` prints the full list.

Cache file: `$XDG_CONFIG_HOME/timewarp/cache.json` (or `~/.config/timewarp/cache.json`). Override with `TIMEWARP_CACHE`.

## Phase 1 — dates and durations

| Tab | Command | What it does |
|---|---|---|
| Count Days | `timewarp count START END` | Signed calendar duration (alias: `between`) |
| Add Days | `timewarp add [DATE] OFFSET...` | Add years, months, weeks, days, and optional time (DATE defaults to today) |
| Add Days | `timewarp sub [DATE] OFFSET...` | Subtract the same offset |
| Workdays | `timewarp workdays START END` | Signed Mon–Fri count |
| Add Workdays | `timewarp add-workdays [DATE] N` | Move N business days |
| Weekday | `timewarp weekday [DATE]` | English weekday + ISO weekday 1–7 |
| Week № | `timewarp week [DATE]` | ISO week date `YYYY-Www-D` |

```bash
timewarp add P7M6D
timewarp add 7 years 6 months
timewarp add 7-6-13              # 7 years, 6 months, 13 days
timewarp add 2026-07-04 7 months 6 days
# Result: 2027-02-10 Saturday

timewarp add 2026-07-04T09:00:00 P7M6DT3H
# Result: 2027-02-10T12:00:00

timewarp count 2026-05-31 2025-04-30
# Duration: -(1 year, 1 month)
# ISO 8601: -P1Y1M
# Total days: -396

timewarp workdays 2026-01-01 2026-01-31 --holidays US
timewarp add-workdays 10 --holidays US
timewarp add-workdays 2026-07-04 10 --holidays US
timewarp weekday
timewarp week 2026-07-04
```

`--include-end` counts the end date (one extra calendar day in the same direction).

`--json` prints a machine-readable object. `-q` prints one ISO 8601 line.

Offsets may be words (`7 months 6 days`, `3 hours`), ISO 8601 durations (`P7M6D`, `PT3H`, `-P10D`), or compact `Y-M-D` with a 1–3 digit year (`7-6-13`). A four-digit year is always a date (`0007-06-13` is year 7, and still needs an offset). Units are applied largest-first: years, months, weeks, days, hours, minutes, seconds. Month-end days clamp: `2026-01-31` + 1 month = `2026-02-28`.

Bare `m` is not a unit (months vs minutes). Use `mo` or `min`.

Workdays skip Saturday and Sunday by default. `--weekend Fri,Sat` changes that. `--holidays US` also skips observed US federal holidays.

## Phase 2 — calendar, sky, and countdowns

| Screen | Command | What you get now |
|---|---|---|
| Create calendar | `timewarp calendar [YEAR]` | Year grid, US holidays marked, ISO dates listed |
| Countdown to Any Date | `timewarp countdown [DATE]` | Signed remaining time (negative if the date is past) |
| Sunrise & Sunset | `timewarp sun --city "New York" [DATE]` | Rise, noon, set, day length |
| Moon phases | `timewarp moon [DATE]` | Phase name, illumination, next new/full |
| Rise / set | `timewarp rise --city "New York" [DATE]` | Rise times as `HH:MM` + zone letter; `--13` / `--33` add +13°/+33° after rise and before set |
| Set | `timewarp set --city "New York" [DATE]` | Set times for the same bodies and period |
| Eclipse lookup | `timewarp eclipse [YEAR]` | Solar and lunar eclipses 2021–2030 |
| Help | `timewarp help [COMMAND]` | Overview, or one command’s usage (`--help` works too) |

```bash
timewarp calendar 2026 --country US
timewarp calendar 2026 --iso          # Monday-first weeks
timewarp countdown 2026-12-31
timewarp sun --city "New York" 2026-07-04
timewarp sun --lat 40.7128 --lon -74.0060 --tz America/New_York
timewarp moon 2026-08-28
timewarp rise --city "New York"
timewarp rise --city "New York" 2026-07-04
timewarp rise --city Indianapolis --13 --33
timewarp rise --city "New York" 2026-07-04 2026-07-10
timewarp rise moon --city "New York" 2026-07-04
timewarp moonrise --city London 2026-08-28
timewarp set --city "New York" 2026-07-04
timewarp set venus --city London
timewarp moonset --city London 2026-08-28
timewarp rise --all --city "New York" 2026-07-04
timewarp eclipse 2026
timewarp help rise
```

Rise/set human output is local `HH:MM` plus a NATO zone letter for the UTC offset: **Q** = UTC−4 (EDT), **R** = UTC−5 (EST), **Z** = UTC, and so on (`17:52R`, `18:52Q`, `13:00Z`). The civil date is not repeated on every cell; it is on the command line (yellow if assumed) or in the date column of a multi-day range. `--13` / `--33` are geometric altitudes above the horizon (not twilight). If the body never reaches that height, the cell is `—`. `-q` and `--json` still use full ISO timestamps.

Eclipse rows are from Fred Espenak / NASA GSFC decade tables (2021–2030). `timewarp sun` uses the NOAA/USNO sunrise algorithm. `timewarp rise` uses Paul Schlyter’s low-precision ephemeris (about 1–2 arcminutes; rise/set typically within a few minutes of almanac times). Moon phase uses synodic age.

`timewarp rise` and `timewarp set` share the same bodies: sun, moon, mercury, venus, mars, jupiter, saturn, uranus, neptune, pluto (Pluto only 1800–2100). With no body name they list every object that is above the horizon during the local day (today if you omit the date). Pass a second ISO date for an inclusive range. `--all` also prints bodies that stay below the horizon. Comets, asteroids, and planetary moons need extra orbital data and are deferred.

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
