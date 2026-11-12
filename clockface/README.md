# clockface

Roman-numeral grandfather clock for a Raspberry Pi. The dial is polished brass. Three apertures in the face show the weather, the moon, and the date. Two mural wheels turn behind the dial (the year, and the civil day). Under them are a calendar, a forecast, and sun and moon times, with a status line pinned to the bottom of the screen. Brass gears sit in the black field around the movement.

Time comes from `chrono::Local`, so the zoneinfo rules on the machine apply, including DST. The kiosk this replaces hardcoded EDT (UTC−4).

The painter is shared. The default build opens it in an eframe window (X11 or Wayland, glow). The `kms` feature on Linux opens it with [eboard](https://docs.rs/eboard/0.3.0/eboard/) instead, which talks to the display directly and does not need X11 or Wayland. `--screenshot` uses a software rasterizer in either build, so it does not need a display.

## What is on the dial

Lengths below are fractions of the face radius. Angles are clockwise from 12. Everything is positioned from the clock center, so changing the diameter or the center moves the complications with the face. Hands, numerals, ticks, and the brass ring are painted after the discs they cross.

- **Brass dial.** Warm metal, lit from the upper left, with a deeper antique falloff toward the rim. The bezel is a rounded brass ring (highlight on the upper left, shade on the lower right). Quarter markers are a brighter brass than the other hour markers. Minute ticks stay dark red. Numerals stay dark engraved brown. The hour and minute hands are black; the second hand is red.
- **Weather** (upper left, 9 o'clock to 12). Temperature, a short condition, and a small scene (sun, cloud, rain, snow, or night).
- **Moon** (upper right, 12 to 3). Northern hemisphere. Waxing lights the right side of the disc. The phase is the mean age since the new moon of 2000-01-06 18:14 UTC, using a synodic month of 29.530588853 days. It is a mean-phase clock, not a full lunar ephemeris.
- **Date** (lower half, 3 o'clock through 6 to 9). Weekday, month, and day, on a cream complication disc.
- **Year wheel** (lower left). A rounded window from about 6:30 to 8 o'clock. The mural turns once per tropical year. The middle of the window shows the current day of the year, blended between the solstices and equinoxes, so the season eases instead of jumping. Winter is snow, spring blossom, summer green, autumn color.
- **Sky wheel** (lower right). The mirror of that window, about 4 o'clock to 5:30. It turns once per local civil day. The middle of the window is "now": midnight is night, dawn and dusk are the color change, noon is day.
- **Calendar, forecast, sun and moon.** Three brass-edged panels under the wheels. Sun and moon times are computed on the machine (NOAA sunrise equation). They are shown in the system timezone, which should be the place's zone.
- **Status bar.** Hostname, local addresses, and whether `timedatectl` reports the clock in sync. A failed weather fetch is prefixed on that line. Addresses that cannot be read are said to be missing; none are invented.

## Build on Raspberry Pi OS 64-bit

Both builds use **glow** (OpenGL / GLES). wgpu is not enabled. eframe 0.37 needs a current stable Rust (1.95 or newer). Raspberry Pi OS packages are usually older:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

### Desktop (X11 or Wayland)

This is `cargo build --release` with no extra features. The window is fullscreen unless you pass `--windowed`.

```bash
sudo apt update
sudo apt install build-essential pkg-config \
  libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
  libxkbcommon-dev libgl1-mesa-dev libegl1-mesa-dev libwayland-dev
cd clockface
cargo build --release
```

### Direct display (`kms`, no X11 or Wayland)

```bash
cargo build --release --features kms
```

Build that on the Pi, or cross to `aarch64-unknown-linux-gnu`. The feature is Linux-only. On other operating systems it does not pull in eboard, so a desktop compile still succeeds.

eboard's default `auto` backend tries DRM/KMS + GBM + EGL first, then the software `linuxfb` framebuffer (`/dev/fbN`). That is the same idea as Qt's `eglfs` and `linuxfb`. The Vivante `viv-fb` backend is not enabled; it is for i.MX6, not the Pi.

The KMS path needs Mesa's GBM and GLES headers and libraries:

```bash
sudo apt install build-essential pkg-config libgbm-dev libegl1-mesa-dev libgles2-mesa-dev
```

Runtime packages are `libgbm1`, `libegl1`, and `libgles2`. To leave eframe and the X11 libraries out of the binary entirely, build with `--no-default-features --features kms`. `--features kms` on top of the default features still *starts* eboard; it just also links the desktop backend.

Nothing else may own the display. Stop the desktop, or boot to the console, and run from a virtual terminal or over SSH:

```bash
sudo systemctl isolate multi-user.target
./target/release/clockface
```

The user running it must be in the `video` and `render` groups and able to open `/dev/dri/card*` (and `/dev/fb0` if it falls back to the framebuffer).

On a Raspberry Pi 4 or 5 the GPU and the display controller can be separate DRM nodes. If the app does not find the screen, set `EBOARD_KMS_DEVICE` (often `/dev/dri/card1`) and, if you need to pick the connector, `EBOARD_KMS_CONNECTOR=HDMI-A-1`. Current Raspberry Pi OS already enables the KMS driver with `dtoverlay=vc4-kms-v3d`.

Force a backend with `EBOARD_BACKEND=kms` or `EBOARD_BACKEND=linuxfb`.

`--windowed` is ignored on this build. eboard is always fullscreen on the display. `--help` says so.

## Run

Fullscreen, diameter 600, centered horizontally, sitting against the top edge:

```bash
./target/release/clockface
```

Custom diameter and position (center is in pixels, origin at the top-left):

```bash
./target/release/clockface --diameter 800 --center-x 960 --center-y 480
```

The same values can live in a TOML file. See `clockface.example.toml`. Flags win over the file.

```bash
./target/release/clockface --config clockface.example.toml
```

Weather defaults to Indianapolis (39.7684, −86.1581), imperial units, refreshed about every 20 minutes from Open-Meteo. No API key. Override the place with `--city` (`indianapolis`, `chicago`, `new york`, `london`, `denver`), or with `--lat` and `--lon`. `--units imperial` or `--units metric`. `--lat` and `--lon` win over `--city`.

If the network is down, the last cached forecast is kept and the status line says the fetch failed. With no cache, the weather aperture says unavailable instead of going blank.

## Screenshot

Writes one PNG of the full kiosk at local 2026-01-15 10:08:37 and exits. No window and no display server. Weather and the calendar in that frame are a fixed January fixture, so the picture does not depend on the network. The status line is the real machine.

```bash
./target/release/clockface --screenshot clock.png --width 1920 --height 1080
```

## Autostart on the desktop

On Raspberry Pi OS with labwc, copy the system autostart and add the clock:

```bash
mkdir -p ~/.config/labwc
cp /etc/xdg/labwc/autostart ~/.config/labwc/autostart
echo "$HOME/TimeWarp/clockface/target/release/clockface" >> ~/.config/labwc/autostart
```

On X11, a `.desktop` file in `~/.config/autostart/` with `Exec=` pointing at the binary does the same job.

For the `kms` build, do not also start the desktop. A systemd user unit or a line in `~/.profile` on tty1 is enough once the machine boots to the console.
