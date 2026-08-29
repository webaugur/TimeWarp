import unittest
from datetime import date
from pathlib import Path

from timewarp.passes import load_tle_file, parse_tle_text, passes_for_day, select_sats, twilight_label
from timewarp.places import lookup_place

TLE = Path(__file__).resolve().parent / "data" / "iss.tle"


class TleParseTests(unittest.TestCase):
    def test_fixture_iss(self):
        sats = load_tle_file(TLE)
        self.assertEqual(len(sats), 1)
        self.assertEqual(sats[0].catalog, 25544)
        self.assertIn("ISS", sats[0].name.upper())
        picked = select_sats(sats, "ISS", all_sats=False)
        self.assertEqual(picked[0].catalog, 25544)

    def test_malformed(self):
        from timewarp.errors import TimeWarpError

        with self.assertRaises(TimeWarpError):
            parse_tle_text("not a tle")


class TwilightLabelTests(unittest.TestCase):
    def test_bins(self):
        self.assertEqual(twilight_label(10.0), "day")
        self.assertEqual(twilight_label(-3.0), "civil")
        self.assertEqual(twilight_label(-9.0), "nautical")
        self.assertEqual(twilight_label(-15.0), "astronomical")
        self.assertEqual(twilight_label(-20.0), "night")


class PassGeometryTests(unittest.TestCase):
    def test_nyc_2019_12_10_has_a_high_pass(self):
        sats = load_tle_file(TLE)
        place = lookup_place("New York")
        rows = passes_for_day(sats[0], date(2019, 12, 10), place, min_elev=10.0)
        self.assertGreaterEqual(len(rows), 1)
        best = max(rows, key=lambda p: p.max_alt_deg)
        self.assertGreater(best.max_alt_deg, 40.0)
        self.assertLess(best.aos, best.tca)
        self.assertLess(best.tca, best.los)
        self.assertEqual(best.tca.date(), date(2019, 12, 10))
        self.assertIn(best.twilight, {"day", "civil", "nautical", "astronomical", "night"})
        self.assertGreaterEqual(best.moon_sep_deg, 0.0)
        self.assertLessEqual(best.moon_sep_deg, 180.0)


if __name__ == "__main__":
    unittest.main()
