#!/bin/bash
# Start the clock on tty1. kms runs clockface directly. labwc starts a
# compositor first, then the desktop build of clockface.
set -euo pipefail

# shellcheck disable=SC1091
source /opt/clock/manifest.env

config="${CLOCK_CONFIG:-/home/${CLOCK_USER}/.config/clockface/clockface.toml}"
binary="${CLOCKFACE_BIN:-/usr/local/bin/clockface}"

drop_splash() {
  sleep 1
  if [[ "$(id -u)" -eq 0 ]]; then
    plymouth quit || true
    return
  fi
  sudo -n /usr/bin/plymouth quit || true
}

hide_cursor() {
  if [[ -c /dev/tty1 ]]; then
    setterm -blank 0 -powerdown 0 -powersave off -cursor off </dev/tty1 >/dev/tty1 || true
  fi
}

if [[ "${CLOCK_KIOSK}" == "labwc" ]]; then
  uid="$(id -u)"
  export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/${uid}}"
  if [[ ! -d "$XDG_RUNTIME_DIR" ]]; then
    install -d -m 700 "$XDG_RUNTIME_DIR"
  fi
  labwc >>/var/log/clock-labwc.log 2>&1 &
  lab_pid=$!
  display=""
  for _ in $(seq 1 100); do
    shopt -s nullglob
    sockets=("${XDG_RUNTIME_DIR}"/wayland-*)
    shopt -u nullglob
    if [[ -S "${sockets[0]:-}" ]]; then
      display="$(basename "${sockets[0]}")"
      break
    fi
    sleep 0.05
  done
  if [[ -z "$display" ]]; then
    echo "clock kiosk: labwc did not open a Wayland socket" >&2
    kill "$lab_pid" 2>/dev/null || true
    exit 1
  fi
  export WAYLAND_DISPLAY="$display"
  drop_splash
  set +e
  "$binary" --config "$config"
  code=$?
  set -e
  kill "$lab_pid" 2>/dev/null || true
  wait "$lab_pid" 2>/dev/null || true
  exit "$code"
fi

hide_cursor
"$binary" --config "$config" &
clock_pid=$!
drop_splash
wait "$clock_pid"
