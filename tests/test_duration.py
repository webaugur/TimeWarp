import unittest
from datetime import date, datetime

from timewarp.duration import Offset, apply_offset, parse_offset, span


class OffsetParseTests(unittest.TestCase):
    def test_human(self):
        off = parse_offset("7 months 6 days")
        self.assertEqual(off, Offset(months=7, days=6))

    def test_iso(self):
        off = parse_offset("P7M6D")
        self.assertEqual(off, Offset(months=7, days=6))

    def test_iso_time(self):
        off = parse_offset("PT3H15M")
        self.assertEqual(off, Offset(hours=3, minutes=15))

    def test_negative_iso(self):
        off = parse_offset("-P7M")
        self.assertEqual(off, Offset(months=-7))

    def test_attached(self):
        off = parse_offset("7mo 6d")
        self.assertEqual(off, Offset(months=7, days=6))


class AddTests(unittest.TestCase):
    def test_user_example(self):
        result = apply_offset(date(2026, 7, 4), Offset(months=7, days=6))
        self.assertEqual(result, date(2027, 2, 10))

    def test_month_end_clamp(self):
        result = apply_offset(date(2026, 1, 31), Offset(months=1))
        self.assertEqual(result, date(2026, 2, 28))

    def test_leap_month_end(self):
        result = apply_offset(date(2024, 1, 31), Offset(months=1))
        self.assertEqual(result, date(2024, 2, 29))

    def test_year_from_feb_29(self):
        result = apply_offset(date(2024, 2, 29), Offset(years=1))
        self.assertEqual(result, date(2025, 2, 28))

    def test_add_time_promotes(self):
        result = apply_offset(date(2026, 7, 4), Offset(hours=3, minutes=15))
        self.assertEqual(result, datetime(2026, 7, 4, 3, 15, 0))

    def test_larger_units_first(self):
        # years/months then days (larger units first)
        result = apply_offset(date(2026, 1, 31), Offset(months=1, days=1))
        self.assertEqual(result, date(2026, 3, 1))


class SpanTests(unittest.TestCase):
    def test_same_day(self):
        s = span(date(2026, 5, 31), date(2026, 5, 31))
        self.assertEqual(s.iso(), "P0D")
        self.assertEqual(s.total_days, 0)

    def test_one_day(self):
        s = span(date(2026, 5, 31), date(2026, 6, 1))
        self.assertEqual(s.total_days, 1)
        self.assertEqual(s.iso(), "P1D")

    def test_include_end(self):
        s = span(date(2026, 5, 31), date(2026, 6, 1), include_end=True)
        self.assertEqual(s.total_days, 2)

    def test_negative_user_example(self):
        # From 2026-05-31, minus 1 year minus 1 month clamps to 2025-04-30.
        start = date(2026, 5, 31)
        end = date(2025, 4, 30)
        s = span(start, end)
        self.assertEqual(s.years, -1)
        self.assertEqual(s.months, -1)
        self.assertEqual(s.days, 0)
        self.assertEqual(s.iso(), "-P1Y1M")
        self.assertLess(s.total_days, 0)
        self.assertEqual(s.total_days, -396)
        self.assertEqual(apply_offset(start, Offset(years=s.years, months=s.months, days=s.days)), end)

    def test_inverse_of_add(self):
        start = date(2026, 7, 4)
        end = apply_offset(start, Offset(months=7, days=6))
        s = span(start, end)
        self.assertEqual(s.years, 0)
        self.assertEqual(s.months, 7)
        self.assertEqual(s.days, 6)
        self.assertEqual(apply_offset(start, Offset(months=s.months, days=s.days)), end)

    def test_datetime_span(self):
        s = span(datetime(2026, 7, 4, 10, 0, 0), datetime(2026, 7, 5, 11, 30, 0))
        self.assertEqual(s.days, 1)
        self.assertEqual(s.hours, 1)
        self.assertEqual(s.minutes, 30)
        self.assertEqual(s.iso(), "P1DT1H30M")

    def test_negative_datetime(self):
        s = span(datetime(2026, 7, 5, 11, 30, 0), datetime(2026, 7, 4, 10, 0, 0))
        self.assertEqual(s.iso(), "-P1DT1H30M")
        self.assertEqual(s.sign, -1)


if __name__ == "__main__":
    unittest.main()
