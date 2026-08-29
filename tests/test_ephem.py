import unittest
from datetime import datetime, timezone

from timewarp.ephem import BODIES, SYMBOLS, day_number, format_body, position, sun_state
from timewarp.places import lookup_place
from timewarp.rise import events_for_day


# Schlyter tutorial: 19 April 1990, 00:00 UT, d = -3543.0
TEST = datetime(1990, 4, 19, 0, 0, tzinfo=timezone.utc)


class SymbolTests(unittest.TestCase):
    def test_every_body_has_a_symbol(self):
        for name in BODIES:
            self.assertIn(name, SYMBOLS)
            self.assertTrue(SYMBOLS[name])
            self.assertIn(SYMBOLS[name], format_body(name))
            self.assertIn(name, format_body(name))

    def test_color_tints_symbol_and_keeps_width(self):
        plain = format_body("mars", width=12)
        tinted = format_body("mars", color=True, width=12)
        self.assertTrue(plain.startswith("♂"))
        self.assertIn("\033[1;38;2;", tinted)
        self.assertIn("mars", tinted)
        self.assertEqual(len(plain), 12)


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
        # New York 2026-07-04: moonset ~09:53 local. Rise is late evening.
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
            events_for_day("sedna", datetime(2026, 7, 4).date(), place)

    def test_ceres_and_io_return(self):
        from unittest.mock import patch

        from timewarp.errors import TimeWarpError
        from timewarp.places import lookup_place
        from timewarp.rise import events_for_day

        place = lookup_place("London")
        # Frozen Kepler table if SBDB is unreachable (no live JPL in this test).
        with patch("timewarp.jpl.fetch_sbdb", side_effect=TimeWarpError("offline")), patch(
            "timewarp.horizons.fetch_horizons", side_effect=TimeWarpError("offline")
        ):
            for body in ("ceres", "io", "halley"):
                p = position(body, TEST)
                self.assertEqual(p.body, body)
                self.assertTrue(0 <= p.ra_deg < 360)
                ev = events_for_day(body, datetime(2026, 7, 4).date(), place)
                self.assertTrue(ev.rises or ev.sets or ev.note)

    def test_period_two_days(self):
        from datetime import date

        from timewarp.rise import events_for_period

        place = lookup_place("New York")
        rows = events_for_period("sun", date(2026, 7, 4), date(2026, 7, 5), place)
        self.assertEqual([r.date for r in rows], [date(2026, 7, 4), date(2026, 7, 5)])
        self.assertTrue(all(r.visible for r in rows))

    def test_polar_night_sun_not_visible(self):
        from datetime import date

        from timewarp.places import Place

        tromso = Place("Tromso", 69.6492, 18.9553, "Europe/Oslo")
        ev = events_for_day("sun", date(2026, 12, 21), tromso)
        self.assertFalse(ev.visible)
        self.assertFalse(ev.after_rise_33)
        self.assertFalse(ev.before_set_33)

    def test_sun_13_and_33_summer(self):
        from datetime import date

        place = lookup_place("Indianapolis")
        ev = events_for_day("sun", date(2026, 7, 4), place)
        self.assertTrue(ev.rises and ev.sets)
        self.assertTrue(ev.after_rise_13 and ev.before_set_13)
        self.assertTrue(ev.after_rise_33 and ev.before_set_33)
        seq = [
            ev.rises[0],
            ev.after_rise_13[0],
            ev.after_rise_33[0],
            ev.before_set_33[0],
            ev.before_set_13[0],
            ev.sets[0],
        ]
        self.assertEqual(seq, sorted(seq))

    def test_sun_33_unreached_in_winter(self):
        from datetime import date

        place = lookup_place("Indianapolis")
        ev = events_for_day("sun", date(2026, 12, 21), place)
        self.assertTrue(ev.rises and ev.sets)
        self.assertTrue(ev.after_rise_13 and ev.before_set_13)
        self.assertFalse(ev.after_rise_33)
        self.assertFalse(ev.before_set_33)


if __name__ == "__main__":
    unittest.main()
