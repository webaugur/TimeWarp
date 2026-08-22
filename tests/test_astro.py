import unittest
from datetime import date

from timewarp.astro import moon_info, sun_times
from timewarp.eclipses import list_eclipses
from timewarp.places import lookup_place


class SunTests(unittest.TestCase):
    def test_new_york_july_has_long_day(self):
        place = lookup_place("New York")
        result = sun_times(date(2026, 7, 4), place)
        self.assertIsNotNone(result.sunrise)
        self.assertIsNotNone(result.sunset)
        self.assertGreater(result.day_length_seconds, 14 * 3600)
        self.assertLess(result.day_length_seconds, 16 * 3600)
        self.assertEqual(result.sunrise.tzinfo.key, "America/New_York")

    def test_arctic_june(self):
        from timewarp.places import Place

        tromso = Place("Tromso", 69.6492, 18.9553, "Europe/Oslo")
        result = sun_times(date(2026, 6, 21), tromso)
        self.assertTrue(result.note or result.day_length_seconds and result.day_length_seconds > 20 * 3600)


class MoonTests(unittest.TestCase):
    def test_known_fullish_around_catalog_lunar(self):
        info = moon_info(date(2026, 8, 28))
        self.assertGreater(info.illumination, 0.8)
        self.assertIn(info.phase, {"Full Moon", "Waning Gibbous", "Waxing Gibbous"})


class EclipseTests(unittest.TestCase):
    def test_2026_matches_screenshot_pair(self):
        rows = list_eclipses(year=2026)
        kinds = {(e.date.isoformat(), e.kind, e.type) for e in rows}
        self.assertIn(("2026-08-12", "solar", "total"), kinds)
        lunar = [e for e in rows if e.kind == "lunar" and e.date.isoformat().startswith("2026-08")]
        self.assertEqual(len(lunar), 1)
        self.assertEqual(lunar[0].end_date.isoformat(), "2026-08-28")

    def test_2027_annular(self):
        rows = list_eclipses(year=2027)
        self.assertTrue(any(e.date.isoformat() == "2027-02-06" and e.type == "annular" for e in rows))


if __name__ == "__main__":
    unittest.main()
