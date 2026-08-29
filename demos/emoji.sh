#!/usr/bin/env bash
# Walk through TimeWarp human output with emoji on. Any key runs the next demo; q quits.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
TW="$ROOT/bin/timewarp"

if [[ ! -x $TW ]]; then
  echo "timewarp launcher missing at $TW" >&2
  exit 127
fi

# Force emoji even if stdout is a pipe; fixtures so we do not wait on NASA/Nager.
export FORCE_COLOR=1
unset NO_COLOR

HOLIDAY_DIR=$(mktemp -d)
SBDB_DIR=$(mktemp -d)
TLE_DIR=$(mktemp -d)
trap 'rm -rf "$HOLIDAY_DIR" "$SBDB_DIR" "$TLE_DIR"' EXIT
cp "$ROOT/tests/data/holidays-GB-2026.json" "$HOLIDAY_DIR/GB-2026.json"
cp "$ROOT/tests/data/holidays-DE-2026.json" "$HOLIDAY_DIR/DE-2026.json"
cp "$ROOT/tests/data/sbdb-ceres.json" "$SBDB_DIR/ceres.json"
cp "$ROOT/tests/data/sbdb-67p.json" "$SBDB_DIR/67p.json"
cp "$ROOT/tests/data/satcat-iss.csv" "$TLE_DIR/satcat.csv"
export TIMEWARP_HOLIDAY_DIR=$HOLIDAY_DIR
export TIMEWARP_SBDB_DIR=$SBDB_DIR
export TIMEWARP_TLE_DIR=$TLE_DIR

pause() {
  local key
  printf '\n\033[2mPress any key for next demo (q to quit)…\033[0m'
  # One keypress is the signal; do not sleep.
  IFS= read -r -n 1 -s key || true
  printf '\n\n'
  case "${key:-}" in
    q | Q) exit 0 ;;
  esac
}

banner() {
  printf '\033[1m%s\033[0m\n' "$1"
  printf '\033[36m  $ %s\033[0m\n\n' "$2"
}

run() {
  banner "$1" "$2"
  pause
  # FORCE_COLOR=1 (set above) turns emoji on even when stdout is not a TTY.
  bash -c "\"$TW\" $2"
  echo
}

printf '\n\033[1mTimeWarp emoji reel\033[0m\n'
printf 'TTY color + emoji. -q / --json stay plain ISO.\n'

run "☀️  Sunrise, twilight, azimuth (New York, Independence Day)" \
  'sun --city "New York" 2026-07-04'

run "🌙  Moon phase and next quarters" \
  'moon 2026-08-28 --city Indianapolis'

run "⬆️  Everything above the horizon that day (planet emoji row)" \
  'rise --city London 2026-07-04'

run "🪨  Ceres (SBDB Kepler)" \
  'rise ceres --city London 2026-07-04'

run "☄️  67P (SBDB Kepler)" \
  'rise 67p --city London 2026-07-04'

run "📅  Month sheet: civil twilight, sun, moon" \
  'month 2026-07 --city Indianapolis'

run "🌃  Same month with nautical + astronomical columns" \
  'month 2026-07 --city Indianapolis --twilight'

run "🎉  US state holidays (California overlay)" \
  'holidays 2026 --country US --region CA'

run "🎉  Nager subdivision: Bavaria (DE-BY)" \
  'holidays 2026 --country DE --region BY'

run "☀️  Equinoxes and solstices" \
  'seasons 2026 --city Indianapolis'

run "☀️🌙  Eclipse catalog" \
  'eclipse 2026'

run "🛰️  ISS pass vs twilight, moon, visual mag" \
  "passes ISS --city \"New York\" --tle \"$ROOT/tests/data/iss.tle\" 2019-12-10"

run "⏳  Countdown (signed remaining time)" \
  'countdown 2026-12-31T00:00:00'

run "❌  Error path (invalid date)" \
  'count 2026-05-31 2025-04-31 || true'

printf '\033[1mDone.\033[0m Modern terminals should have shown ☀️🌙🪐☄️🛰️🎉⏳❌\n'
