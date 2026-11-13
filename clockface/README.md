# clockface

Roman-numeral grandfather clock for a Raspberry Pi. The dial is polished brass. The upper half of the face is a horizon: one sky for the time of day and the weather, with the date and the temperature in the two lower quarters. Two mural wheels turn behind the dial (the year, and the civil day). Under them are two boards, a calendar and a forecast with the sun and moon, and a status line pinned to the bottom of the screen. The two frames together are exactly as wide as the face.

Time comes from `chrono::Local`, so the zoneinfo rules on the machine apply, including DST. The kiosk this replaces hardcoded EDT (UTC−4).

The painter is shared. The default build opens it in an eframe window (X11 or Wayland, glow). The `kms` feature on Linux opens it with [eboard](https://docs.rs/eboard/0.3.0/eboard/) instead, which talks to the display directly and does not need X11 or Wayland. `--screenshot` uses a software rasterizer in either build, so it does not need a display.

## What is on the dial

Lengths below are fractions of the face radius. Angles are clockwise from 12. Everything is positioned from the clock center, so changing the diameter or the center moves the complications with the face. Hands, numerals, ticks, and the brass ring are painted after the discs they cross.

- **Brass dial.** A polished dish: tight specular, a softer reflection, turned rings, and a round bezel. Hour markers and the other 5-minute marks are raised gold pins. The 1-minute marks take the Cycles of Life color of the time that mark stands for on this half-day (below). Numerals are solid black. The hour and minute hands are black; the second hand is red. The window and `--screenshot` fill the same mesh, so the metal and the pin length match.
- **Horizon** (upper half, sitting on the 9–3 line). One sky, not two quarters. It is a layered SVG theme (`scenes/default/`), not a painted gradient. The trunk stays up; leaves, ground, sky, sun, stars, clouds, rain, snow, storm, and fog are groups the clock shows or hides. How to draw another theme is in [scenes/CREATING_A_THEME.md](scenes/CREATING_A_THEME.md). The moon is a solid shaded disc in that sky at 45° from 12, with the phase for the current time. The unlit side is a darkened surface, day and night, so the sky does not show through it. Northern hemisphere: waxing lights the right side. The phase is the mean age since the new moon of 2000-01-06 18:14 UTC, using a synodic month of 29.530588853 days. It is a mean-phase clock, not a full lunar ephemeris.
- **Date** (lower left, 6 to 9). Weekday, month, and day, on a cream complication disc.
- **Temperature** (lower right, 3 to 6). The reading and the condition name, on a matching cream disc.
- **Year wheel** (lower left). The window is the square corner outside the dial, between the 9 o'clock tangent and the 6 o'clock tangent. The mural is a disc on the shaft (`scenes/wheels/year.svg`) and turns once per tropical year. Winter, spring, summer, and autumn are drawn large in that outer band, so the corner shows a season rather than a wash of color. January is winter: snow, pines, a small cabin.
- **Day wheel** (lower right). The mirror corner, between 3 o'clock and 6. The disc is `scenes/wheels/day.svg` and turns once per local civil day. Night, dawn, day, and dusk are separate scenes. Mid-morning shows a high sun, blue sky, and trees.
- **Calendar and forecast.** Two untitled boards under the wheels, together exactly as wide as the face, on a near-black field with a visible grid of unlit lamps. Letters and numbers are an original 5×7 amber dot matrix (about 16 px tall on the 643-pixel face). The lines are left-aligned, so each TimeWarp mark leads the line and is drawn in that same amber, not as a color emoji. The left board is the calendar: a calendar or holiday mark, then the event, and the date or clock time indented under it. The right board leads with sunrise and sunset (⬆️ and ⬇️, the same marks TimeWarp uses), the season, the moon phase, and moonrise and moonset when that local day has them, then the five-day forecast. A wrapped line, on either board, continues two character cells in from the left. The centered date and temperature discs still lead with their marks. Sunrise and sunset use the NOAA equation. Moonrise and moonset use the same Schlyter scan as TimeWarp. Times are the machine's local civil clock.
- **Status bar.** The same dark lamp field, with lamps a little smaller than the boards. On the left: the hostname, the networked-computers mark (amber, like the frame marks), the IPv4 address of the interface that holds the default route, and a real fault when there is one. The crate version (`v` plus the package version) sits at the right edge. A long fault is cut with `...` so it does not run into the version. Loopback, docker, bridges, and tunnels are not listed, and neither is IPv6. A synced clock adds nothing. A missing address is an em dash. Nothing is invented.

## Cycles of Life colors

The 1-minute marks use H. Spencer Lewis's daily cycle from *Self-Mastery and Fate with the Cycles of Life* (AMORC, 1929), Chart E. The day is seven equal parts from midnight to midnight (about 3 h 25 min 43 s: 00:00, 03:25, 06:51, 10:17, 13:42, 17:08, 20:34). Lewis says to use local standard time and to ignore daylight saving. Chart E gives each period a letter, and the letter changes with the weekday. The dial shows the civil half the hour hand is in, midnight to noon or noon to midnight. Each of the 60 marks is a 12-minute slice of that half, colored by the slice's midpoint. Thursday 10:08, the `--screenshot` hour, is itself the third period and the letter G, but the morning ring also carries the periods before and after it.

Lewis prints no pigments. The color is the planet of that letter, in the order his cycle is built on (A Sun, B Venus, C Mercury, D Moon, E Saturn, F Jupiter, G Mars), using Max Heindel's planet colors from *The Message of the Stars* (1918), the table on p. 64:

| Letter | Planet | Color |
| --- | --- | --- |
| A | Sun | orange |
| B | Venus | yellow |
| C | Mercury | violet |
| D | Moon | green |
| E | Saturn | indigo |
| F | Jupiter | blue |
| G | Mars | red |

On the Thursday 10:08 screenshot the morning ring runs Saturn indigo from 12 to just past 3, Jupiter blue until just past 6, Mars red until 10, and Sun orange from there back to 12. The 5-minute marks stay gold.

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

With no `--diameter`, the face is the largest one that keeps the whole column on screen: the dial, the squared windows, the two frames, and a single-line status bar. Width and height both limit it. On a 1920×1080 frame that is a 643 pixel face. A window defaults to 1200×800 so it fits a 1080p desktop with a panel, a title bar, and a dock; there the face is 468 pixels.

```bash
./target/release/clockface
./target/release/clockface --windowed
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

If the network is down, the last cached forecast is kept and the status line appends the fetch error. With no cache, the weather scene says unavailable instead of going blank.

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
