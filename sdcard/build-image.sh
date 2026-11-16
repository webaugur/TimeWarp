#!/usr/bin/env bash
# Build a Raspberry Pi OS Lite image that boots into clockface.
#
# Downloads the pinned Trixie arm64 Lite image, checks it against the official
# .sha256 (and the pin in config), then customizes an offline copy: cloud-init
# user-data / network-config, config.txt, cmdline.txt, the cross-built binary,
# timewarp sources, and a first-boot provisioner. Optionally writes a block
# device after a confirmation that refuses mounted and system disks.
set -euo pipefail

SDCARD="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${SDCARD}/.." && pwd)"
CONFIG="${SDCARD}/config.example.env"
DRY_RUN=0
DEVICE=""
OUTPUT=""
SKIP_CROSS=0
SKIP_THEME=0
ALLOW_SYNTHETIC=0
WIZARD_SRC_FLAG=""

usage() {
  cat <<'EOF'
Usage: build-image.sh [options]

  --config FILE     shell config (default: sdcard/config.example.env)
  --output FILE     customized image path (default: sdcard/out/clock.img)
  --device PATH     also write the image to a block device
  --dry-run         print the plan; do not download, mount, or write
  --skip-cross      use sdcard/out/clockface instead of cross-build.sh
  --skip-theme      use sdcard/out/plymouth instead of regenerating it
  --wizard-src PATH local IndianaDell overlay or wizard PNG
  --allow-synthetic permit a stand-in wizard (not for a card you will boot)
  -h, --help

Passwords are never read from the config file. Export CLOCK_PASSWORD and, when
CLOCK_WIFI_SSID is set, CLOCK_WIFI_PSK. A terminal is prompted otherwise.

A real build needs root (loop mounts):
  sudo --preserve-env=CLOCK_PASSWORD,CLOCK_WIFI_PSK ./build-image.sh
EOF
}

die() {
  echo "build-image: $*" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --config)
      CONFIG="$2"
      shift 2
      ;;
    --output)
      OUTPUT="$2"
      shift 2
      ;;
    --device)
      DEVICE="$2"
      shift 2
      ;;
    --dry-run) DRY_RUN=1; shift ;;
    --skip-cross) SKIP_CROSS=1; shift ;;
    --skip-theme) SKIP_THEME=1; shift ;;
    --allow-synthetic) ALLOW_SYNTHETIC=1; shift ;;
    --wizard-src)
      WIZARD_SRC_FLAG="$2"
      shift 2
      ;;
    -h | --help) usage; exit 0 ;;
    *) die "unknown argument: $1" ;;
  esac
done

[[ -f "$CONFIG" ]] || die "config not found: ${CONFIG}"
# shellcheck disable=SC1090
set -a
# shellcheck disable=SC1090
source "$CONFIG"
set +a

if [[ -n "$WIZARD_SRC_FLAG" ]]; then
  WIZARD_SRC="$WIZARD_SRC_FLAG"
fi

: "${PI_OS_URL:?PI_OS_URL is required}"
: "${PI_OS_SHA256:?PI_OS_SHA256 is required}"
: "${CLOCK_HOSTNAME:=clock}"
: "${CLOCK_USER:=clock}"
: "${CLOCK_SSH:=1}"
: "${CLOCK_WIFI_SSID:=}"
: "${CLOCK_WIFI_COUNTRY:=US}"
: "${CLOCK_WIFI_HIDDEN:=0}"
: "${CLOCK_TIMEZONE:=America/Indiana/Indianapolis}"
: "${CLOCK_LOCALE:=en_US.UTF-8}"
: "${CLOCK_KEYBOARD:=us}"
: "${CLOCK_ROTATION:=90}"
: "${CLOCK_KIOSK:=kms}"
: "${CLOCK_AUDIO_SINK:=analog}"
: "${CLOCKFACE_REF:=}"
: "${WIZARD_SRC:=}"
: "${CLOCK_SSH_AUTHORIZED_KEYS:=}"

validate() {
  case "$CLOCK_KIOSK" in
    kms | labwc) ;;
    *) die "CLOCK_KIOSK must be kms or labwc" ;;
  esac
  case "$CLOCK_ROTATION" in
    0 | 90 | 180 | 270) ;;
    *) die "CLOCK_ROTATION must be 0, 90, 180, or 270" ;;
  esac
  [[ "$CLOCK_HOSTNAME" =~ ^[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?$ ]] \
    || die "bad hostname ${CLOCK_HOSTNAME}"
  [[ "$CLOCK_USER" =~ ^[a-z_][a-z0-9_-]*$ ]] || die "bad user ${CLOCK_USER}"
  [[ "$CLOCK_SSH" == "0" || "$CLOCK_SSH" == "1" ]] || die "CLOCK_SSH must be 0 or 1"
  [[ "$PI_OS_SHA256" =~ ^[0-9a-fA-F]{64}$ ]] || die "PI_OS_SHA256 is not a sha256 hex string"
  if [[ -n "$CLOCK_WIFI_COUNTRY" && ! "$CLOCK_WIFI_COUNTRY" =~ ^[A-Za-z]{2}$ ]]; then
    die "CLOCK_WIFI_COUNTRY must be two letters"
  fi
  if [[ -n "$CLOCK_SSH_AUTHORIZED_KEYS" && ! -f "$CLOCK_SSH_AUTHORIZED_KEYS" ]]; then
    die "authorized keys file not found: ${CLOCK_SSH_AUTHORIZED_KEYS}"
  fi
}

validate

video_line="$(
  PYTHONPATH="${SDCARD}/lib${PYTHONPATH:+:$PYTHONPATH}" \
    python3 -c 'import sys; from boot_config import video_token; print(video_token(int(sys.argv[1]), sys.argv[2]))' \
    "$CLOCK_ROTATION" "$CLOCK_KIOSK"
)"

echo "image:  ${PI_OS_URL}"
echo "sha256: ${PI_OS_SHA256} (must match the official .sha256)"
echo "host:   ${CLOCK_HOSTNAME}  user: ${CLOCK_USER}  tz: ${CLOCK_TIMEZONE}"
echo "kiosk:  ${CLOCK_KIOSK}  rotation: ${CLOCK_ROTATION}"
echo "video:  ${video_line}"
echo "audio:  ${CLOCK_AUDIO_SINK}"
if [[ -n "$CLOCKFACE_REF" ]]; then
  echo "clockface ref: ${CLOCKFACE_REF}"
else
  echo "clockface ref: working tree"
fi
if [[ -n "$CLOCK_WIFI_SSID" ]]; then
  echo "wifi:   ${CLOCK_WIFI_SSID} (${CLOCK_WIFI_COUNTRY})"
else
  echo "wifi:   not configured"
fi

if [[ "$DRY_RUN" -eq 1 ]]; then
  echo
  echo "dry-run: would cross-build (unless --skip-cross), generate the Plymouth theme,"
  echo "download and verify the image, loop-mount it, and write cloud-init plus /opt/clock."
  if [[ -n "$DEVICE" ]]; then
    echo "dry-run: would refuse to write ${DEVICE} without confirmation, and would refuse"
    echo "a mounted disk or the disk that holds /."
  fi
  cross_dry=(--dry-run)
  if [[ "$CLOCK_KIOSK" == "labwc" ]]; then
    cross_dry+=(--labwc)
  fi
  CLOCKFACE_REF="${CLOCKFACE_REF}" "${SDCARD}/cross-build.sh" "${cross_dry[@]}"
  exit 0
fi

if [[ "$(id -u)" -ne 0 ]]; then
  die "run as root so losetup can mount the image: sudo --preserve-env=CLOCK_PASSWORD,CLOCK_WIFI_PSK ${SDCARD}/build-image.sh"
fi

prompt_secret() {
  local var_name="$1"
  local label="$2"
  local first second
  if [[ -n "${!var_name:-}" ]]; then
    return 0
  fi
  if [[ ! -t 0 ]]; then
    die "set ${var_name} in the environment (it is not stored in the config file)"
  fi
  read -r -s -p "${label}: " first
  echo
  read -r -s -p "again: " second
  echo
  [[ "$first" == "$second" ]] || die "${label} entries did not match"
  [[ -n "$first" ]] || die "${label} was empty"
  printf -v "$var_name" '%s' "$first"
}

prompt_secret CLOCK_PASSWORD "Password for ${CLOCK_USER}"
if [[ -n "$CLOCK_WIFI_SSID" ]]; then
  prompt_secret CLOCK_WIFI_PSK "Wi-Fi password for ${CLOCK_WIFI_SSID}"
fi

if ! CLOCK_PASSWORD_HASH="$(printf '%s' "$CLOCK_PASSWORD" | openssl passwd -6 -stdin)"; then
  die "openssl passwd failed"
fi
unset CLOCK_PASSWORD

CACHE="${SDCARD}/cache"
mkdir -p "$CACHE" "${SDCARD}/out"
XZ_NAME="$(basename "$PI_OS_URL")"
XZ_PATH="${CACHE}/${XZ_NAME}"
if [[ ! -f "$XZ_PATH" ]]; then
  echo "downloading ${PI_OS_URL}"
  curl -fL --retry 3 --retry-delay 2 -o "${XZ_PATH}.partial" "$PI_OS_URL"
  mv "${XZ_PATH}.partial" "$XZ_PATH"
fi
official_hash="$(curl -fsSL "${PI_OS_URL}.sha256" | awk 'NR==1 { print $1 }')"
if [[ "${official_hash,,}" != "${PI_OS_SHA256,,}" ]]; then
  die "official checksum ${official_hash} does not match pinned PI_OS_SHA256 ${PI_OS_SHA256}"
fi
printf '%s  %s\n' "$PI_OS_SHA256" "$XZ_PATH" | sha256sum -c -

PRISTINE="${CACHE}/${XZ_NAME%.xz}"
if [[ ! -f "$PRISTINE" ]]; then
  echo "decompressing ${XZ_PATH}"
  xz -dk "$XZ_PATH"
fi

WORK="${OUTPUT:-${SDCARD}/out/clock.img}"
mkdir -p "$(dirname "$WORK")"
if [[ "$WORK" != "$PRISTINE" ]]; then
  echo "copying image to ${WORK}"
  cp --sparse=always "$PRISTINE" "$WORK"
fi

if [[ "$SKIP_CROSS" -eq 0 ]]; then
  cross_args=()
  if [[ "$CLOCK_KIOSK" == "labwc" ]]; then
    cross_args+=(--labwc)
  fi
  CLOCKFACE_REF="${CLOCKFACE_REF}" "${SDCARD}/cross-build.sh" "${cross_args[@]}"
fi
[[ -x "${SDCARD}/out/clockface" ]] || die "missing ${SDCARD}/out/clockface (cross-build failed or --skip-cross)"
[[ -f "${SDCARD}/out/clockface.features" ]] || die "missing clockface.features"

THEME="${SDCARD}/out/plymouth"
if [[ "$SKIP_THEME" -eq 0 ]]; then
  theme_args=(
    --out "$THEME"
    --rotation "$CLOCK_ROTATION"
    --kiosk "$CLOCK_KIOSK"
    --cache "${CACHE}/theme"
  )
  if [[ -n "$WIZARD_SRC" ]]; then
    theme_args+=(--wizard-src "$WIZARD_SRC")
  fi
  if [[ "$ALLOW_SYNTHETIC" -eq 1 ]]; then
    theme_args+=(--synthetic)
  fi
  python3 "${SDCARD}/make-boot-theme.py" "${theme_args[@]}"
fi
[[ -f "${THEME}/clockface.plymouth" ]] || die "missing Plymouth theme in ${THEME}"

# shellcheck disable=SC1091
source "${SDCARD}/out/clockface.features"
AUDIO_FLAG="no"
TW_FLAG=(--no-timewarp-bin)
if [[ "${audio:-no}" == "yes" ]]; then
  AUDIO_FLAG="yes"
fi
if [[ "${timewarp_bin:-no}" == "yes" ]]; then
  TW_FLAG=(--timewarp-bin /usr/local/bin/timewarp)
fi
# A features file from before the LED work has no leds= line: leave [leds] out.
LEDS_FLAG="${leds:-omit}"
# display=yes when this checkout accepts [display]. Older features files omit it.
ROTATION_ARGS=()
if [[ "${display:-no}" == "yes" ]]; then
  ROTATION_ARGS=(--rotation "$CLOCK_ROTATION")
fi
EXAMPLE_TOML="${SDCARD}/out/clockface.example.toml"
if [[ ! -f "$EXAMPLE_TOML" ]]; then
  EXAMPLE_TOML="${REPO}/clockface/clockface.example.toml"
fi

INSTANCE="clock-sdcard-$(date +%s)"
GIT_REV="${git:-unknown}"

LOOP=""
BOOT_MNT=""
ROOT_MNT=""
cleanup() {
  if [[ -n "$BOOT_MNT" ]] && mountpoint -q "$BOOT_MNT"; then
    umount "$BOOT_MNT" || true
  fi
  if [[ -n "$ROOT_MNT" ]] && mountpoint -q "$ROOT_MNT"; then
    umount "$ROOT_MNT" || true
  fi
  if [[ -n "$LOOP" ]]; then
    losetup -d "$LOOP" || true
  fi
  if [[ -n "$BOOT_MNT" ]]; then
    rmdir "$BOOT_MNT" 2>/dev/null || true
  fi
  if [[ -n "$ROOT_MNT" ]]; then
    rmdir "$ROOT_MNT" 2>/dev/null || true
  fi
}
trap cleanup EXIT

# Loop devices are often built with max_part=0, so partition nodes never
# appear. The boot partition is edited with mtools (no vfat module). The root
# filesystem is mounted by MBR offset.
command -v mcopy >/dev/null 2>&1 || die "install mtools to edit the FAT boot partition"
mapfile -t PARTS < <(python3 "${SDCARD}/lib/boot_config.py" partitions --image "$WORK")
[[ "${#PARTS[@]}" -ge 2 ]] || die "could not read two partitions from ${WORK}"
read -r _boot_type BOOT_START _boot_secs <<<"${PARTS[0]}"
read -r _root_type ROOT_START ROOT_SECS <<<"${PARTS[1]}"
BOOT_OFFSET=$((BOOT_START * 512))
BOOT_MNT="$(mktemp -d /tmp/clock-boot.XXXXXX)"
ROOT_MNT="$(mktemp -d /tmp/clock-root.XXXXXX)"
fat_get() {
  mcopy -i "${WORK}@@${BOOT_OFFSET}" "::${1}" "${2}"
}
fat_put() {
  mcopy -o -i "${WORK}@@${BOOT_OFFSET}" "${1}" "::${2}"
}
fat_get cmdline.txt "${BOOT_MNT}/cmdline.txt"
fat_get config.txt "${BOOT_MNT}/config.txt"
mount -o "loop,offset=$((ROOT_START * 512)),sizelimit=$((ROOT_SECS * 512))" "$WORK" "$ROOT_MNT"
[[ -d "${ROOT_MNT}/etc" ]] || die "root partition has no /etc"

python3 "${SDCARD}/lib/boot_config.py" cmdline \
  --kiosk "$CLOCK_KIOSK" \
  --rotation "$CLOCK_ROTATION" \
  --instance-id "$INSTANCE" \
  --wifi-country "$CLOCK_WIFI_COUNTRY" \
  --input "${BOOT_MNT}/cmdline.txt" > "${BOOT_MNT}/cmdline.txt.new"
mv "${BOOT_MNT}/cmdline.txt.new" "${BOOT_MNT}/cmdline.txt"
fat_put "${BOOT_MNT}/cmdline.txt" cmdline.txt

python3 "${SDCARD}/lib/boot_config.py" config-txt \
  --input "${BOOT_MNT}/config.txt" > "${BOOT_MNT}/config.txt.new"
mv "${BOOT_MNT}/config.txt.new" "${BOOT_MNT}/config.txt"
fat_put "${BOOT_MNT}/config.txt" config.txt

export CLOCK_HOSTNAME CLOCK_USER CLOCK_PASSWORD_HASH CLOCK_TIMEZONE CLOCK_LOCALE
export CLOCK_KEYBOARD CLOCK_SSH
user_args=(user-data)
if [[ -n "$CLOCK_SSH_AUTHORIZED_KEYS" ]]; then
  user_args+=(--authorized-keys "$CLOCK_SSH_AUTHORIZED_KEYS")
fi
python3 "${SDCARD}/lib/boot_config.py" "${user_args[@]}" > "${BOOT_MNT}/user-data"
fat_put "${BOOT_MNT}/user-data" user-data
python3 "${SDCARD}/lib/boot_config.py" meta-data --instance-id "$INSTANCE" > "${BOOT_MNT}/meta-data"
fat_put "${BOOT_MNT}/meta-data" meta-data
if [[ -n "$CLOCK_WIFI_SSID" ]]; then
  export CLOCK_WIFI_SSID CLOCK_WIFI_PSK CLOCK_WIFI_COUNTRY CLOCK_WIFI_HIDDEN
  python3 "${SDCARD}/lib/boot_config.py" network-config > "${BOOT_MNT}/network-config"
  fat_put "${BOOT_MNT}/network-config" network-config
fi
unset CLOCK_WIFI_PSK CLOCK_PASSWORD_HASH

install -d -m 0755 \
  "${ROOT_MNT}/opt/clock/bin" \
  "${ROOT_MNT}/opt/clock/files" \
  "${ROOT_MNT}/opt/clock/lib" \
  "${ROOT_MNT}/opt/clock/timewarp" \
  "${ROOT_MNT}/etc/systemd/system/multi-user.target.wants" \
  "${ROOT_MNT}/etc/sudoers.d" \
  "${ROOT_MNT}/usr/local/bin" \
  "${ROOT_MNT}/usr/share/plymouth/themes/clockface"

install -m 0755 "${SDCARD}/out/clockface" "${ROOT_MNT}/usr/local/bin/clockface"
install -m 0755 "${SDCARD}/provision.sh" "${ROOT_MNT}/opt/clock/provision.sh"
install -m 0755 "${SDCARD}/files/kiosk-start.sh" "${ROOT_MNT}/opt/clock/bin/kiosk-start.sh"
install -m 0755 "${SDCARD}/files/labwc-autostart.sh" "${ROOT_MNT}/opt/clock/bin/labwc-autostart.sh"
install -m 0755 "${SDCARD}/files/set-audio-sink.sh" "${ROOT_MNT}/opt/clock/bin/set-audio-sink.sh"
install -m 0644 "${SDCARD}/lib/boot_config.py" "${ROOT_MNT}/opt/clock/lib/boot_config.py"
install -m 0644 "${SDCARD}/files/clock-provision.service" \
  "${ROOT_MNT}/etc/systemd/system/clock-provision.service"
ln -sfn /etc/systemd/system/clock-provision.service \
  "${ROOT_MNT}/etc/systemd/system/multi-user.target.wants/clock-provision.service"
rm -rf "${ROOT_MNT}/usr/share/plymouth/themes/clockface"
install -d -m 0755 "${ROOT_MNT}/usr/share/plymouth/themes/clockface"
cp -a "${THEME}/." "${ROOT_MNT}/usr/share/plymouth/themes/clockface/"
# The preview GIF does not belong in the initramfs.
rm -f "${ROOT_MNT}/usr/share/plymouth/themes/clockface/preview.gif"

tar -C "$REPO" \
  --exclude='__pycache__' \
  --exclude='*.egg-info' \
  -cf - pyproject.toml README.md LICENSE src \
  | tar -C "${ROOT_MNT}/opt/clock/timewarp" -xf -

python3 "${SDCARD}/lib/boot_config.py" clockface-toml \
  --example "$EXAMPLE_TOML" \
  --audio "$AUDIO_FLAG" \
  --leds "$LEDS_FLAG" \
  "${TW_FLAG[@]}" \
  "${ROTATION_ARGS[@]}" \
  > "${ROOT_MNT}/opt/clock/clockface.toml"
python3 "${SDCARD}/lib/boot_config.py" cache-json > "${ROOT_MNT}/opt/clock/cache.json"
install -m 0644 "$EXAMPLE_TOML" "${ROOT_MNT}/opt/clock/clockface.example.toml"

cat > "${ROOT_MNT}/opt/clock/manifest.env" <<EOF
CLOCK_HOSTNAME=${CLOCK_HOSTNAME}
CLOCK_USER=${CLOCK_USER}
CLOCK_TIMEZONE=${CLOCK_TIMEZONE}
CLOCK_LOCALE=${CLOCK_LOCALE}
CLOCK_KEYBOARD=${CLOCK_KEYBOARD}
CLOCK_ROTATION=${CLOCK_ROTATION}
CLOCK_KIOSK=${CLOCK_KIOSK}
CLOCK_AUDIO_SINK=${CLOCK_AUDIO_SINK}
CLOCK_SSH=${CLOCK_SSH}
CLOCK_WIFI_COUNTRY=${CLOCK_WIFI_COUNTRY}
CLOCK_INSTANCE=${INSTANCE}
CLOCKFACE_GIT=${GIT_REV}
CLOCKFACE_AUDIO=${AUDIO_FLAG}
CLOCKFACE_LEDS=${LEDS_FLAG}
PI_OS_DATE=${PI_OS_DATE:-}
EOF
chmod 0644 "${ROOT_MNT}/opt/clock/manifest.env"

# The kiosk user may drop Plymouth and nothing else. General sudo stays passworded.
cat > "${ROOT_MNT}/etc/sudoers.d/clock-plymouth" <<EOF
${CLOCK_USER} ALL=(root) NOPASSWD: /usr/bin/plymouth quit
EOF
chmod 0440 "${ROOT_MNT}/etc/sudoers.d/clock-plymouth"
if command -v visudo >/dev/null 2>&1; then
  if ! visudo -cf "${ROOT_MNT}/etc/sudoers.d/clock-plymouth" >/dev/null; then
    die "generated sudoers file was rejected by visudo"
  fi
fi

sync
echo "customized image: ${WORK}"

disk_base() {
  local node="$1"
  local pk
  pk="$(lsblk -ndo PKNAME "$node" 2>/dev/null || true)"
  if [[ -n "$pk" ]]; then
    printf '/dev/%s\n' "$pk"
  else
    printf '%s\n' "$node"
  fi
}

write_device() {
  local dev="$1"
  local image="$2"
  local base root_src root_base mounts
  [[ -b "$dev" ]] || die "not a block device: ${dev}"
  base="$(disk_base "$dev")"
  root_src="$(findmnt -n -o SOURCE /)"
  root_base="$(disk_base "$root_src")"
  if [[ "$base" == "$root_base" ]]; then
    die "refusing to write ${dev}: ${base} holds / (${root_src})"
  fi
  if findmnt -n /boot >/dev/null 2>&1; then
    local boot_base
    boot_base="$(disk_base "$(findmnt -n -o SOURCE /boot)")"
    if [[ "$base" == "$boot_base" ]]; then
      die "refusing to write ${dev}: ${base} holds /boot"
    fi
  fi
  mounts="$(lsblk -nr -o MOUNTPOINT "$base" | sed '/^$/d' || true)"
  if [[ -n "$mounts" ]]; then
    die "refusing to write ${dev}: ${base} has a mounted filesystem"
  fi
  echo
  echo "============================================================"
  echo " THIS WILL ERASE ${dev}"
  echo " Disk ${base}"
  echo " Image ${image}"
  echo "============================================================"
  if [[ "${CLOCK_WRITE_CONFIRM:-}" == "$dev" ]]; then
    echo "CLOCK_WRITE_CONFIRM matches ${dev}"
  else
    if [[ ! -t 0 ]]; then
      die "refusing to write ${dev} without a terminal; set CLOCK_WRITE_CONFIRM=${dev}"
    fi
    local answer
    read -r -p "Type ${dev} to overwrite it: " answer
    [[ "$answer" == "$dev" ]] || die "not confirmed"
  fi
  dd if="$image" of="$dev" bs=4M conv=fsync status=progress
  sync
  echo "wrote ${dev}"
}

if [[ -n "$DEVICE" ]]; then
  write_device "$DEVICE" "$WORK"
fi
