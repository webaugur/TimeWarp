# Clock SD card

The procedure, the hardware checklist, and the risks are in the root README under **Rebuilding the clock SD card**. This file is the short map of the directory.

| File | Role |
| --- | --- |
| `config.example.env` | Hostname, user, Wi-Fi, timezone, rotation, `kms` or `labwc`, audio sink, image pin. No passwords. |
| `build-image.sh` | Download the pinned Trixie Lite image, verify SHA256, customize it offline. `--dry-run` prints the plan. `--device` writes a card after a typed confirmation. |
| `cross-build.sh` | `clockface` for `aarch64-unknown-linux-gnu` in a Debian trixie container. `--labwc` for the desktop binary. `--dry-run` prints the cargo line. |
| `cross/Dockerfile` | Trixie amd64 plus arm64 `-dev` packages, including `libasound2-dev`. |
| `provision.sh` | First boot on the Pi. Packages, timewarp venv, kiosk unit, analog audio, timesyncd, Plymouth, `MODULES=most`. Log: `/var/log/clock-provision.log`. |
| `make-boot-theme.py` | Wizard Plymouth frames. `--wizard-src` if IndianaDell cannot be fetched. `--synthetic` is for tests only. |
| `lib/boot_config.py` | cmdline, config.txt, cloud-init, and the clockface TOML. |
| `files/` | kiosk launcher, labwc autostart, PipeWire sink helper, provision unit. |

```bash
sdcard/build-image.sh --dry-run
sudo --preserve-env=CLOCK_PASSWORD,CLOCK_WIFI_PSK \
  sdcard/build-image.sh --config sdcard/config.env
```

`CLOCKFACE_REF` empty means the checkout you have. The card's `clockface.toml` gets `[audio]` (`tick = "15db"`, `mute_path = "/tmp/mute"`) and `[leds]` (`backend = "spi"`, `/dev/spidev0.0`, 20 pixels, brightness 192, `gbr`, 400 kHz, `honor_mute`) for a revision that has them; the parser rejects unknown tables, so an older `CLOCKFACE_REF` gets neither. A `--labwc` binary has no SPI writer and gets `backend = "none"`. The chime and tick FLACs are compiled into the binary; nothing is copied to `sounds_dir`.
