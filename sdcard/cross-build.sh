#!/usr/bin/env bash
# Cross-build clockface for the Pi 3 (aarch64). Do not compile Rust on the Pi:
# 1 GB is not enough for the egui release build.
#
# Default method: a Debian trixie amd64 container with arm64 multiarch -dev
# packages (see cross/Dockerfile). rustc runs natively and links the Trixie
# sonames the Pi boots (libasound.so.2, libgbm.so.1, libEGL, libGLESv2).
# qemu-user compilation is not used; it is slow and easy to OOM.
#
# libasound2-dev is always installed, including for a checkout that does not
# link ALSA yet (chimes/ticking are PR #43). The script builds whatever
# clockface revision is checked out, or CLOCKFACE_REF when that is set.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SDCARD="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MODE="kms"
DRY_RUN=0
RUNTIME="${CLOCK_CROSS_RUNTIME:-}"
IMAGE="${CLOCK_CROSS_IMAGE:-timewarp-clockface-cross:trixie}"
OUT="${SDCARD}/out"

usage() {
  cat <<'EOF'
Usage: cross-build.sh [--labwc] [--kms] [--dry-run] [--runtime docker|podman]

  --kms       DRM/KMS binary (default): --no-default-features --features kms
  --labwc     desktop/winit binary for the labwc fallback
  --dry-run   print the cargo line and the image, then exit
  --runtime   container engine (default: podman, else docker)

Environment:
  CLOCKFACE_REF   branch, tag, or SHA to build instead of the working tree
  CLOCK_CROSS_RUNTIME, CLOCK_CROSS_IMAGE
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --labwc) MODE="labwc"; shift ;;
    --kms) MODE="kms"; shift ;;
    --dry-run) DRY_RUN=1; shift ;;
    --runtime)
      RUNTIME="$2"
      shift 2
      ;;
    -h | --help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage; exit 2 ;;
  esac
done

if [[ -z "$RUNTIME" ]]; then
  if command -v podman >/dev/null 2>&1; then
    RUNTIME="podman"
  elif command -v docker >/dev/null 2>&1; then
    RUNTIME="docker"
  else
    RUNTIME=""
  fi
fi

case "$MODE" in
  kms)
    CARGO_ARGS=(build --release --locked --target aarch64-unknown-linux-gnu --no-default-features --features kms)
    ;;
  labwc)
    CARGO_ARGS=(build --release --locked --target aarch64-unknown-linux-gnu)
    ;;
  *)
    echo "bad mode ${MODE}" >&2
    exit 2
    ;;
esac

SRC="$ROOT"
CLEAN_WORKTREE=""
if [[ -n "${CLOCKFACE_REF:-}" ]]; then
  echo "clockface ref: ${CLOCKFACE_REF}"
  if [[ "$DRY_RUN" -eq 0 ]]; then
    CLEAN_WORKTREE="$(mktemp -d)"
    rmdir "$CLEAN_WORKTREE"
    ref=""
    if git -C "$ROOT" fetch --depth 1 origin "$CLOCKFACE_REF"; then
      ref="FETCH_HEAD"
    elif git -C "$ROOT" rev-parse --verify "${CLOCKFACE_REF}^{commit}" >/dev/null 2>&1; then
      ref="$CLOCKFACE_REF"
    else
      echo "cross-build: cannot resolve CLOCKFACE_REF=${CLOCKFACE_REF}" >&2
      exit 1
    fi
    git -C "$ROOT" worktree add --detach "$CLEAN_WORKTREE" "$ref"
    SRC="$CLEAN_WORKTREE"
  fi
fi

cleanup() {
  if [[ -n "$CLEAN_WORKTREE" && -d "$CLEAN_WORKTREE" ]]; then
    git -C "$ROOT" worktree remove --force "$CLEAN_WORKTREE" || rm -rf "$CLEAN_WORKTREE"
  fi
}
trap cleanup EXIT

audio="no"
timewarp_bin="no"
if [[ -f "${SRC}/clockface/Cargo.toml" ]] && grep -q 'rodio' "${SRC}/clockface/Cargo.toml"; then
  audio="yes"
fi
if [[ -f "${SRC}/clockface/src/settings.rs" ]] && grep -q 'mute_path' "${SRC}/clockface/src/settings.rs"; then
  audio="yes"
fi
if [[ -f "${SRC}/clockface/src/settings.rs" ]] && grep -q 'timewarp_bin' "${SRC}/clockface/src/settings.rs"; then
  timewarp_bin="yes"
fi

echo "mode: ${MODE}"
echo "audio libs: always (crate audio=${audio})"
echo "cargo: cargo ${CARGO_ARGS[*]}"
echo "image: ${IMAGE}"

if [[ "$DRY_RUN" -eq 1 ]]; then
  if [[ -z "$RUNTIME" ]]; then
    echo "container runtime: none found (install podman or docker before a real build)"
  else
    echo "container runtime: ${RUNTIME}"
  fi
  exit 0
fi

if [[ -z "$RUNTIME" ]]; then
  echo "cross-build: need podman or docker. Compiling on the Pi is intentionally unsupported." >&2
  exit 1
fi

mkdir -p "$OUT"
"$RUNTIME" build -t "$IMAGE" -f "${SDCARD}/cross/Dockerfile" "${SDCARD}/cross"

# The container writes the binary to a directory owned by the caller.
mkdir -p "${OUT}/target"
# CARGO_ARGS is expanded by the container shell, not this one.
# shellcheck disable=SC2016
"$RUNTIME" run --rm \
  -v "${SRC}:/src:ro" \
  -v "${OUT}/target:/out" \
  -e CARGO_TARGET_DIR=/out \
  -e PKG_CONFIG_ALLOW_CROSS=1 \
  -e PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig \
  -e CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
  -e CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
  -e "CARGO_ARGS=${CARGO_ARGS[*]}" \
  "$IMAGE" \
  bash -lc 'cd /src/clockface && cargo $CARGO_ARGS && aarch64-linux-gnu-strip --strip-unneeded /out/aarch64-unknown-linux-gnu/release/clockface'

install -m 0755 "${OUT}/target/aarch64-unknown-linux-gnu/release/clockface" "${OUT}/clockface"
git_rev="unknown"
if git -C "$SRC" rev-parse HEAD >/dev/null 2>&1; then
  git_rev="$(git -C "$SRC" rev-parse HEAD)"
fi
cp -f "${SRC}/clockface/clockface.example.toml" "${OUT}/clockface.example.toml"
cat > "${OUT}/clockface.features" <<EOF
target=aarch64-unknown-linux-gnu
kiosk=${MODE}
audio=${audio}
timewarp_bin=${timewarp_bin}
git=${git_rev}
EOF
echo "wrote ${OUT}/clockface"
echo "features ${OUT}/clockface.features"
