"""Config text for the clock image. No image mount and no network."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))

from boot_config import (
    merge_cmdline,
    merge_config_txt,
    output_size,
    pre_rotate_cw,
    render_clockface_toml,
    render_network_config,
    render_user_data,
    video_token,
    viewer_size,
)


class GeometryTest(unittest.TestCase):
    def test_kms_portrait_is_the_rotated_framebuffer(self) -> None:
        self.assertEqual(viewer_size(90), (1024, 1280))
        self.assertEqual(output_size(90, "kms"), (1024, 1280))
        self.assertEqual(pre_rotate_cw(90, "kms"), 0)
        self.assertEqual(video_token(90, "kms"), "video=HDMI-A-1:1280x1024@60,rotate=90")

    def test_labwc_prerotates_onto_the_panel(self) -> None:
        self.assertEqual(output_size(90, "labwc"), (1280, 1024))
        self.assertEqual(pre_rotate_cw(90, "labwc"), 90)
        self.assertNotIn("rotate=", video_token(90, "labwc"))

    def test_half_turns_keep_the_landscape_framebuffer(self) -> None:
        self.assertEqual(output_size(0, "kms"), (1280, 1024))
        self.assertEqual(output_size(180, "kms"), (1280, 1024))
        self.assertEqual(output_size(270, "kms"), (1024, 1280))


class CmdlineTest(unittest.TestCase):
    def test_splash_and_nocloud_are_added_once(self) -> None:
        existing = (
            "console=serial0,115200 console=tty1 root=PARTUUID=abc "
            "rootfstype=ext4 fsck.repair=yes rootwait quiet splash"
        )
        merged = merge_cmdline(
            existing,
            kiosk="kms",
            rotation=90,
            instance_id="clock-sdcard-1",
            wifi_country="us",
        ).split()
        self.assertEqual(merged.count("quiet"), 1)
        self.assertEqual(merged.count("splash"), 1)
        self.assertIn("plymouth.ignore-serial-consoles", merged)
        self.assertIn("logo.nologo", merged)
        self.assertIn("vt.global_cursor_default=0", merged)
        self.assertIn("consoleblank=0", merged)
        self.assertIn("cfg80211.ieee80211_regdom=US", merged)
        self.assertIn("ds=nocloud;i=clock-sdcard-1", merged)
        self.assertIn("video=HDMI-A-1:1280x1024@60,rotate=90", merged)
        self.assertTrue(merged[0].startswith("console="))

    def test_rerun_replaces_video_and_instance(self) -> None:
        once = merge_cmdline(
            "console=tty1 rootwait",
            kiosk="labwc",
            rotation=90,
            instance_id="clock-sdcard-1",
            wifi_country="US",
        )
        twice = merge_cmdline(
            once,
            kiosk="kms",
            rotation=180,
            instance_id="clock-sdcard-2",
            wifi_country="US",
        ).split()
        videos = [tok for tok in twice if tok.startswith("video=")]
        clouds = [tok for tok in twice if tok.startswith("ds=nocloud")]
        self.assertEqual(videos, ["video=HDMI-A-1:1280x1024@60,rotate=180"])
        self.assertEqual(clouds, ["ds=nocloud;i=clock-sdcard-2"])


class ConfigTxtTest(unittest.TestCase):
    def test_spi_and_splash_and_legacy_rotate(self) -> None:
        text = merge_config_txt(
            "dtparam=audio=on\n#dtparam=spi=on\ndisplay_rotate=1\n"
        )
        self.assertIn("dtparam=spi=on\n", text)
        self.assertNotIn("#dtparam=spi=on", text)
        self.assertIn("disable_splash=1\n", text)
        self.assertNotIn("display_rotate=1", text)
        again = merge_config_txt(text)
        self.assertEqual(again.count("dtparam=spi=on"), 1)
        self.assertEqual(again.count("disable_splash=1"), 1)


class CloudInitTest(unittest.TestCase):
    def test_user_keeps_a_password_prompt(self) -> None:
        text = render_user_data(
            hostname="clock",
            user="clock",
            password_hash="$6$rounds=1$salt$hash",
            timezone="America/Indiana/Indianapolis",
            locale="en_US.UTF-8",
            keyboard="us",
            ssh=True,
            authorized_keys=[],
        )
        self.assertTrue(text.startswith("#cloud-config\n"))
        self.assertIn("sudo: null", text)
        self.assertNotIn("NOPASSWD", text)
        self.assertIn("ssh_pwauth: true", text)
        self.assertIn("systemctl, enable, --now, ssh", text)
        self.assertIn("Check-Date", text)

    def test_wifi_keeps_ethernet(self) -> None:
        text = render_network_config(
            ssid='cafe "wifi"',
            psk="secret",
            country="us",
            hidden=False,
        )
        self.assertIn("eth0:", text)
        self.assertIn('cafe \\"wifi\\"', text)
        self.assertIn('regulatory-domain: "US"', text)
        self.assertEqual(render_network_config(ssid="", psk="", country="US", hidden=False), "")


class PartitionTest(unittest.TestCase):
    def test_mbr_slots(self) -> None:
        import tempfile

        from boot_config import mbr_partitions

        blob = bytearray(512)
        blob[510:512] = b"\x55\xaa"
        entry = bytearray(16)
        entry[4] = 0x0C
        entry[8:12] = (16384).to_bytes(4, "little")
        entry[12:16] = (1048576).to_bytes(4, "little")
        blob[446:462] = entry
        entry2 = bytearray(16)
        entry2[4] = 0x83
        entry2[8:12] = (1064960).to_bytes(4, "little")
        entry2[12:16] = (100).to_bytes(4, "little")
        blob[462:478] = entry2
        with tempfile.TemporaryDirectory() as tmp:
            image = Path(tmp) / "card.img"
            image.write_bytes(blob)
            self.assertEqual(
                mbr_partitions(image),
                [(0x0C, 16384, 1048576), (0x83, 1064960, 100)],
            )


class TomlTest(unittest.TestCase):
    def test_audio_keys_follow_the_crate(self) -> None:
        example = Path(__file__).resolve().parents[2].joinpath(
            "clockface", "clockface.example.toml"
        ).read_text(encoding="utf-8")
        plain = render_clockface_toml(
            example, audio=False, timewarp_bin="/usr/local/bin/timewarp"
        )
        self.assertNotIn("[audio]", plain)
        self.assertIn('units = "imperial"', plain)
        self.assertIn('timewarp_bin = "/usr/local/bin/timewarp"', plain)
        self.assertNotIn("# timewarp_bin", plain)
        with_audio = render_clockface_toml(
            example, audio=True, timewarp_bin="/usr/local/bin/timewarp"
        )
        self.assertIn('tick = "15db"', with_audio)
        self.assertIn('mute_path = "/tmp/mute"', with_audio)
        self.assertNotIn("gpio_path =", with_audio.split("[audio]", 1)[1])


if __name__ == "__main__":
    unittest.main()
