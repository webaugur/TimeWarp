"""Plymouth frame geometry. Uses the synthetic wizard so IndianaDell is not required."""

from __future__ import annotations

import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from PIL import Image, ImageDraw

SDCARD = Path(__file__).resolve().parents[1]
SCRIPT = SDCARD / "make-boot-theme.py"


def load_theme():
    spec = importlib.util.spec_from_file_location("make_boot_theme", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class RotateTest(unittest.TestCase):
    def test_clockwise_quarter_turn_moves_the_top_to_the_right(self) -> None:
        theme = load_theme()
        image = Image.new("RGB", (1024, 1280), (0, 0, 0))
        image.putpixel((512, 0), (255, 0, 0))
        turned = theme.rotate_cw(image, 90)
        self.assertEqual(turned.size, (1280, 1024))
        self.assertEqual(turned.getpixel((1279, 512)), (255, 0, 0))
        self.assertEqual(theme.rotate_cw(image, 0).size, (1024, 1280))


class EyeFillTest(unittest.TestCase):
    def test_only_the_paired_eye_sockets_are_filled(self) -> None:
        theme = load_theme()
        image = Image.new("RGBA", (80, 80), (0, 0, 0, 0))
        draw = ImageDraw.Draw(image)
        skin = (236, 186, 150, 255)
        draw.rectangle([8, 8, 71, 71], fill=skin)
        # Two hollow eyes with dark pupils, plus a scroll-like hole that must stay clear.
        for left in (18, 46):
            draw.ellipse([left, 24, left + 14, 44], fill=(0, 0, 0, 0))
            draw.ellipse([left + 4, 30, left + 10, 38], fill=(16, 42, 48, 255))
        draw.rectangle([22, 56, 40, 66], fill=(0, 0, 0, 0))
        filled = theme.fill_hollow_eyes(image)
        pupil = filled.getpixel((25, 34))
        self.assertLess(theme._luminance(*pupil[:3]), 80)
        self.assertEqual(pupil[3], 255)
        # A pixel in the sclera ring, outside the pupil, becomes opaque white.
        sclera = filled.getpixel((19, 28))
        self.assertEqual(sclera[3], 255)
        self.assertGreater(sclera[0], 240)
        # The extra interior hole, and the exterior, stay transparent.
        self.assertEqual(filled.getpixel((30, 60))[3], 0)
        self.assertEqual(filled.getpixel((0, 0))[3], 0)


class FrameTest(unittest.TestCase):
    def test_kms_and_labwc_frame_sizes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for kiosk, size in (("kms", (1024, 1280)), ("labwc", (1280, 1024))):
                out = root / kiosk
                subprocess.run(
                    [
                        sys.executable,
                        str(SCRIPT),
                        "--synthetic",
                        "--no-text",
                        "--frames",
                        "2",
                        "--rotation",
                        "90",
                        "--kiosk",
                        kiosk,
                        "--out",
                        str(out),
                    ],
                    check=True,
                )
                with Image.open(out / "animation-0001.png") as frame:
                    self.assertEqual(frame.size, size)
                with Image.open(out / "throbber-0002.png") as throb:
                    self.assertEqual(throb.size, size)
                self.assertTrue((out / "clockface.plymouth").is_file())
                with Image.open(out / "preview.gif") as preview:
                    self.assertEqual(preview.size[1], 640)
                    self.assertGreater(preview.n_frames, 1)


if __name__ == "__main__":
    unittest.main()
