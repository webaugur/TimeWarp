import unittest
from datetime import date

from timewarp.liturgy import compute_liturgy, easter_sunday, roman_date
from tests.test_cli import run


class ComputusTests(unittest.TestCase):
    def test_known_easters(self):
        self.assertEqual(easter_sunday(2024), date(2024, 3, 31))
        self.assertEqual(easter_sunday(2025), date(2025, 4, 20))
        self.assertEqual(easter_sunday(2026), date(2026, 4, 5))

    def test_september_is_ordinary_and_kalends(self):
        day = compute_liturgy(date(2026, 9, 22))
        self.assertEqual(day.season, "Ordinary Time")
        self.assertIsNone(day.feast)
        self.assertEqual(day.easter, date(2026, 4, 5))
        self.assertEqual(day.roman_abbr, "a.d. X Kal. Oct.")
        self.assertEqual(day.ash_wednesday, date(2026, 2, 18))

    def test_christmas_and_ides(self):
        day = compute_liturgy(date(2026, 12, 25))
        self.assertEqual(day.season, "Christmas")
        self.assertEqual(day.feast, "Christmas")
        abbr, _plain = roman_date(date(2026, 3, 15))
        self.assertEqual(abbr, "Id. Mar.")


class LiturgyCliTests(unittest.TestCase):
    def test_orthodox_easter_2024(self):
        from timewarp.liturgy import julian_easter

        _julian, greg = julian_easter(2024)
        self.assertEqual(greg, date(2024, 5, 5))

    def test_1962_septuagesima_and_latin(self):
        day = compute_liturgy(date(2026, 2, 1), calendar="1962", lang="la")
        self.assertEqual(day.season, "Septuagesima")
        christmas = compute_liturgy(date(2026, 12, 25), lang="la")
        self.assertEqual(christmas.feast, "Nativitas Domini")

    def test_ember_after_lucy(self):
        from timewarp.liturgy import ember_dates

        days = ember_dates(2026)
        self.assertTrue(any(d.month == 12 and d.weekday() == 2 for d in days))

    def test_may_1_rank_differs(self):
        old = compute_liturgy(date(2026, 5, 1), calendar="1962")
        new = compute_liturgy(date(2026, 5, 1), calendar="1970")
        self.assertEqual(old.rank, "feast")
        self.assertEqual(new.rank, "optional")

    def test_quiet(self):
        code, out, err = run("liturgy", "-q", "2026-09-22")
        self.assertEqual(code, 0, err)
        self.assertIn("Ordinary Time", out)
        self.assertIn("2026-04-05", out)
        self.assertIn("a.d. X Kal. Oct.", out)
