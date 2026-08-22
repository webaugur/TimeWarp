import unittest
from datetime import datetime, timezone

from timewarp.ephem import day_number, position, sun_state
from timewarp.places import lookup_place
from timewarp.rise import events_for_day


# Schlyter tutorial: 19 April 1990, 00:00 UT, d = -3543.0
TEST = datetime(1990, 4, 19, 0, 0, tzinfo=timezone.utc)


class DayNumberTests(unittest.TestCase):
    def test_tutorial_d(self):
        self.assertAlmostEqual(day_number(TEST), -3543.0, places=6)


class SunTests(unittest.TestCase):
    def test_tutorial_sun(self):
        s = sun_state(day_number(TEST))
        self.assertAlmostEqual(s.lon, 28.6869, places=2)
        self.assertAlmostEqual(s.r, 1.004323, places=4)
        self.assertAlmostEqual(s.ra, 26.6580, places=2)
        self.assertAlmostEqual(s.dec, 11.0084, places=2)

    def test_sun_position_wrapper(self):
        p = position("sun", TEST)
        self.assertAlmostEqual(p.ra_deg, 26.6580, places=2)
        self.assertEqual(p.distance_unit, "AU")


class MoonTests(unittest.TestCase):
    def test_tutorial_moon(self):
        p = position("moon", TEST)
        self.assertAlmostEqual(p.ecl_lon, 306.9484, delta=0.05)
        self.assertAlmostEqual(p.ecl_lat, -0.5856, delta=0.05)
        self.assertAlmostEqual(p.distance, 60.6779, delta=0.05)
        self.assertAlmostEqual(p.ra_deg, 309.5011, delta=0.05)
        self.assertAlmostEqual(p.dec_deg, -19.1032, delta=0.05)


class PlanetTests(unittest.TestCase):
    def test_tutorial_mercury_geocentric(self):
        p = position("mercury", TEST)
        self.assertAlmostEqual(p.ra_deg, 43.2598, delta=0.05)
        self.assertAlmostEqual(p.dec_deg, 19.6460, delta=0.05)
        self.assertAlmostEqual(p.distance, 0.748296, delta=0.001)

    def test_inner_and_giant_planets_return(self):
        for body in ("venus", "mars", "jupiter", "saturn"):
            p = position(body, TEST)
            self.assertEqual(p.body, body)
            self.assertTrue(0 <= p.ra_deg < 360)
            self.assertTrue(-90 <= p.dec_deg <= 90)
            self.assertGreater(p.distance, 0)


class RiseTests(unittest.TestCase):
    def test_sun_rise_matches_noaa_within_a_few_minutes(self):
        from timewarp.astro import sun_times

        place = lookup_place("New York")
        day = datetime(2026, 7, 4, tzinfo=timezone.utc)
        noaa = sun_times(day.date(), place)
        sch = events_for_day("sun", day.date(), place)
        self.assertTrue(sch.rises)
        self.assertTrue(sch.sets)
        self.assertIsNotNone(noaa.sunrise)
        delta = abs((sch.rises[0] - noaa.sunrise).total_seconds())
        self.assertLess(delta, 8 * 60, msg=f"Schlyter {sch.rises[0]} vs NOAA {noaa.sunrise}")

    def test_moonrise_new_york_july_4_2026(self):
        place = lookup_place("New York")
        result = events_for_day("moon", datetime(2026, 7, 4).date(), place)
        # timeanddate New York 2026-07-04 moonset ~09:53. Rise is late evening.
        self.assertTrue(result.sets, msg=result.note)
        self.assertTrue(result.rises, msg=result.note)
        self.assertEqual(result.sets[0].tzinfo.key, "America/New_York")
        set_min = result.sets[0].hour * 60 + result.sets[0].minute
        self.assertLess(abs(set_min - (9 * 60 + 53)), 8)
        rise_min = result.rises[0].hour * 60 + result.rises[0].minute
        self.assertGreater(rise_min, 21 * 60)

    def test_moon_may_skip_a_rise(self):
        # Scan a month; at least one day should lack moonrise or moonset.
        place = lookup_place("New York")
        missing = 0
        from datetime import date, timedelta

        d = date(2026, 7, 1)
        for _ in range(31):
            ev = events_for_day("moon", d, place)
            if not ev.rises or not ev.sets:
                missing += 1
            d += timedelta(days=1)
        self.assertGreaterEqual(missing, 1)

    def test_venus_has_events_or_note(self):
        place = lookup_place("London")
        ev = events_for_day("venus", datetime(2026, 7, 4).date(), place)
        self.assertTrue(ev.rises or ev.sets or ev.note)

    def test_unknown_body(self):
        from timewarp.errors import TimeWarpError

        place = lookup_place("UTC")
        with self.assertRaises(TimeWarpError):
            events_for_day("ceres", datetime(2026, 7, 4).date(), place)


if __name__ == "__main__":
    unittest.main()
