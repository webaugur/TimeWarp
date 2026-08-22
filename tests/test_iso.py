import unittest
from datetime import date, datetime, timezone

from timewarp.errors import TimeWarpError
from timewarp.iso import format_instant, parse_instant, weekday_name


class ParseIsoTests(unittest.TestCase):
    def test_date(self):
        self.assertEqual(parse_instant("2026-07-04"), date(2026, 7, 4))

    def test_datetime_naive(self):
        self.assertEqual(parse_instant("2026-07-04T09:30:00"), datetime(2026, 7, 4, 9, 30, 0))

    def test_datetime_space_separator(self):
        self.assertEqual(parse_instant("2026-07-04 09:30:00"), datetime(2026, 7, 4, 9, 30, 0))

    def test_zulu(self):
        got = parse_instant("2026-07-04T13:30:00Z")
        self.assertEqual(got, datetime(2026, 7, 4, 13, 30, 0, tzinfo=timezone.utc))

    def test_offset(self):
        got = parse_instant("2026-07-04T09:30:00-04:00")
        self.assertEqual(got.utcoffset().total_seconds(), -4 * 3600)

    def test_week_date(self):
        # 2026-07-04 is Saturday, week 27, weekday 6
        self.assertEqual(parse_instant("2026-W27-6"), date(2026, 7, 4))

    def test_ordinal(self):
        self.assertEqual(parse_instant("2026-185"), date(2026, 7, 4))

    def test_rejects_locale_order(self):
        with self.assertRaises(TimeWarpError):
            parse_instant("07/04/2026")
        with self.assertRaises(TimeWarpError):
            parse_instant("04-07-2026")

    def test_april_31_is_invalid(self):
        with self.assertRaises(TimeWarpError) as ctx:
            parse_instant("2025-04-31")
        msg = str(ctx.exception)
        self.assertIn("April 2025 has 30 days", msg)
        self.assertIn("2025-04-30", msg)

    def test_feb_29_non_leap(self):
        with self.assertRaises(TimeWarpError) as ctx:
            parse_instant("2026-02-29")
        self.assertIn("February 2026 has 28 days", str(ctx.exception))

    def test_weekday_english_not_locale(self):
        self.assertEqual(weekday_name(date(2026, 7, 4)), "Saturday")

    def test_format_date(self):
        self.assertEqual(format_instant(date(2026, 7, 4)), "2026-07-04")


if __name__ == "__main__":
    unittest.main()
