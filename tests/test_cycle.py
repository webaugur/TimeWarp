import json
import unittest
from datetime import date, datetime, timezone

from timewarp.cycle import (
    GREENWICH,
    cycle_1690,
    daily_period,
    life_period,
    rosicrucian_stamp,
    soul_period,
    yearly_period,
)
from tests.test_cli import run


class StampTests(unittest.TestCase):
    def test_2026_aug_29_is_3379_162(self):
        stamp = rosicrucian_stamp(date(2026, 8, 29))
        self.assertEqual(stamp.stamp(), "3379.162")
        self.assertEqual(stamp.rc_year, 3379)
        self.assertEqual(stamp.day, 162)
        self.assertEqual(stamp.equinox.date(), date(2026, 3, 20))

    def test_before_equinox_is_previous_rc_year(self):
        stamp = rosicrucian_stamp(date(2026, 3, 19))
        self.assertEqual(stamp.rc_year, 3378)

    def test_equinox_day_is_000_after_sunrise(self):
        # Year rolls at sunrise on the equinox date, not at 14:32Z.
        stamp = rosicrucian_stamp(datetime(2026, 3, 20, 12, 0, tzinfo=timezone.utc))
        self.assertEqual(stamp.stamp(), "3379.000")

    def test_before_sunrise_on_equinox_date_is_previous_year(self):
        stamp = rosicrucian_stamp(datetime(2026, 3, 20, 3, 0, tzinfo=timezone.utc), GREENWICH)
        self.assertEqual(stamp.rc_year, 3378)

    def test_after_equinox_instant(self):
        stamp = rosicrucian_stamp(datetime(2026, 3, 21, tzinfo=timezone.utc))
        self.assertEqual(stamp.rc_year, 3379)
        self.assertGreaterEqual(stamp.day, 0)


class Cycle1690Tests(unittest.TestCase):
    def test_epoch_337(self):
        cyc = cycle_1690(date(337, 3, 22))
        self.assertEqual(cyc.index, 0)
        self.assertEqual(cyc.start.date(), date(337, 3, 21))

    def test_2026_still_cycle_zero_ending_2027(self):
        cyc = cycle_1690(date(2026, 8, 29))
        self.assertEqual(cyc.index, 0)
        self.assertEqual(cyc.end.date(), date(2027, 3, 20))
        self.assertGreater(cyc.remaining_days, 200)
        self.assertLess(cyc.remaining_days, 220)


class LewisTests(unittest.TestCase):
    def test_life_first_period(self):
        n, age, lo, hi = life_period(date(2020, 1, 1), date(2026, 8, 29))
        self.assertEqual(n, 1)
        self.assertEqual(lo, 0)
        self.assertEqual(hi, 7)
        self.assertEqual(age, 6)

    def test_life_second_period(self):
        n, age, lo, hi = life_period(date(2016, 1, 1), date(2026, 8, 29))
        self.assertEqual(n, 2)
        self.assertEqual(lo, 7)

    def test_yearly_starts_at_a(self):
        n, letter, key = yearly_period(date(2000, 8, 29), date(2026, 8, 29), "UTC")
        self.assertEqual(n, 1)
        self.assertEqual(letter, "A")
        self.assertEqual(key, "promotional")

    def test_soul_march_22_is_first(self):
        n, letter = soul_period(date(2000, 3, 22))
        self.assertEqual(n, 1)
        self.assertEqual(letter, "A")

    def test_soul_may_13_is_second(self):
        n, letter = soul_period(date(2000, 5, 13))
        self.assertEqual(n, 2)

    def test_soul_jan_28_is_seventh(self):
        n, letter = soul_period(date(2001, 1, 28))
        self.assertEqual(n, 7)
        self.assertEqual(letter, "G")

    def test_daily_uses_sunrise_and_saturday_letters(self):
        when = datetime(2026, 8, 29, 12, 0, tzinfo=timezone.utc)
        row = daily_period(when, GREENWICH)
        self.assertEqual(row["weekday"], "Saturday")
        self.assertIn(row["letter"], "ABCDEFG")
        self.assertGreaterEqual(row["period"], 1)
        self.assertLessEqual(row["period"], 7)


class CliCycleTests(unittest.TestCase):
    def test_quiet_stamp(self):
        code, out, err = run("cycle", "-q", "2026-08-29")
        self.assertEqual(code, 0, err)
        self.assertEqual(out.strip(), "3379.162")

    def test_json_has_cycle(self):
        code, out, err = run("cycle", "--json", "2026-08-29")
        self.assertEqual(code, 0, err)
        payload = json.loads(out)
        self.assertEqual(payload["stamp"], "3379.162")
        self.assertEqual(payload["cycle_1690"]["index"], 0)

    def test_born_adds_lewis(self):
        code, out, err = run("cycle", "--json", "2026-08-29", "--born", "1960-03-22")
        self.assertEqual(code, 0, err)
        payload = json.loads(out)
        self.assertIn("lewis", payload)
        self.assertEqual(payload["lewis"]["soul_period"], 1)

    def test_alias(self):
        code, out, err = run("rosicrucian", "-q", "2026-08-29")
        self.assertEqual(code, 0, err)
        self.assertEqual(out.strip(), "3379.162")

    def test_no_color_has_no_music_emoji(self):
        code, out, err = run("cycle", "--no-color", "2026-08-29")
        self.assertEqual(code, 0, err)
        self.assertIn("Star Date:", out)
        self.assertNotIn("Stamp:", out)
        self.assertNotIn("🎼", out)
        self.assertNotIn("🎵", out)

    def test_color_puts_note_emoji_with_letter_not_star_date(self):
        code, out, err = run("cycle", "--color", "2026-08-29")
        self.assertEqual(code, 0, err)
        star = next(ln for ln in out.splitlines() if "Star Date:" in ln)
        note = next(ln for ln in out.splitlines() if "Note:" in ln)
        self.assertNotIn("🎼", star)
        self.assertNotIn("🎵", star)
        self.assertIn("🎵", note)


if __name__ == "__main__":
    unittest.main()
