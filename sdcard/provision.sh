#!/usr/bin/env bash
# First boot (and any later re-run) for the clock image. Idempotent.
# Installed at /opt/clock/provision.sh and started by clock-provision.service.
# Log: /var/log/clock-provision.log
set -euo pipefail

if [[ "$(id -u)" -ne 0 ]]; then
  echo "provision: run as root" >&2
  exit 1
fi

# shellcheck disable=SC1091
source /opt/clock/manifest.env

: "${CLOCK_HOSTNAME:?}"
: "${CLOCK_USER:?}"
: "${CLOCK_TIMEZONE:?}"
: "${CLOCK_LOCALE:?}"
: "${CLOCK_KEYBOARD:?}"
: "${CLOCK_ROTATION:?}"
: "${CLOCK_KIOSK:?}"
: "${CLOCK_AUDIO_SINK:?}"

LOG=/var/log/clock-provision.log
touch "$LOG"
chmod 0644 "$LOG"

log() {
  printf '%s %s\n' "$(date -Is)" "$*" | tee -a "$LOG"
}

log "provision start kiosk=${CLOCK_KIOSK} rotation=${CLOCK_ROTATION} user=${CLOCK_USER}"

export DEBIAN_FRONTEND=noninteractive

retry() {
  local attempt
  for attempt in 1 2 3 4 5 6; do
    if "$@"; then
      return 0
    fi
    log "command failed (attempt ${attempt}): $*"
    sleep 15
  done
  return 1
}

log "waiting for ${CLOCK_USER}"
for _ in $(seq 1 60); do
  if id "$CLOCK_USER" >/dev/null 2>&1; then
    break
  fi
  sleep 2
done
id "$CLOCK_USER" >/dev/null 2>&1 || {
  log "cloud-init did not create ${CLOCK_USER}"
  exit 1
}

if [[ -f /usr/share/zoneinfo/${CLOCK_TIMEZONE} ]]; then
  ln -sfn "/usr/share/zoneinfo/${CLOCK_TIMEZONE}" /etc/localtime
  printf '%s\n' "$CLOCK_TIMEZONE" > /etc/timezone
fi
hostnamectl set-hostname "$CLOCK_HOSTNAME" || printf '%s\n' "$CLOCK_HOSTNAME" > /etc/hostname

systemctl enable systemd-timesyncd || log "timesyncd not enabled yet"
systemctl start systemd-timesyncd || true
timedatectl set-ntp true || true

retry apt-get update
runtime_packages=(
  curl
  ca-certificates
  tzdata
  python3
  python3-venv
  python3-pip
  pipx
  libasound2t64
  libxkbcommon0
  libwayland-client0
  libwayland-cursor0
  libwayland-egl1
  libegl1
  libgles2
  libgbm1
  libgl1-mesa-dri
  libdrm2
  libinput10
  pipewire
  pipewire-alsa
  wireplumber
  dbus-user-session
  plymouth
  plymouth-themes
  rpd-plym-splash
  initramfs-tools
)
if [[ "$CLOCK_KIOSK" == "labwc" ]]; then
  runtime_packages+=(labwc wlr-randr swaybg)
fi
retry apt-get install -y "${runtime_packages[@]}"
systemctl enable systemd-timesyncd
systemctl start systemd-timesyncd || true
timedatectl set-ntp true || true

if [[ -f /etc/locale.gen ]]; then
  sed -i "s/^# *${CLOCK_LOCALE} UTF-8/${CLOCK_LOCALE} UTF-8/" /etc/locale.gen
  if ! grep -q "^${CLOCK_LOCALE} UTF-8" /etc/locale.gen; then
    echo "${CLOCK_LOCALE} UTF-8" >> /etc/locale.gen
  fi
  locale-gen "$CLOCK_LOCALE"
  update-locale "LANG=${CLOCK_LOCALE}"
fi
if [[ -f /etc/default/keyboard ]]; then
  sed -i "s/^XKBLAYOUT=.*/XKBLAYOUT=\"${CLOCK_KEYBOARD}\"/" /etc/default/keyboard
fi

for group in video render audio gpio spi input; do
  if ! getent group "$group" >/dev/null; then
    groupadd "$group"
  fi
  usermod -aG "$group" "$CLOCK_USER"
done

home="$(getent passwd "$CLOCK_USER" | cut -d: -f6)"
install -d -o "$CLOCK_USER" -g "$CLOCK_USER" -m 0755 \
  "${home}/.config/clockface" \
  "${home}/.config/timewarp" \
  "${home}/.config/systemd/user/default.target.wants" \
  "${home}/.local/bin" \
  "${home}/.local/share/timewarp"

if [[ ! -f "${home}/.config/timewarp/cache.json" ]]; then
  install -o "$CLOCK_USER" -g "$CLOCK_USER" -m 0644 \
    /opt/clock/cache.json "${home}/.config/timewarp/cache.json"
fi
if [[ ! -f "${home}/.config/clockface/clockface.toml" ]]; then
  install -o "$CLOCK_USER" -g "$CLOCK_USER" -m 0644 \
    /opt/clock/clockface.toml "${home}/.config/clockface/clockface.toml"
fi

venv="${home}/.local/share/timewarp/venv"
if [[ ! -x "${venv}/bin/pip" ]]; then
  sudo -u "$CLOCK_USER" python3 -m venv "$venv"
fi
retry sudo -u "$CLOCK_USER" "${venv}/bin/pip" install --upgrade pip
# pip builds in the source tree (build/, *.egg-info). /opt/clock/timewarp is
# root-owned, so install from a copy the kiosk user owns.
tw_src="$(mktemp -d /var/tmp/clock-timewarp.XXXXXX)"
cp -a /opt/clock/timewarp/. "$tw_src/"
chown -R "$CLOCK_USER:$CLOCK_USER" "$tw_src"
retry sudo -u "$CLOCK_USER" "${venv}/bin/pip" install "$tw_src"
rm -rf "$tw_src"
ln -sfn "${venv}/bin/timewarp" "${home}/.local/bin/timewarp"
ln -sfn "${venv}/bin/timewarp" /usr/local/bin/timewarp
chown -h "$CLOCK_USER:$CLOCK_USER" "${home}/.local/bin/timewarp" || true

cat > /etc/profile.d/clock-path.sh <<'EOF'
# The kiosk and SSH logins both need the timewarp launcher.
export PATH="$HOME/.local/bin:/usr/local/bin:$PATH"
EOF
chmod 0644 /etc/profile.d/clock-path.sh

# Analog jack. Card name "Headphones" is the bcm2835 3.5 mm output on this Pi.
# HDMI is a different card and is not the default. PipeWire is pointed at the
# same output by set-audio-sink.sh once the user session is up.
card_id="Headphones"
if [[ -r /proc/asound/cards ]]; then
  detected="$(
    awk '
      /HDMI/ { next }
      /\[/ {
        line = $0
        sub(/^.*\[/, "", line)
        sub(/\].*$/, "", line)
        gsub(/ /, "", line)
        if (line != "") { print line; exit }
      }
    ' /proc/asound/cards
  )"
  if [[ -n "${detected}" ]]; then
    card_id="$detected"
  fi
fi
cat > /etc/asound.conf <<EOF
pcm.!default {
  type plug
  slave.pcm {
    type hw
    card ${card_id}
    device 0
  }
}
ctl.!default {
  type hw
  card ${card_id}
}
EOF
log "ALSA default card ${card_id} device 0"

install -d /etc/wireplumber/wireplumber.conf.d
cat > /etc/wireplumber/wireplumber.conf.d/51-clock-analog.conf <<'EOF'
monitor.alsa.rules = [
  {
    matches = [
      { device.name = "~alsa_card.*[Hh][Dd][Mm][Ii].*" }
    ]
    actions = {
      update-props = {
        device.disabled = true
      }
    }
  }
]
EOF

cat > "${home}/.config/systemd/user/clock-audio.service" <<'EOF'
[Unit]
Description=Prefer the analog headphone jack
After=wireplumber.service pipewire.service

[Service]
Type=oneshot
ExecStart=/opt/clock/bin/set-audio-sink.sh
RemainAfterExit=yes

[Install]
WantedBy=default.target
EOF
chown "$CLOCK_USER:$CLOCK_USER" "${home}/.config/systemd/user/clock-audio.service"
ln -sfn "${home}/.config/systemd/user/clock-audio.service" \
  "${home}/.config/systemd/user/default.target.wants/clock-audio.service"
chown -h "$CLOCK_USER:$CLOCK_USER" \
  "${home}/.config/systemd/user/default.target.wants/clock-audio.service" || true
loginctl enable-linger "$CLOCK_USER" || true

if [[ -f /etc/kbd/config ]]; then
  sed -i 's/^BLANK_TIME=.*/BLANK_TIME=0/' /etc/kbd/config || true
  sed -i 's/^BLANK_DPMS=.*/BLANK_DPMS=off/' /etc/kbd/config || true
  sed -i 's/^POWERDOWN_TIME=.*/POWERDOWN_TIME=0/' /etc/kbd/config || true
fi
install -d /etc/systemd/logind.conf.d
cat > /etc/systemd/logind.conf.d/clock.conf <<'EOF'
[Login]
IdleAction=ignore
EOF

uid="$(id -u "$CLOCK_USER")"
install -d "/run/user/${uid}"
chown "$CLOCK_USER:$CLOCK_USER" "/run/user/${uid}"
chmod 0700 "/run/user/${uid}"

if [[ "$CLOCK_KIOSK" == "kms" ]]; then
  cat > /etc/systemd/system/clockface.service <<EOF
[Unit]
Description=Grandfather clock (KMS)
After=clock-provision.service systemd-user-sessions.service plymouth-start.service
Conflicts=getty@tty1.service
ConditionPathExists=/var/lib/clock-provision/done

[Service]
User=${CLOCK_USER}
Group=${CLOCK_USER}
SupplementaryGroups=video render audio gpio spi input
TTYPath=/dev/tty1
StandardInput=tty-force
StandardOutput=journal
StandardError=journal
WorkingDirectory=${home}
Environment=PATH=${home}/.local/bin:/usr/local/bin:/usr/bin:/bin
Environment=XDG_RUNTIME_DIR=/run/user/${uid}
Environment=EBOARD_BACKEND=kms
Environment=EBOARD_KMS_CONNECTOR=HDMI-A-1
ExecStartPre=/opt/clock/bin/wait-for-pipewire.sh
ExecStart=/opt/clock/bin/kiosk-start.sh
ExecStartPost=+/bin/sh -c 'sleep 1; /usr/bin/plymouth quit || true'
Restart=always
RestartSec=2

[Install]
WantedBy=multi-user.target
EOF
  rm -f /etc/profile.d/clock-kiosk.sh
else
  install -d -o "$CLOCK_USER" -g "$CLOCK_USER" -m 0755 "${home}/.config/labwc"
  cat > "${home}/.config/labwc/autostart" <<'EOF'
/opt/clock/bin/labwc-autostart.sh
EOF
  cat > "${home}/.config/labwc/rc.xml" <<'EOF'
<?xml version="1.0"?>
<labwc_config>
  <core>
    <gap>0</gap>
  </core>
</labwc_config>
EOF
  chown -R "$CLOCK_USER:$CLOCK_USER" "${home}/.config/labwc"
  install -d /etc/systemd/system/getty@tty1.service.d
  cat > /etc/systemd/system/getty@tty1.service.d/autologin.conf <<EOF
[Service]
ExecStart=
ExecStart=-/sbin/agetty --autologin ${CLOCK_USER} --noclear %I linux
EOF
  cat > /etc/profile.d/clock-kiosk.sh <<'EOF'
# POSIX: /etc/profile is sourced by /bin/sh, not bash.
if [ -z "${SSH_CONNECTION:-}" ] && [ "$(tty 2>/dev/null || true)" = /dev/tty1 ]; then
  exec /opt/clock/bin/kiosk-start.sh
fi
EOF
  chmod 0644 /etc/profile.d/clock-kiosk.sh
  rm -f /etc/systemd/system/clockface.service
fi

cat > /opt/clock/bin/wait-for-pipewire.sh <<EOF
#!/bin/bash
set -euo pipefail
socket="/run/user/${uid}/pipewire-0"
for _ in \$(seq 1 40); do
  if [[ -S "\$socket" ]]; then
    exit 0
  fi
  sleep 0.25
done
exit 0
EOF
chmod 0755 /opt/clock/bin/wait-for-pipewire.sh

# Re-apply splash tokens in case raspi-config rewrote cmdline, then rebuild
# the initramfs with MODULES=most (Trixie sometimes ignores a new theme until
# the initramfs actually contains the DRM modules).
if command -v raspi-config >/dev/null 2>&1; then
  raspi-config nonint do_boot_splash 0 || log "raspi-config do_boot_splash returned non-zero"
fi
if [[ -f /boot/firmware/cmdline.txt ]]; then
  python3 /opt/clock/lib/boot_config.py cmdline \
    --kiosk "$CLOCK_KIOSK" \
    --rotation "$CLOCK_ROTATION" \
    --instance-id "${CLOCK_INSTANCE:-clock-sdcard}" \
    --wifi-country "${CLOCK_WIFI_COUNTRY:-}" \
    --input /boot/firmware/cmdline.txt > /boot/firmware/cmdline.txt.new
  mv /boot/firmware/cmdline.txt.new /boot/firmware/cmdline.txt
fi
if [[ -f /etc/initramfs-tools/initramfs.conf ]]; then
  sed -i 's/^MODULES=.*/MODULES=most/' /etc/initramfs-tools/initramfs.conf
fi
if command -v plymouth-set-default-theme >/dev/null 2>&1; then
  plymouth-set-default-theme clockface || true
  plymouth-set-default-theme --rebuild-initrd clockface
fi

install -d /var/lib/clock-provision
date -Is > /var/lib/clock-provision/done
if [[ "$CLOCK_KIOSK" == "kms" ]]; then
  systemctl enable clockface.service
fi
systemctl disable clock-provision.service || true
log "provision finished"
if [[ "$CLOCK_KIOSK" == "kms" ]]; then
  systemctl start clockface.service || log "clockface did not start; it will start on the next boot"
fi
