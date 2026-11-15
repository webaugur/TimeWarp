#!/bin/bash
# labwc session startup. Background and rotation only. kiosk-start.sh runs clockface.
set -euo pipefail

# shellcheck disable=SC1091
source /opt/clock/manifest.env

swaybg -c '#000000' >/dev/null 2>&1 &

# CLOCK_ROTATION is clockwise. wl_output transforms count counter-clockwise
# (like the kernel's rotate=), so 90 clockwise is --transform 270.
case "${CLOCK_ROTATION}" in
  0) transform=normal ;;
  90) transform=270 ;;
  180) transform=180 ;;
  270) transform=90 ;;
  *) echo "clock labwc: bad rotation ${CLOCK_ROTATION}" >&2; exit 1 ;;
esac

output="${CLOCK_OUTPUT:-HDMI-A-1}"
if ! wlr-randr --output "$output" --transform "$transform"; then
  output="$(wlr-randr | awk 'NF && $1 !~ /^(Position|Transform|Scale|Modes|Enabled|Current)/ { print $1; exit }')"
  if [[ -n "${output}" ]]; then
    wlr-randr --output "$output" --transform "$transform"
  fi
fi
