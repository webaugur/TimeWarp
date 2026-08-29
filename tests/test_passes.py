import os
import tempfile
import unittest
from datetime import date
from pathlib import Path

from timewarp.passes import (
    load_tle_file,
    normalize_catalog,
    parse_tle_text,
    passes_for_day,
    select_sats,
    standard_magnitude,
    twilight_label,
    visual_magnitude,
)
from timewarp.places import lookup_place

TLE = Path(__file__).resolve().parent / "data" / "iss.tle"
SATCAT = Path(__file__).resolve().parent / "data" / "satcat-iss.csv"

_TLE_DIR = Path(tempfile.gettempdir()) / "timewarp-tests-tle-passes"
_TLE_DIR.mkdir(parents=True, exist_ok=True)
if SATCAT.is_file():
    (_TLE_DIR / "satcat.csv").write_bytes(SATCAT.read_bytes())
os.environ["TIMEWARP_TLE_DIR"] = str(_TLE_DIR)


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
    def setUp(self):
        import timewarp.passes as passes_mod

        passes_mod._SATCAT = None
        passes_mod._SATCAT_KEY = None

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
        self.assertIsNotNone(best.magnitude)
        self.assertGreater(best.magnitude, -6.0)
        self.assertLess(best.magnitude, 8.0)


class CatalogTests(unittest.TestCase):
    def test_aliases(self):
        self.assertEqual(normalize_catalog("gps"), "gps-ops")
        self.assertEqual(normalize_catalog("visual"), "visual")

    def test_unknown(self):
        from timewarp.errors import TimeWarpError

        with self.assertRaises(TimeWarpError):
            normalize_catalog("not-a-catalog")


class MagnitudeTests(unittest.TestCase):
    def test_iss_rcs_stdmag(self):
        std = standard_magnitude(399.0524)
        self.assertAlmostEqual(std, 5.0 - 2.5 * __import__("math").log10(399.0524), places=6)
        mag = visual_magnitude(400.0, 90.0, 399.0524)
        self.assertLess(mag, std)


if __name__ == "__main__":
    unittest.main()
