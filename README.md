# TimeWarp

Command-line date calculators, including **negative** spans (end before start).

Dates you **type** are **ISO 8601** only. There is no `MM/DD` vs `DD/MM`. Time of day is optional. `-q` and `--json` also print ISO 8601. Sky clocks (`sun`, rise/set, `moon` event times, `seasons`) use `HH:MM` plus a NATO zone letter (`17:52R`) instead of repeating the calendar date.

On a color TTY, human output uses **emoji** (🌞 🌙 ☄️ 🛰️ 🎉). Labels and clocks are ASCII so columns line up; glyphs sit at the **end of the line**. Putting ☀️/🌞 in the same cell as a time is what shoved the rest of the row over (terminals disagree with East Asian Width on VS16 sequences). `NO_COLOR`, `--no-color`, pipes, `-q`, and `--json` stay plain (IAU symbols, no emoji). `--color` belongs after the subcommand (`sun --color …`); it is also accepted on the parent parser.

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

Emoji reel (any key for the next command, `q` to quit):

```bash
./demos/emoji.sh
```

Editable install is still available:

```bash
python3 -m pip install -e /path/to/TimeWarp
```

### Portable zip (no Python install)

GitHub Actions builds an **onedir** folder per OS (`timewarp` + `_internal/`). Unzip and run `timewarp` / `timewarp.exe`. Same CLI; no Python on the machine.

```bash
python3 -m pip install -e ".[portable]"
python3 -m PyInstaller --noconfirm --clean timewarp.spec
dist/timewarp/timewarp --version
```

Artifacts: **Actions → portable** (manual run or a `v*` tag). Names look like `timewarp-1.1.1-linux-x86_64.zip`.

- **Windows:** `tzdata` is bundled so `--city` and `ZoneInfo` work. Settings go under `%APPDATA%\timewarp`; caches under `%LOCALAPPDATA%\timewarp`. SmartScreen may warn (unsigned).
- **macOS:** unsigned; first launch may need right-click → Open, or `xattr -d com.apple.quarantine timewarp`. arm64 from `macos-latest`.
- **Linux:** x86_64 glibc runner binary.

Nager / JPL / Celestrak still need a network (or a cache you already filled). US holidays, sun/moon/planets, and eclipses work offline.

`TIMEWARP_CACHE` and `TIMEWARP_*_DIR` still override paths (USB stick, etc.). See [Environment](#environment).

### Remembered settings

`--city`, `--tz`, `--lat`/`--lon`, `--color`/`--no-color`, `--holidays`, `--weekend`, and `--country` are stored with `save` (scriptable `load` / `unload`). They are **not** the TLE or SBDB file caches.

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

`timewarp cache save|load|unload|orbits` is the same family, nested. Aliases: `cache save` = `set`; `load` = `show`; `unload` = `clear`; `cache orbits` = `sbdb`.

Python's `locale` module does not list cities. Named places come from IANA tzdata (`zoneinfo` / `zone1970.tab`) plus US state capitals, Canadian provincial/territorial capitals, Mexican state capitals, and extras such as San Jose, CA. `timewarp cities` prints the full list.

Settings file: `$XDG_CONFIG_HOME/timewarp/cache.json` (or `~/.config/timewarp/cache.json`; on Windows `%APPDATA%\timewarp\cache.json`). Override with `TIMEWARP_CACHE`.

### Data caches (TLE, SBDB, holidays, Horizons)

These live under `~/.cache/timewarp/` (or `%LOCALAPPDATA%\timewarp\`). `rise` / `passes` do **not** auto-refresh the bulk dumps; you update them on purpose.

**TLE** (`TIMEWARP_TLE_DIR`, default `…/tle/`, 24 hours for Celestrak fetches):

```bash
timewarp save --tle path/to/iss.tle   # copy a 2-line or 3-line file into the cache
timewarp save --tle                   # refetch ISS from Celestrak
timewarp save --tle 25544             # refetch by catalog number
timewarp save --catalog visual        # refetch a Celestrak group
```

If `--tle` is an existing path, TimeWarp installs that file (and indexes each sat by name and catalog number). Otherwise the value is a Celestrak name or catalog number (default **ISS**). Combine `--tle FILE --catalog visual` to store the same text as that group. `--all` on `passes` lists every sat in the loaded set.

**SBDB dump** (`TIMEWARP_SBDB_DIR` / `TIMEWARP_SBDB_CATALOG`, default `…/sbdb/catalog-h11.json`):

```bash
timewarp cache orbits --refresh              # fetch from JPL sbdb_query.api
timewarp cache orbits --file dump.json       # install a downloaded JSON
timewarp cache sbdb --file dump.json         # same (alias)
timewarp rise --list-sb                      # names in the dump
timewarp rise --list-sb --refresh            # refetch, then list
```

`--file` accepts TimeWarp `{objects}` JSON or a raw JPL `{fields,data}` table. Copying either shape onto `catalog-h11.json` also works. `--file` and `--refresh` cannot be used together. Per-body files (`ceres.json`, `433.json`, …) still use a 7-day TTL and a one-shot `sbdb.api` lookup.

**Holidays:** Nager JSON `…/holidays/{CC}-{year}.json` for 30 days (`TIMEWARP_HOLIDAY_DIR`). `--refresh` on `holidays` / `calendar` / `workdays` refetches. US calendars are offline via python-holidays.

**Horizons moons:** `…/horizons/{name}.json`, 7 days (`TIMEWARP_HORIZONS_DIR`).

## Phase 1 — dates and durations

| Tab | Command | What it does |
|---|---|---|
| Count Days | `timewarp count START END` | Signed calendar duration (aliases: `between`, `duration`) |
| Add Days | `timewarp add [DATE] OFFSET...` | Add years, months, weeks, days, and optional time (DATE defaults to today) |
| Add Days | `timewarp sub [DATE] OFFSET...` | Subtract the same offset (alias: `subtract`) |
| Workdays | `timewarp workdays START END` | Signed Mon–Fri count (alias: `workday`) |
| Add Workdays | `timewarp add-workdays [DATE] N` | Move N business days (alias: `add-workday`) |
| Weekday | `timewarp weekday [DATE]` | English weekday + ISO weekday 1–7 |
| Week № | `timewarp week [DATE]` | ISO week date `YYYY-Www-D` (aliases: `weekno`, `week-number`) |

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

Workdays skip Saturday and Sunday by default. `--weekend Fri,Sat` changes that.

- `--holidays US` — US **federal** public holidays from [python-holidays](https://github.com/vacanza/holidays) (offline, including observed dates).
- `--holidays US --region IN` (or `US-IN` / `Indiana`) — that **state’s** calendar from the same library (Cesar Chavez in CA, Patriots’ Day in MA, etc.). There is **no city/county** overlay.
- `--holidays GB` / `calendar --country DE` — [Nager.Date](https://date.nager.at) JSON, cached under `~/.cache/timewarp/holidays/{CC}-{year}.json` for 30 days (`TIMEWARP_HOLIDAY_DIR` overrides). `--refresh` refetches. `--region` is an ISO 3166-2 subdivision from that year’s feed (`DE-BY`, `BY`, or `Bavaria`; `CA-ON` / `Ontario`; `AU-NSW` / `New South Wales`). GB defaults to England (`GB-ENG`); `--region GB-SCT` or `Scotland` selects Scotland. There is **no city/county** overlay.

`timewarp holidays 2026 --country US --region CA` lists them. `timewarp holidays 2026 --country DE --region BY` is Bavaria.

## Phase 2 — calendar, sky, and countdowns

| Screen | Command | What you get now |
|---|---|---|
| Create calendar | `timewarp calendar [YEAR]` | Year grid; US: python-holidays (federal or `--region` state); others: Nager.Date |
| Month sheet | `timewarp month [YYYY-MM] --city NAME` | One row per day: civil twilight, sunrise/set, moonrise/set, illumination |
| Countdown to Any Date | `timewarp countdown [DATE]` | Signed remaining time (negative if the date is past) |
| Today | `timewarp today --city NAME [DATE]` | One screen: weekday, holiday, sun, moon, RC note/color, ISS. Not the full `cycle` sheet |
| Sunrise & Sunset | `timewarp sun --city "New York" [DATE]` | Rise, noon, set, twilight, azimuth, day length |
| Moon phases | `timewarp moon [DATE]` | Phase, illumination, next new/full/quarter *times* |
| Seasons | `timewarp seasons [YEAR]` | March/September equinox, June/December solstice |
| Rise / set | `timewarp rise --city "New York" [DATE]` | Rise times as `HH:MM` + zone letter; `--13` / `--33` add +13°/+33° after rise and before set. Named extras plus SBDB ids (`433`) |
| Set | `timewarp set --city "New York" [DATE]` | Set times for the same bodies and period |
| Eclipse lookup | `timewarp eclipse [YEAR]` | Solar and lunar eclipses 1900–2199. Omit YEAR for the next 8 from today; `--limit N` caps the list |
| Rosicrucian cycle | `timewarp cycle [DATE]` | Year CE+1353; RC **day** starts at **local midnight**. Star date `3379.162`. `--born` adds Lewis periods. Alias: `rosicrucian` |
| Satellite passes | `timewarp passes [SAT] [DATE] --city NAME` | AOS / max / LOS vs twilight, moon, and visual mag; `--catalog visual`; `--tle FILE`; `--min-elev` (default 10°) |
| Help | `timewarp help [COMMAND]` | Overview, or one command’s usage (`--help` works too; alias: `?`) |

```bash
timewarp calendar 2026 --country US
timewarp calendar 2026 --iso          # Monday-first weeks
timewarp month 2026-07 --city Indianapolis
timewarp month --city Indianapolis --twilight
timewarp countdown 2026-12-31
timewarp today --city Indianapolis
timewarp today --city Indianapolis 2026-07-04
timewarp today --city Indianapolis 2019-12-10 --tle tests/data/iss.tle
timewarp sun --city "New York" 2026-07-04
timewarp sun --lat 40.7128 --lon -74.0060 --tz America/New_York
timewarp moon 2026-08-28
timewarp moon 2026-08-28 --city Indianapolis
timewarp seasons 2026
timewarp seasons 2026 --city Indianapolis
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
timewarp eclipse --limit 8
timewarp eclipse 1919
timewarp cycle 2026-08-29
timewarp cycle --born 1960-03-22 --city Indianapolis
timewarp rise ceres --city London
timewarp rise 433 --city London
timewarp cache orbits --refresh
timewarp cache orbits --file path/to/sbdb.json
timewarp rise --list-sb
timewarp rise --list-sb --refresh
timewarp rise iris --city London
timewarp rise io --city London
timewarp rise 67p --city London
timewarp passes --city Indianapolis
timewarp passes ISS --city "New York" --tle tests/data/iss.tle 2019-12-10
timewarp passes --catalog visual --city Indianapolis
timewarp passes --catalog visual --all --city Indianapolis --min-elev 20
timewarp save --tle tests/data/iss.tle
timewarp save --catalog visual
timewarp help rise
```

`timewarp cycle` (alias `rosicrucian`) prints the **Rosicrucian year**: Common Era + **1353**. The year is the **March equinox date**; the RC **day** (and that new year) starts at **local midnight**, not at the equinox instant. Default place is Greenwich. The **star date** is `YEAR.DDD` — midnights since the equinox-date midnight (`2026-08-29` → `3379.162`). A **1690-year** cycle starts at midnight on the **337 CE** equinox date (proleptic Gregorian) and rolls at the 2027 equinox midnight. Daily A–G letters and their **color period** (gold, green, orange, orchid, periwinkle, sky, coral) follow the public [AMORC cycles clock](https://cycles.amorc.org/en/cycles), counted from midnight. `--born` adds H. Spencer Lewis period *numbers* (7-year life, ~52-day yearly, soul grid from 22 March); the book’s essays are not copied. `-q` prints the star date.

`timewarp today --city NAME` is one local day: weekday, ISO week, holiday if any (US federal by default; `--holidays` / `--country` / `--region`), civil dawn/dusk and sunrise/set, moon phase plus moonrise/set, RC **star date + daily note and color** (not the 1690-year sheet or Lewis), a season or eclipse line only if it falls that day, and ISS passes (fail-soft if there is no TLE; `--tle FILE` installs nothing, it only reads). Place is required like `sun` (cached `--city` counts). `-q` prints date, weekday, sunrise–sunset, star date, and the note letter.

`sun` and rise/set human output is local `HH:MM` plus a NATO zone letter for the UTC offset: **Q** = UTC−4 (EDT), **R** = UTC−5 (EST), **Z** = UTC, and so on (`17:52R`, `18:52Q`, `13:00Z`). Keys in a view share one right-aligned column (Rise/Transit/Set line up with Body/Date/Place). Clock extras (azimuth, ISO timestamps) are a third column sized only from those rows, so a long Place line cannot stretch `moon`’s next-quarter dates. The civil date is not repeated on every rise/set cell; it is on the command line (yellow if assumed) or in the date column of a multi-day range. `--13` / `--33` are geometric altitudes above the horizon (not twilight). If the body never reaches that height, the cell is `—`. `-q` and `--json` still use full ISO timestamps.

`timewarp month YYYY-MM --city NAME` (alias `almanac`) is a printable month sheet: civil dawn/dusk (sun −6°), sunrise/set, day length, moonrise/set, illumination. `--twilight` adds nautical (−12°) and astronomical (−18°) columns. Omit the month to use this month (yellow on the reconstructed CLI).

Eclipse rows are computed with Meeus, *Astronomical Algorithms* ch. 54 (UTC date of greatest eclipse, 1900–2199). `timewarp eclipse` lists that year; omit the year for the next 8 from today; `--limit N` caps the list. `timewarp sun` uses the NOAA/USNO algorithm for rise, set, and civil/nautical/astronomical twilight (sun at −6°/−12°/−18°). `timewarp rise` and moon **event times** (new, quarters, full) use Paul Schlyter’s low-precision ephemeris (about 1–2 arcminutes). The named moon **phase** and illumination on `moon` still come from synodic age. `timewarp seasons` is when solar ecliptic longitude hits 0°/90°/180°/270°.

`timewarp rise` and `timewarp set` default to the sun, moon, and planets (Pluto only 1800–2100). Named extras: asteroids `ceres` `pallas` `juno` `vesta` `hygiea` `eros`; comets `halley` `encke` `tempel1` `67p`; moons `io` `europa` `ganymede` `callisto` `titan` `triton` `phobos` `deimos`. You can also pass a [JPL SBDB](https://ssd-api.jpl.nasa.gov/doc/sbdb.html) number or designation (`433`, `Bennu`). `timewarp cache orbits --refresh` downloads a dump of **numbered asteroids with H≤11** plus **numbered comets** (`sbdb_query.api`); `rise iris` then needs no extra fetch. `timewarp cache orbits --file dump.json` installs a dump you already downloaded (TimeWarp `{objects}` JSON or a raw JPL `{fields,data}` table) into `~/.cache/timewarp/sbdb/catalog-h11.json`. Copying either shape onto that path also works. `TIMEWARP_SBDB_CATALOG` points at another file. `rise --list-sb` lists the dump. That is still two-body Kepler, not Horizons/DE. With no body name they list every default object above the horizon that local day. Pass a second ISO date for an inclusive range. `--all` also prints default bodies that stay below the horizon. Asteroids and comets use two-body Keplerian orbits from SBDB (`~/.cache/timewarp/sbdb/`, 7 days, `TIMEWARP_SBDB_DIR` overrides); a frozen table is used if the named-body fetch fails. Named planetary moons use [JPL Horizons](https://ssd-api.jpl.nasa.gov/doc/horizons.html) osculating elements about the parent (`~/.cache/timewarp/horizons/`, `TIMEWARP_HORIZONS_DIR`); circular orbits if Horizons is unreachable. Good for rise/set, not spacecraft navigation.

`timewarp passes` uses SGP4 on a NORAD TLE (`pip install sgp4`, already in this project’s `.venv`). Default satellite is ISS. Without `--tle`, elements are fetched from Celestrak and cached under `~/.cache/timewarp/tle` for 24 hours (`TIMEWARP_TLE_DIR`). `--catalog visual` (or `stations`, `starlink`, `gps` → `gps-ops`, …) loads that Celestrak `GROUP`; omit a sat name, or pass `--all`, to list the whole group. Files may be **2-line** or **3-line** (optional name, then lines starting `1 ` / `2 `), including Space-Track 3LE. The catalog field (columns 3–7) is numeric **0–99999** or **Alpha-5** (**100000–339999**; letters skip I and O). Each pass lists acquisition, max elevation (time, altitude, azimuth), loss, the **sky bin** at max (day / civil / nautical / astronomical / night), moon altitude, angular **separation from the moon**, and an approximate **visual magnitude** from satcat RCS (range and phase; `satcat.csv` cached 7 days). `--min-elev` defaults to 10°. A TLE more than 14 days from the requested date prints a warning. `timewarp save --tle FILE` copies a dump into that cache without running a pass.

Later work is tracked in [GitHub issues](https://github.com/webaugur/TimeWarp/issues) and `TODO.md`.

## Aliases

| Command | Also |
|---|---|
| `count` | `between`, `duration` |
| `sub` | `subtract` |
| `workdays` | `workday` |
| `add-workdays` | `add-workday` |
| `week` | `weekno`, `week-number` |
| `month` | `almanac` |
| `cycle` | `rosicrucian` |
| `help` | `?` |
| `load` | `show` |
| `unload` | `clear` |
| `cache orbits` | `sbdb` |
| `cache save` / `cache load` / `cache unload` | `set` / `show` / `clear` |

`moonrise` is `rise moon`. `moonset` is `set moon`. Global flags: `--version`, `--color` / `--no-color` (also after the subcommand). Most commands take `-q` / `--json`.

## Environment

| Variable | What it overrides |
|---|---|
| `TIMEWARP_CACHE` | Remembered-settings JSON (`cache.json`) |
| `TIMEWARP_HOLIDAY_DIR` | Nager holiday JSON directory |
| `TIMEWARP_SBDB_DIR` | SBDB per-body JSON + default `catalog-h11.json` |
| `TIMEWARP_SBDB_CATALOG` | SBDB dump file (instead of `…/sbdb/catalog-h11.json`) |
| `TIMEWARP_HORIZONS_DIR` | Horizons moon-element JSON |
| `TIMEWARP_TLE_DIR` | TLE files and `satcat.csv` |
| `TIMEWARP_DEBUG` | `1` / `true` / `yes` → print a traceback on unexpected errors |
| `NO_COLOR` | Plain text (no emoji / ANSI), even on a TTY |
| `FORCE_COLOR` | `1` / `true` / `yes` → color / emoji even when piped |
| `XDG_CONFIG_HOME` / `XDG_CACHE_HOME` | Unix config / cache roots |
| `APPDATA` / `LOCALAPPDATA` | Windows settings / cache roots |

## Input formats

Accepted:

- `YYYY-MM` (month sheet)
- `YYYY-MM-DD`
- `YYYY-MM-DDTHH:MM[:SS][Z|+HH:MM]` (space instead of `T` is also accepted)
- `YYYY-Www-D` (ISO week date)
- `YYYY-DDD` (ordinal date)
- `today`, `now`, `yesterday`, `tomorrow`

Rejected (on purpose): `04/31/2025`, `31-04-2025`, `April 31, 2025`.

## Tests

```bash
PYTHONPATH=src python3 -m unittest discover -s tests -v
# holidays + satellite + color:  python3 -m venv .venv && .venv/bin/pip install -e .
```

MIT License — see `LICENSE`. Inspired by timeanddate.com.
