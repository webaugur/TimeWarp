#!/usr/bin/env python3
"""Boot-partition and clockface config text for the clock SD card.

build-image.sh calls this. The functions stay free of device I/O so tests can
check cmdline tokens, the cloud-init user, and the clockface TOML without
mounting an image.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from pathlib import Path

PANEL_MODE = "1280x1024@60"
HDMI_CONNECTOR = "HDMI-A-1"

CMDLINE_FLAGS = (
    "quiet",
    "splash",
    "plymouth.ignore-serial-consoles",
    "logo.nologo",
    "vt.global_cursor_default=0",
    "consoleblank=0",
)

VIDEO_RE = re.compile(r"^video=HDMI-A-1:1280x1024@60(?:,rotate=\d+)?$")
NOCLOUD_RE = re.compile(r"^ds=nocloud(?:;i=.*)?$")
REGDOM_RE = re.compile(r"^cfg80211\.ieee80211_regdom=.*$")


def viewer_size(rotation: int) -> tuple[int, int]:
    """Framebuffer size after the kernel has applied `rotate=`."""
    check_rotation(rotation)
    if rotation % 180 == 0:
        return (1280, 1024)
    return (1024, 1280)


def output_size(rotation: int, kiosk: str) -> tuple[int, int]:
    """Pixel size of one Plymouth frame on the unrotated panel or the rotated fb."""
    check_rotation(rotation)
    check_kiosk(kiosk)
    if kiosk == "kms":
        return viewer_size(rotation)
    return (1280, 1024)


def pre_rotate_cw(rotation: int, kiosk: str) -> int:
    """Degrees clockwise to turn an upright frame into panel pixels.

    The kms path puts `rotate=` on the kernel command line, so Plymouth already
    draws in the viewer's orientation. The labwc path leaves the panel in its
    native landscape scanout and rotates in the compositor, which starts after
    Plymouth, so the splash itself has to be pre-rotated.
    """
    check_rotation(rotation)
    check_kiosk(kiosk)
    if kiosk == "kms":
        return 0
    return rotation


def check_rotation(rotation: int) -> None:
    if rotation not in (0, 90, 180, 270):
        raise ValueError(f"rotation must be 0, 90, 180, or 270 (got {rotation})")


def check_kiosk(kiosk: str) -> None:
    if kiosk not in ("kms", "labwc"):
        raise ValueError(f"kiosk must be kms or labwc (got {kiosk!r})")


def kernel_rotate(rotation: int) -> int:
    """DRM/Wayland degrees for a clockwise `rotation`.

    CLOCK_ROTATION is how far the picture turns clockwise on the panel. The
    kernel's `video=...,rotate=` value (DRM_MODE_ROTATE_*) and wlr-randr's
    `--transform` (wl_output.transform) both count counter-clockwise, so 90
    clockwise is 270 there. Writing rotate=90 showed the clock upside down
    on the portrait Dell.
    """
    check_rotation(rotation)
    return (360 - rotation) % 360


def video_token(rotation: int, kiosk: str) -> str:
    check_rotation(rotation)
    check_kiosk(kiosk)
    if kiosk == "labwc":
        return f"video={HDMI_CONNECTOR}:{PANEL_MODE}"
    return f"video={HDMI_CONNECTOR}:{PANEL_MODE},rotate={kernel_rotate(rotation)}"


def merge_cmdline(
    existing: str,
    *,
    kiosk: str,
    rotation: int,
    instance_id: str,
    wifi_country: str,
) -> str:
    """Return one cmdline line with splash, NoCloud, and the Dell panel mode."""
    check_kiosk(kiosk)
    check_rotation(rotation)
    if not instance_id or any(ch.isspace() for ch in instance_id):
        raise ValueError("instance id must be one token")
    tokens = [tok for tok in existing.replace("\n", " ").split(" ") if tok]
    kept: list[str] = []
    for tok in tokens:
        if VIDEO_RE.match(tok) or NOCLOUD_RE.match(tok) or REGDOM_RE.match(tok):
            continue
        kept.append(tok)
    for flag in CMDLINE_FLAGS:
        if flag not in kept:
            kept.append(flag)
    country = wifi_country.strip().upper()
    if country:
        if not re.fullmatch(r"[A-Z]{2}", country):
            raise ValueError(f"Wi-Fi country must be two letters (got {wifi_country!r})")
        kept.append(f"cfg80211.ieee80211_regdom={country}")
    kept.append(f"ds=nocloud;i={instance_id}")
    kept.append(video_token(rotation, kiosk))
    return " ".join(kept) + "\n"


def merge_config_txt(existing: str) -> str:
    """Enable SPI, hide the firmware rainbow, and drop legacy display_rotate."""
    lines = existing.splitlines()
    out: list[str] = []
    for line in lines:
        stripped = line.strip()
        if stripped.startswith(("display_rotate=", "#display_rotate=")):
            out.append("# display_rotate is ignored by vc4-kms-v3d; rotation is on cmdline or wlr-randr")
            continue
        out.append(line)
    text = "\n".join(out)
    if text and not text.endswith("\n"):
        text += "\n"
    text = _set_config_key(text, "disable_splash", "1")
    text = _set_config_key(text, "dtparam=spi", "on")
    return text


def _set_config_key(text: str, key: str, value: str) -> str:
    """Set `key=value`, uncommenting a stock `#key=value` line when present."""
    pattern = re.compile(rf"^[ \t]*#?\s*{re.escape(key)}=.*$", re.MULTILINE)
    replacement = f"{key}={value}"
    if pattern.search(text):
        return pattern.sub(replacement, text, count=1)
    if text and not text.endswith("\n"):
        text += "\n"
    return text + replacement + "\n"


def yaml_quote(value: str) -> str:
    escaped = value.replace("\\", "\\\\").replace('"', '\\"')
    return f'"{escaped}"'


def render_meta_data(instance_id: str) -> str:
    if not instance_id or any(ch.isspace() for ch in instance_id):
        raise ValueError("instance id must be one token")
    return f"instance-id: {instance_id}\n"


def render_user_data(
    *,
    hostname: str,
    user: str,
    password_hash: str,
    timezone: str,
    locale: str,
    keyboard: str,
    ssh: bool,
    authorized_keys: list[str],
) -> str:
    """cloud-init user-data in the shape Raspberry Pi Imager writes for Trixie.

    `sudo: null` is explicit. The image's default user is merged over with
    passwordless sudo, and omitting the key keeps that. A clock on the shop
    network should ask for the password.
    """
    if not password_hash.startswith("$"):
        raise ValueError("password hash must be a crypt hash ($6$...)")
    lines = [
        "#cloud-config",
        "manage_resolv_conf: false",
        "",
        f"hostname: {yaml_quote(hostname)}",
        "manage_etc_hosts: true",
        "packages:",
        "- avahi-daemon",
        "",
        "apt:",
        "  preserve_sources_list: true",
        "  conf: |",
        "    Acquire {",
        '      Check-Date "false";',
        "    };",
        "",
        f"timezone: {yaml_quote(timezone)}",
        f"locale: {yaml_quote(locale)}",
        "keyboard:",
        "  model: pc105",
        f"  layout: {yaml_quote(keyboard)}",
        "user:",
        f"  name: {yaml_quote(user)}",
        "  shell: /bin/bash",
        "  lock_passwd: false",
        f"  passwd: {yaml_quote(password_hash)}",
        "  sudo: null",
    ]
    keys = [key.strip() for key in authorized_keys if key.strip()]
    if keys:
        lines.append("  ssh_authorized_keys:")
        for key in keys:
            lines.append(f"    - {yaml_quote(key)}")
    lines.append("")
    if ssh:
        lines.append("ssh_pwauth: true")
        lines.append("runcmd:")
        lines.append("  - [ systemctl, enable, --now, ssh ]")
    lines.append("")
    return "\n".join(lines)


def render_network_config(
    *,
    ssid: str,
    psk: str,
    country: str,
    hidden: bool,
) -> str:
    """Netplan network-config. Empty when Wi-Fi is not configured.

    Ethernet stays on DHCP either way: the published image already brings up
    eth0, and this file replaces that config when it is present.
    """
    if not ssid:
        return ""
    if not psk:
        raise ValueError("Wi-Fi SSID is set but the PSK is empty")
    country = country.strip().upper()
    if not re.fullmatch(r"[A-Z]{2}", country):
        raise ValueError("Wi-Fi country must be two letters when an SSID is set")
    lines = [
        "network:",
        "  version: 2",
        "  ethernets:",
        "    eth0:",
        "      dhcp4: true",
        "      dhcp6: true",
        "      optional: true",
        "  wifis:",
        "    wlan0:",
        "      dhcp4: true",
        f"      regulatory-domain: {yaml_quote(country)}",
        "      access-points:",
        f"        {yaml_quote(ssid)}:",
    ]
    if hidden:
        lines.append("          hidden: true")
    lines.append(f"          password: {yaml_quote(psk)}")
    lines.append("      optional: true")
    lines.append("")
    return "\n".join(lines)


def supports_audio(clockface_dir: Path) -> bool:
    cargo = (clockface_dir / "Cargo.toml").read_text(encoding="utf-8")
    if re.search(r"\brodio\b", cargo):
        return True
    settings = clockface_dir / "src" / "settings.rs"
    return settings.is_file() and "mute_path" in settings.read_text(encoding="utf-8")


def supports_timewarp_bin(clockface_dir: Path) -> bool:
    settings = clockface_dir / "src" / "settings.rs"
    if not settings.is_file():
        return False
    return "timewarp_bin" in settings.read_text(encoding="utf-8")


def supports_leds(clockface_dir: Path) -> bool:
    settings = clockface_dir / "src" / "settings.rs"
    return (clockface_dir / "src" / "leds.rs").is_file() and (
        settings.is_file() and "LedsFile" in settings.read_text(encoding="utf-8")
    )


def render_clockface_toml(
    example: str,
    *,
    audio: bool,
    timewarp_bin: str | None,
    leds: str | None = None,
    rotation: int | None = None,
) -> str:
    """Start from clockface.example.toml and pin the shop clock's settings.

    The parser uses deny_unknown_fields on every table, so `[audio]` and
    `[leds]` are written only for a revision that has them (`audio` and
    `leds` are decided from the source by the caller). `leds` is the
    backend name: "spi" for the kms build (leds-spi feature), "none" for a
    build that cannot drive the strip, or None to leave the table out.
    `rotation` is clockwise degrees (0, 90, 180, 270). None leaves `[display]`
    out, for a checkout whose parser does not accept the table.
    """
    text = example
    if not text.endswith("\n"):
        text += "\n"
    text = _drop_commented_table(text, "audio")
    text = _drop_commented_table(text, "leds")
    text = _replace_top_level(text, "units", '"imperial"')
    if timewarp_bin:
        text = _replace_or_insert_timewarp_bin(text, timewarp_bin)
    if audio:
        text = _upsert_audio(text)
    if leds:
        text = _upsert_leds(text, leds)
    if rotation is not None:
        check_rotation(rotation)
        text = _upsert_display(text, rotation)
    return text


def _drop_commented_table(text: str, name: str) -> str:
    """Remove a fully commented `[name]` block from the installed file.

    The example documents `[audio]` with `#` lines. Those are comments, but the
    shop file for a build that does not accept the table must not contain the
    header, and an audio build must not keep the example's `gpio_path` line
    beside the real table.
    """
    header = re.compile(rf"^#\s*\[{re.escape(name)}\]\s*$")
    lines = text.splitlines(keepends=True)
    out: list[str] = []
    skipping = False
    for line in lines:
        bare = line.rstrip("\n")
        if header.match(bare):
            skipping = True
            continue
        if skipping:
            if bare.startswith("#"):
                continue
            skipping = False
            if bare.strip() == "":
                continue
        out.append(line)
    return "".join(out)


def _replace_top_level(text: str, key: str, toml_value: str) -> str:
    pattern = re.compile(rf"^{re.escape(key)}\s*=.*$", re.MULTILINE)
    replacement = f"{key} = {toml_value}"
    if pattern.search(text):
        return pattern.sub(replacement, text, count=1)
    return f"{replacement}\n{text}"


def _replace_or_insert_timewarp_bin(text: str, path: str) -> str:
    """Set the top-level `timewarp_bin`.

    The example documents the key below `[place]`, so substituting the
    commented line in place would make it `place.timewarp_bin`, which the
    parser rejects. The key goes before the first table header instead.
    """
    line = f'timewarp_bin = "{path}"\n'
    text = re.sub(r"(?m)^#?\s*timewarp_bin\s*=.*\n", "", text)
    header = re.search(r"(?m)^\[", text)
    if header is None:
        return text + line
    return text[: header.start()] + line + "\n" + text[header.start() :]


def _upsert_audio(text: str) -> str:
    block = (
        "\n"
        "# Ticking and chimes (sounds are compiled into the binary).\n"
        "# /tmp/mute containing 1 silences the tick.\n"
        "# gpio_path stays unset: Trixie deprecated sysfs GPIO, so a GPIO5\n"
        "# mute switch needs libgpiod and is not wired by these scripts.\n"
        "[audio]\n"
        "enabled = true\n"
        'tick = "15db"\n'
        'mute_path = "/tmp/mute"\n'
    )
    if re.search(r"^\[audio\]\s*$", text, re.MULTILINE):
        text = re.sub(r'(?m)^tick\s*=.*$', 'tick = "15db"', text, count=1)
        text = re.sub(
            r'(?m)^mute_path\s*=.*$', 'mute_path = "/tmp/mute"', text, count=1
        )
        return text
    return text + block


def _upsert_leds(text: str, backend: str) -> str:
    """The 20-pixel strip on SPI0 MOSI, configured the way the old clock was."""
    if backend not in ("spi", "none"):
        raise ValueError(f"leds backend must be spi or none (got {backend!r})")
    text = re.sub(r"(?ms)^\[leds\]\s*\n(?:[^\[\n][^\n]*\n|\n)*", "", text)
    if not text.endswith("\n"):
        text += "\n"
    return text + (
        "\n"
        "# NeoPixel strip on /dev/spidev0.0 (GPIO10 MOSI). gbr and 400 kHz match\n"
        "# the old Pi scripts. /tmp/mute containing 1 also turns the strip off.\n"
        "[leds]\n"
        f'backend = "{backend}"\n'
        'device = "/dev/spidev0.0"\n'
        "count = 20\n"
        "brightness = 192\n"
        'order = "gbr"\n'
        "freq_hz = 400000\n"
        "honor_mute = true\n"
        'mute_path = "/tmp/mute"\n'
    )


def _upsert_display(text: str, rotation: int) -> str:
    text = _drop_commented_table(text, "display")
    text = re.sub(r"(?ms)^\[display\]\s*\n(?:[^\[\n][^\n]*\n|\n)*", "", text)
    if not text.endswith("\n"):
        text += "\n"
    return text + (
        "\n"
        "# Clockwise degrees. The kms build lays the face out at the swapped\n"
        "# size and eboard blits it onto the panel. vc4 cannot rotate 90/270.\n"
        "[display]\n"
        f"rotation = {rotation}\n"
    )


def render_cache_json() -> str:
    return json.dumps({"city": "Indianapolis"}, indent=2) + "\n"


def mbr_partitions(image: Path) -> list[tuple[int, int, int]]:
    """MBR slots as (type, start_sector, sector_count).

    Loop devices are often built with max_part=0, so partition nodes never
    appear. Callers mount with an offset instead.
    """
    with image.open("rb") as handle:
        mbr = handle.read(512)
    if len(mbr) < 512 or mbr[510:512] != b"\x55\xaa":
        raise ValueError(f"{image} has no MBR signature")
    parts: list[tuple[int, int, int]] = []
    for index in range(4):
        entry = mbr[446 + 16 * index : 446 + 16 * index + 16]
        if entry == bytes(16):
            continue
        part_type = entry[4]
        start = int.from_bytes(entry[8:12], "little")
        count = int.from_bytes(entry[12:16], "little")
        if count == 0:
            continue
        parts.append((part_type, start, count))
    if len(parts) < 2:
        raise ValueError(f"{image} needs a boot partition and a root partition")
    return parts


def _env(name: str, default: str = "") -> str:
    return os.environ.get(name, default)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="cmd", required=True)

    meta = sub.add_parser("meta-data")
    meta.add_argument("--instance-id", required=True)

    user = sub.add_parser("user-data")
    user.add_argument("--authorized-keys", default="", help="path to a keys file")

    sub.add_parser("network-config")

    cmd = sub.add_parser("cmdline")
    cmd.add_argument("--kiosk", required=True)
    cmd.add_argument("--rotation", type=int, required=True)
    cmd.add_argument("--instance-id", required=True)
    cmd.add_argument("--wifi-country", default="")
    cmd.add_argument("--input", required=True, help="existing cmdline.txt")

    krot = sub.add_parser("kernel-rotate")
    krot.add_argument("--rotation", type=int, required=True)

    cfg = sub.add_parser("config-txt")
    cfg.add_argument("--input", required=True)

    toml_cmd = sub.add_parser("clockface-toml")
    toml_cmd.add_argument("--example", required=True)
    toml_cmd.add_argument("--clockface-dir", default="")
    toml_cmd.add_argument("--timewarp-bin", default="/usr/local/bin/timewarp")
    toml_cmd.add_argument("--audio", choices=("auto", "yes", "no"), default="auto")
    toml_cmd.add_argument(
        "--leds",
        choices=("auto", "spi", "none", "omit"),
        default="auto",
        help="auto: spi when the source has [leds], else omit the table",
    )
    toml_cmd.add_argument(
        "--no-timewarp-bin",
        action="store_true",
        help="omit timewarp_bin (this revision rejects the key)",
    )
    toml_cmd.add_argument(
        "--rotation",
        type=int,
        default=None,
        help="clockwise [display] rotation; omit the table when absent",
    )

    sub.add_parser("cache-json")

    parts = sub.add_parser("partitions")
    parts.add_argument("--image", required=True)

    args = parser.parse_args(argv)
    if args.cmd == "meta-data":
        sys.stdout.write(render_meta_data(args.instance_id))
    elif args.cmd == "user-data":
        keys: list[str] = []
        if args.authorized_keys:
            keys = Path(args.authorized_keys).read_text(encoding="utf-8").splitlines()
        sys.stdout.write(
            render_user_data(
                hostname=_env("CLOCK_HOSTNAME", "clock"),
                user=_env("CLOCK_USER", "clock"),
                password_hash=_env("CLOCK_PASSWORD_HASH"),
                timezone=_env("CLOCK_TIMEZONE", "America/Indiana/Indianapolis"),
                locale=_env("CLOCK_LOCALE", "en_US.UTF-8"),
                keyboard=_env("CLOCK_KEYBOARD", "us"),
                ssh=_env("CLOCK_SSH", "1") != "0",
                authorized_keys=keys,
            )
        )
    elif args.cmd == "network-config":
        sys.stdout.write(
            render_network_config(
                ssid=_env("CLOCK_WIFI_SSID"),
                psk=_env("CLOCK_WIFI_PSK"),
                country=_env("CLOCK_WIFI_COUNTRY", "US"),
                hidden=_env("CLOCK_WIFI_HIDDEN", "0") == "1",
            )
        )
    elif args.cmd == "cmdline":
        existing = Path(args.input).read_text(encoding="utf-8")
        sys.stdout.write(
            merge_cmdline(
                existing,
                kiosk=args.kiosk,
                rotation=args.rotation,
                instance_id=args.instance_id,
                wifi_country=args.wifi_country,
            )
        )
    elif args.cmd == "kernel-rotate":
        sys.stdout.write(f"{kernel_rotate(args.rotation)}\n")
    elif args.cmd == "config-txt":
        existing = Path(args.input).read_text(encoding="utf-8")
        sys.stdout.write(merge_config_txt(existing))
    elif args.cmd == "clockface-toml":
        clockface = Path(args.clockface_dir) if args.clockface_dir else None
        example = Path(args.example).read_text(encoding="utf-8")
        if args.audio == "auto":
            if clockface is None:
                raise ValueError("--audio auto needs --clockface-dir")
            audio = supports_audio(clockface)
        else:
            audio = args.audio == "yes"
        if args.no_timewarp_bin or (
            clockface is not None and not supports_timewarp_bin(clockface)
        ):
            bin_path = None
        else:
            bin_path = args.timewarp_bin
        if args.leds == "auto":
            if clockface is None:
                raise ValueError("--leds auto needs --clockface-dir")
            leds = "spi" if supports_leds(clockface) else None
        elif args.leds == "omit":
            leds = None
        else:
            leds = args.leds
        sys.stdout.write(
            render_clockface_toml(
                example,
                audio=audio,
                timewarp_bin=bin_path,
                leds=leds,
                rotation=args.rotation,
            )
        )
    elif args.cmd == "cache-json":
        sys.stdout.write(render_cache_json())
    elif args.cmd == "partitions":
        for part_type, start, count in mbr_partitions(Path(args.image)):
            sys.stdout.write(f"{part_type:x} {start} {count}\n")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except ValueError as err:
        print(f"boot_config: {err}", file=sys.stderr)
        raise SystemExit(2) from err
