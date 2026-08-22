import unittest

from timewarp.iana_places import _iso6709, iter_na_tz_places
from timewarp.places import lookup_place, place_names


class IanaCoordTests(unittest.TestCase):
    def test_indianapolis_zone_tab(self):
        lat, lon = _iso6709("+394606-0860929")
        self.assertAlmostEqual(lat, 39.768333, places=4)
        self.assertAlmostEqual(lon, -86.158056, places=4)

    def test_na_zones_include_indianapolis(self):
        names = {n for n, *_ in iter_na_tz_places()}
        self.assertIn("Indianapolis", names)


class CapitalLookupTests(unittest.TestCase):
    def test_indianapolis(self):
        p = lookup_place("Indianapolis")
        self.assertEqual(p.tz, "America/Indiana/Indianapolis")
        self.assertAlmostEqual(p.lat, 39.7686, places=3)

    def test_san_jose_california(self):
        p = lookup_place("San Jose, CA")
        self.assertEqual(p.name, "San Jose")
        self.assertEqual(p.tz, "America/Los_Angeles")
        self.assertAlmostEqual(p.lat, 37.3382, places=3)

    def test_us_canada_mexico_capitals(self):
        for name in (
            "Sacramento",
            "Albany",
            "Austin",
            "Edmonton",
            "Quebec City",
            "Whitehorse",
            "Mexicali",
            "Chetumal",
            "Washington, D.C.",
        ):
            p = lookup_place(name)
            self.assertEqual(p.name.split(",")[0] in name or p.name == name, True)
            self.assertTrue(p.tz.startswith("America/") or p.tz.startswith("Pacific/"))

    def test_names_include_requested_cities(self):
        names = set(place_names())
        self.assertIn("Indianapolis", names)
        self.assertIn("San Jose", names)
        self.assertGreater(len(names), 80)


if __name__ == "__main__":
    unittest.main()
