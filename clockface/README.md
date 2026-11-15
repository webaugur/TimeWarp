# clockface

Roman-numeral grandfather clock for a Raspberry Pi. The dial is a cream-to-amber ring with a hard edge. The lower half of the face is a horizon: one landscape for the time of day, the season, and the weather, with the temperature written over it. The two upper quarters hold a sun disc and a moon disc. Two mural wheels turn behind the dial (the year, and the civil day). Under them are two boards, a calendar and a forecast with the sun and moon, and a status line pinned to the bottom of the screen. The two frames together are exactly as wide as the face.

Time comes from `chrono::Local`, so the zoneinfo rules on the machine apply, including DST. The kiosk this replaces hardcoded EDT (UTC−4).

The painter is shared. The default build opens it in an eframe window (X11 or Wayland, glow). The `kms` feature on Linux opens it with [eboard](https://docs.rs/eboard/0.3.0/eboard/) instead, which talks to the display directly and does not need X11 or Wayland. `--screenshot` uses a software rasterizer in either build, so it does not need a display.

## What is on the dial

Lengths below are fractions of the face radius. Angles are clockwise from 12. Everything is positioned from the clock center, so changing the diameter or the center moves the complications with the face. Hands, numerals, ticks, and the dial ring are painted after the discs they cross.

- **Dial.** A radial wash from pale cream (`#ffffb2`) at the center to warm amber (`#ffcc7f`) where the ring ends. The wash runs to the circle and stops. There is no bezel and no outline stroke. Hour markers and the other 5-minute marks are raised gold pins with a thin darker-gold edge so they read on the cream. The 1-minute marks take the Cycles of Life color of the time that mark stands for on this half-day (below). All twelve numerals, I through XII, sit on the brass band in a radial ring (VI upside down, IIII rather than IV), filled near-black and a fifth smaller than the source ring, still centered on that radius. The hour and minute hands are ornate polished gold, fill and shading only, with openwork that stays clear. The second hand is red. The gold hub stays on top. Hour, minute, wind, and second hands share that second-hand shadow. A bright green wind hand (`#39FF14`), about as wide as the minute hand, is painted over the gold hands and under the red second hand, and carries station-model barbs. The window and `--screenshot` fill the same mesh. The hand artwork and the numeral ring are each adapted from a CC0 vector clock illustration on Pixabay. The dial gradient is from the same illustration as the numerals.
- **Horizon** (lower half, sitting on the 9–3 line, ground toward 6). One landscape, not two quarters. It is a layered SVG theme (`scenes/default/`), drawn at twice the aperture's pixel size and cached until the weather band, the time-of-day band, or the size changes. The sky is a photograph: clear, partly cloudy, overcast, or night. Dawn and dusk are translucent tints on the daytime plate. Rain, snow, fog, and lightning stay vector layers on top of the ground and the tree. The horizon is the season's ground, with a low roll of the same soil at each end and the middle left open so the clouds reach the line. A thin band along the 9–3 line fades the top of that sky to black. There is no sun or moon in the landscape. How to draw another theme is in [scenes/CREATING_A_THEME.md](scenes/CREATING_A_THEME.md). The temperature and the condition are painted over the ground, in light type with a dark shadow. The plates are CC0, cropped to sky, and credited below.
- **Sun** (upper left, 9 to 12). Solid black, then the current GOES SUVI 304 Å disc from [NOAA SWPC](https://services.swpc.noaa.gov/images/animations/suvi/primary/304/latest.png). The live clock fetches that PNG about every 15 minutes and stores it in `~/.cache/clockface/suvi-304.png`. It is a US government work and is not copied into this repo. Offline, the last file is shown; with no file, a drawn sun is used.
- **Moon** (upper right, 12 to 3). Solid black, then a photograph of the full moon shaded to the current phase. The unlit side is the same photograph darkened (earthshine), not a cut-out and not a cartoon. The picture is NASA Scientific Visualization Studio item [5001](https://svs.gsfc.nasa.gov/5001/), a public-domain mosaic of 1,231 Lunar Reconnaissance Orbiter Narrow Angle Camera frames from the summer of 2018 (Ernie Wright, USRA; NASA/GSFC/LROC). A 512-pixel copy is `images/lro-full-moon.png`. Northern hemisphere: waxing lights the right side. The phase is the mean age since the new moon of 2000-01-06 18:14 UTC, using a synodic month of 29.530588853 days.

## Sky photographs

Each plate is a CC0 1.0 dedication ([Creative Commons Zero](https://creativecommons.org/publicdomain/zero/1.0/)), cropped to sky so the landscape does not contain a sun, a moon, or the ground. Copies are `scenes/default/skies/`.

| File | Used for | Source |
| --- | --- | --- |
| `clear.jpg` | Clear day, and the base under the dawn and dusk tints when the cover is low | [Sky clouds.JPG](https://commons.wikimedia.org/wiki/File:Sky_clouds.JPG), 12345danNL, own work, 14 March 2013 |
| `partly.jpg` | Partly cloudy | [Clouds-17.jpg](https://commons.wikimedia.org/wiki/File:Clouds-17.jpg), ViajeroExtraviado, own work, 9 April 2021 |
| `over.jpg` | Overcast, rain, snow, storm, and fog | [Overcast or Cloudy Sky 01.jpg](https://commons.wikimedia.org/wiki/File:Overcast_or_Cloudy_Sky_01.jpg), PagesAndPathsPH, own work, 10 July 2023 |
| `night.jpg` | Night | [Cévennes France night sky with stars 01.jpg](https://commons.wikimedia.org/wiki/File:C%C3%A9vennes_France_night_sky_with_stars_01.jpg), Arnaud Vergnet, own work, 4 July 2020 |
- **Year wheel** (lower left). The window is the square corner outside the dial, between the 9 o'clock tangent and the 6 o'clock tangent. The mural is a disc on the shaft (`scenes/wheels/year.svg`) and turns once per tropical year. Winter, spring, summer, and autumn are drawn large in that outer band, so the corner shows a season rather than a wash of color. January is winter: snow, pines, a small cabin.
- **Day wheel** (lower right). The mirror corner, between 3 o'clock and 6. The disc is `scenes/wheels/day.svg` and turns once per local civil day. Night, dawn, day, and dusk are separate scenes. Mid-morning shows a high sun, blue sky, and trees.
- **Calendar and forecast.** Two untitled boards under the wheels, together exactly as wide as the face, on a near-black field with a visible grid of unlit lamps. Letters and numbers are an original 5×7 amber dot matrix (about 16 px tall on the 643-pixel face). The lines are left-aligned, so each TimeWarp mark leads the line and is drawn in that same amber, not as a color emoji. The left board leads with the local time (a clock mark, then HH:MM:SS and the Cycles of Life color name, for example `10:08:37 RED`), then the calendar: a calendar or holiday mark, the event, and the date or clock time indented under it. The right board leads with the local date (a calendar mark, then a line such as THU JAN 15 2026). A single row of lit lamps sits under each of those headings. One dark lamp row stays above the rule, and a full character cell of unlit lamps sits below it, then sunrise and sunset (⬆️ and ⬇️, the same marks TimeWarp uses), the season, the moon phase, and moonrise and moonset when that local day has them, then the five-day forecast. A wrapped line, on either board, continues two character cells in from the left. Sunrise and sunset use the NOAA equation. Moonrise and moonset use the same Schlyter scan as TimeWarp. Times are the machine's local civil clock.
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
