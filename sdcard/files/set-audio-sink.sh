#!/bin/bash
# Point PipeWire at the analog headphone jack (or CLOCK_AUDIO_SINK).
set -euo pipefail

# shellcheck disable=SC1091
source /opt/clock/manifest.env

export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"

want="${CLOCK_AUDIO_SINK:-analog}"
ready=0
for _ in $(seq 1 40); do
  if wpctl status >/dev/null 2>&1; then
    ready=1
    break
  fi
  sleep 0.5
done
if [[ "$ready" -ne 1 ]]; then
  echo "clock audio: pipewire is not up; ALSA card default still applies" >&2
  exit 0
fi

if [[ -n "$want" && "$want" != "analog" ]]; then
  wpctl set-default "$want"
  exit 0
fi

sink_id="$(
  wpctl status | awk '
    /Sinks:/ { sinks = 1; next }
    /Sources:/ { sinks = 0 }
    sinks && /[Aa]nalog|[Hh]eadphone/ && !/[Hh][Dd][Mm][Ii]/ {
      if (match($0, /[0-9]+/)) {
        print substr($0, RSTART, RLENGTH)
        exit
      }
    }
  '
)"
if [[ -z "$sink_id" ]]; then
  echo "clock audio: no analog sink in wpctl status" >&2
  exit 0
fi
wpctl set-default "$sink_id"
echo "clock audio: default sink ${sink_id}"
