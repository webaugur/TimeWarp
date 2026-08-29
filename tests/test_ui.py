import unittest

from timewarp.ui import EMOJI, format_body, icon, sky_bin_label, want_color


class FormatBodyTests(unittest.TestCase):
    def test_plain_keeps_iau_symbol(self):
        self.assertTrue(format_body("mars").startswith("♂"))
        self.assertIn("mars", format_body("mars"))

    def test_emoji_uses_modern_glyph(self):
        labeled = format_body("mars", color=True, emoji=True)
        self.assertIn(EMOJI["mars"], labeled)
        self.assertIn("mars", labeled)

    def test_width_accounts_for_cells(self):
        plain = format_body("sun", width=12, emoji=False)
        self.assertEqual(len(plain), 12)


class IconTests(unittest.TestCase):
    def test_off(self):
        self.assertEqual(icon("error", emoji=False), "")
        self.assertEqual(sky_bin_label("civil", emoji=False), "civil")

    def test_on(self):
        self.assertEqual(icon("error", emoji=True), "❌")
        self.assertIn("civil", sky_bin_label("civil", emoji=True))
        self.assertIn("🏙️", sky_bin_label("civil", emoji=True))


class GlyphPadTests(unittest.TestCase):
    def test_pads_up_never_crops(self):
        from rich.cells import cell_len

        from timewarp.ui import glyph_pad

        self.assertEqual(cell_len(glyph_pad("")), 3)
        self.assertGreaterEqual(cell_len(glyph_pad("🌞")), 2)
        self.assertGreaterEqual(cell_len(glyph_pad("♂️")), 2)
        self.assertGreaterEqual(cell_len(glyph_pad("☀️")), 2)


class GridTests(unittest.TestCase):
    def test_clocks_not_cropped(self):
        from io import StringIO

        from timewarp.ui import print_grid

        buf = StringIO()
        print_grid(
            ["sat", "aos", "max"],
            [["ISS (ZARYA)", "10:34R", "10:37R"]],
            color=False,
            file=buf,
        )
        text = buf.getvalue()
        self.assertIn("10:34R", text)
        self.assertIn("ISS (ZARYA)", text)


class WantColorTests(unittest.TestCase):
    def test_no_color_env(self):
        import os

        old = os.environ.get("NO_COLOR")
        os.environ["NO_COLOR"] = "1"
        try:
            self.assertFalse(want_color())
        finally:
            if old is None:
                os.environ.pop("NO_COLOR", None)
            else:
                os.environ["NO_COLOR"] = old


if __name__ == "__main__":
    unittest.main()
