# clockface

Roman-numeral analog clock for a Raspberry Pi kiosk. It is the start of a grandfather-clock UI: a reusable egui painter plus a fullscreen shell. Later it can pull calendar and moon data from TimeWarp's `--json` output. The face is drawn from a geometry spec (cream dial, teardrop ticks, XII / III / VI / IX, shadowed hands). It does not embed the old timeanddate.com script.

Time comes from `chrono::Local`, so the zoneinfo rules on the Pi apply, including DST. The kiosk this replaces hardcoded EDT (UTC−4) and ran an hour fast in winter.

## Build on Raspberry Pi OS 64-bit

The binary uses the **glow** backend (OpenGL / GLES through glutin), which is what Pi OS can drive. The wgpu backend is not enabled.

Install a current Rust toolchain (Raspberry Pi OS packages are usually too old; this crate's lockfile needs Rust 1.88 or newer) and the window-system headers:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
sudo apt update
sudo apt install build-essential pkg-config \
  libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
  libxkbcommon-dev libgl1-mesa-dev libegl1-mesa-dev libwayland-dev
```

From this directory:

```bash
cd clockface
cargo build --release
```

The binary is `target/release/clockface`. It is an aarch64 ELF when built on Pi OS 64-bit (`uname -m` prints `aarch64`).

## Run

Fullscreen, diameter 600, centered horizontally, sitting against the top edge:

```bash
./target/release/clockface
```

Custom diameter and position (center is in pixels, origin at the top-left of the window):

```bash
./target/release/clockface --diameter 720 --center-x 960 --center-y 420
```

Flags override an optional TOML file:

```bash
./target/release/clockface --config clockface.example.toml --diameter 800
```

```toml
diameter = 600.0
center_x = 960.0
center_y = 300.0
windowed = false
```

Omit `center_x` to center horizontally. Omit `center_y` to use `diameter / 2`. `--windowed` opens a decorated window (default size 1920×1080, change with `--width` and `--height`). `--fullscreen` wins if the config file says `windowed = true`.

The window is cleared to black and repainted at about 60 fps, so the hour and minute hands move continuously and the second hand plays its short bounce at the start of each second.

If Wayland will not open a GL context, force the X11 backend:

```bash
WINIT_UNIX_BACKEND=x11 ./target/release/clockface
```

## Screenshot without a display

`--screenshot` renders one frame at local time **10:08:37** (15 January 2026) with the same painter and writes a PNG. No window is opened.

```bash
./target/release/clockface --screenshot clock.png --diameter 600 --center-x 960 --center-y 300
```

Width and height default to 1920×1080. With the defaults and no center flags, the face is centered at `(width/2, diameter/2)`.

## Autostart fullscreen

Build the release binary first, then start it with the desktop session (it needs the display; a system service will not see Wayland).

**labwc** (Raspberry Pi OS Bookworm and later). A user `autostart` replaces the system one, so copy the original and append the clock:

```bash
mkdir -p ~/.config/labwc
cp /etc/xdg/labwc/autostart ~/.config/labwc/autostart
echo "$HOME/TimeWarp/clockface/target/release/clockface --config $HOME/.config/clockface.toml &" >> ~/.config/labwc/autostart
```

**X11** (`raspi-config` desktop on X, or a `.desktop` session):

```ini
# ~/.config/autostart/clockface.desktop
[Desktop Entry]
Type=Application
Name=Clockface
Exec=/home/pi/TimeWarp/clockface/target/release/clockface
X-GNOME-Autostart-enabled=true
```

Log out and back in. The clock covers the screen; Alt+F4 or a switch to another tty stops it. Use `--windowed` while tuning `--diameter`, `--center-x`, and `--center-y`.

## Library

`RomanClockStyle` holds the geometry parameters. `paint` draws with the kiosk defaults:

```rust
clockface::paint(&painter, center, diameter, chrono::Local::now());
```

Call `clockface::install_fonts` once on the egui context so XII / III / VI / IX use the bundled Liberation Serif (SIL Open Font License, see `fonts/OFL.txt`).

## Test

```bash
cargo test
```

The render test paints 10:08:37 headlessly and checks the dial, rim, hands, and numeral color.
