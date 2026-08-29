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


class KvAlignTests(unittest.TestCase):
    def test_keys_right_align_and_emoji_trails(self):
        from io import StringIO

        from timewarp.ui import print_kv

        buf = StringIO()
        print_kv(
            [
                ("⬆️", "Sunrise:", "05:32Q", "58.2° NE"),
                ("🕛", "Solar noon:", "13:01Q", ""),
                ("⬇️", "Sunset:", "20:31Q", "301.5° NW"),
            ],
            color=True,
            file=buf,
        )
        lines = [ln.rstrip() for ln in buf.getvalue().splitlines() if ln.strip()]
        self.assertEqual({ln.index(":") for ln in lines}, {10})
        self.assertEqual(lines[0].index("05:32Q"), lines[1].index("13:01Q"))
        self.assertEqual(lines[0].index("05:32Q"), lines[2].index("20:31Q"))
        self.assertEqual(lines[0].index("⬆️"), lines[2].index("⬇️"))
        self.assertLess(lines[0].index("05:32Q"), lines[0].index("58.2°"))
        self.assertLess(lines[0].index("58.2°"), lines[0].index("⬆️"))

    def test_blocks_share_colon_column(self):
        from io import StringIO

        from timewarp.ui import print_kv_blocks

        buf = StringIO()
        print_kv_blocks(
            [
                [("Body:", "ceres"), ("RA/Dec (noon):", "73.524° / 20.352°")],
                [("Rise:", "03:10A", "55.2° NE"), ("Transit:", "11:04A", "alt 58.8°"), ("Set:", "18:59A", "304.9° NW")],
            ],
            color=False,
            file=buf,
        )
        lines = [ln.rstrip() for ln in buf.getvalue().splitlines() if ln.strip()]
        self.assertEqual({ln.index(":") for ln in lines}, {13})

    def test_place_does_not_stretch_iso_extras(self):
        from io import StringIO

        from timewarp.ui import print_kv_blocks

        buf = StringIO()
        print_kv_blocks(
            [
                [
                    ("Body:", "moon"),
                    ("Place:", "Indianapolis (America/Indiana/Indianapolis)"),
                ],
                [
                    ("Next new:", "23:27Q", "2026-09-11T03:27:50+00:00"),
                    ("Next first Q:", "16:44Q", "2026-09-18T20:44:35+00:00"),
                ],
            ],
            color=False,
            file=buf,
        )
        lines = [ln.rstrip() for ln in buf.getvalue().splitlines() if ln.strip()]
        new = next(ln for ln in lines if ln.strip().startswith("Next new:"))
        first = next(ln for ln in lines if "Next first Q:" in ln)
        self.assertEqual(new.index("23:27Q"), first.index("16:44Q"))
        self.assertEqual(new.index("2026-09-11"), new.index("23:27Q") + len("23:27Q") + 2)
        self.assertEqual({ln.index(":") for ln in lines}, {12})


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
