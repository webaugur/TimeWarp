"""Plymouth frame geometry. Uses the synthetic wizard so IndianaDell is not required."""

from __future__ import annotations

import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from PIL import Image

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
