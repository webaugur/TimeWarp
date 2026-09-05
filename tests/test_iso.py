import unittest
from datetime import date, datetime, timedelta, timezone

from timewarp.errors import TimeWarpError
from timewarp.iso import (
    BMT,
    datetime_from_beats,
    format_clock,
    format_instant,
    format_swatch,
    looks_like_instant,
    parse_instant,
    swatch_beats,
    tz_letter,
    weekday_name,
)


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

    def test_military_zone_letters(self):
        z = datetime(2026, 7, 4, 13, 0, tzinfo=timezone.utc)
        self.assertEqual(tz_letter(z), "Z")
        self.assertEqual(format_clock(z), "13:00Z @583")
        q = datetime(2026, 7, 4, 18, 52, tzinfo=timezone(timedelta(hours=-4)))
        self.assertEqual(format_clock(q), "18:52Q @994")
        r = datetime(2026, 1, 4, 17, 52, tzinfo=timezone(timedelta(hours=-5)))
        self.assertEqual(format_clock(r), "17:52R @994")

    def test_swatch_bmt_midnight_is_000(self):
        # @000 is 00:00 BMT = 23:00 UTC.
        midnight_bmt = datetime(2026, 7, 3, 23, 0, tzinfo=timezone.utc)
        self.assertEqual(format_swatch(midnight_bmt), "@000")
        self.assertAlmostEqual(swatch_beats(midnight_bmt), 0.0, places=6)
        noon_bmt = datetime(2026, 7, 4, 11, 0, tzinfo=timezone.utc)
        self.assertEqual(format_swatch(noon_bmt), "@500")

    def test_swatch_wraps_to_000(self):
        just_shy = datetime(2026, 7, 3, 22, 59, 59, 500000, tzinfo=timezone.utc)
        self.assertEqual(format_swatch(just_shy), "@000")

    def test_parse_beats_on_date(self):
        midnight = parse_instant("2026-07-04T@000")
        self.assertEqual(midnight, datetime(2026, 7, 4, 0, 0, tzinfo=BMT))
        self.assertEqual(format_swatch(midnight), "@000")
        noon = parse_instant("2026-07-04 @500")
        self.assertEqual(noon.astimezone(timezone.utc), datetime(2026, 7, 4, 11, 0, tzinfo=timezone.utc))
        self.assertEqual(format_swatch(noon), "@500")
        self.assertEqual(format_swatch(datetime_from_beats(date(2026, 7, 4), 583.333)), "@583")

    def test_parse_bare_beats(self):
        got = parse_instant("@500")
        self.assertEqual(format_swatch(got), "@500")
        self.assertEqual(got.tzinfo, BMT)
        self.assertTrue(looks_like_instant("@500"))
        self.assertTrue(looks_like_instant("2026-07-04T@000"))

    def test_parse_beats_out_of_range(self):
        with self.assertRaises(TimeWarpError) as ctx:
            parse_instant("2026-07-04T@1000")
        self.assertIn("@000", str(ctx.exception))
        with self.assertRaises(TimeWarpError):
            parse_instant("@abc")


if __name__ == "__main__":
    unittest.main()
