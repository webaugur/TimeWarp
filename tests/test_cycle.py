import json
import unittest
from datetime import date, datetime, timezone
from zoneinfo import ZoneInfo

from timewarp.cycle import (
    GREENWICH,
    NOTE_HEX,
    NOTE_NAME,
    NOTE_PLANET,
    chart_e_letter,
    cycle_1690,
    daily_period,
    dst_advance_seconds,
    life_period,
    period_index,
    rosicrucian_stamp,
    soul_period,
    yearly_period,
)
from timewarp.places import lookup_place
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

    def test_equinox_day_is_000_after_midnight(self):
        # Year rolls at local midnight on the equinox date, not at 14:32Z.
        stamp = rosicrucian_stamp(datetime(2026, 3, 20, 3, 0, tzinfo=timezone.utc), GREENWICH)
        self.assertEqual(stamp.stamp(), "3379.000")

    def test_before_midnight_on_equinox_date_is_previous_year(self):
        stamp = rosicrucian_stamp(datetime(2026, 3, 19, 23, 0, tzinfo=timezone.utc), GREENWICH)
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

    def test_daily_uses_midnight_and_saturday_letters(self):
        when = datetime(2026, 8, 29, 12, 0, tzinfo=timezone.utc)
        row = daily_period(when, GREENWICH)
        self.assertEqual(row["weekday"], "Saturday")
        self.assertIn(row["letter"], "ABCDEFG")
        self.assertGreaterEqual(row["period"], 1)
        self.assertLessEqual(row["period"], 7)
        self.assertEqual(row["color"], NOTE_NAME[row["letter"]])
        self.assertEqual(row["color_hex"], NOTE_HEX[row["letter"]])
        self.assertEqual(row["planet"], NOTE_PLANET[row["letter"]])

    def test_planets_follow_the_letter_not_the_period_number(self):
        self.assertEqual(
            [NOTE_PLANET[letter] for letter in "ABCDEFG"],
            ["sun", "venus", "mercury", "moon", "saturn", "jupiter", "mars"],
        )
        # Sunday's first period is G, Mars, not the Sun that used to sit on period 1.
        self.assertEqual(chart_e_letter(0, 0), "G")
        self.assertEqual(NOTE_PLANET["G"], "mars")

    def test_sunday_2223_est_is_jupiter_blue(self):
        # 2026-01-18 is a Sunday. 22:23 EST is Lewis's last period, letter F.
        indy = lookup_place("Indianapolis")
        when = datetime(2026, 1, 18, 22, 23, tzinfo=ZoneInfo("America/Indiana/Indianapolis"))
        row = daily_period(when, indy)
        self.assertEqual(row["dst_advance_seconds"], 0)
        self.assertEqual(row["letter"], "F")
        self.assertEqual(row["color"], "blue")
        self.assertEqual(row["color_hex"], "#1e5ab0")
        self.assertEqual(row["planet"], "jupiter")
        self.assertEqual(row["period"], 7)

    def test_exact_seventh_not_the_rounded_minute(self):
        # 03:25:42 standard is still the first period. 03:25:43 is the second.
        # Lewis prints 03:25 as the boundary; classification uses 86400/7.
        indy = lookup_place("Indianapolis")
        zone = ZoneInfo("America/Indiana/Indianapolis")
        before = daily_period(datetime(2026, 1, 18, 3, 25, 42, tzinfo=zone), indy)
        after = daily_period(datetime(2026, 1, 18, 3, 25, 43, tzinfo=zone), indy)
        self.assertEqual(period_index(3 * 3600 + 25 * 60 + 42), 0)
        self.assertEqual(period_index(3 * 3600 + 25 * 60 + 43), 1)
        self.assertEqual(before["letter"], "G")
        self.assertEqual(before["color"], "red")
        self.assertEqual(after["letter"], "A")
        self.assertEqual(after["color"], "orange")
        self.assertEqual(after["color_hex"], "#ff8c00")
        self.assertEqual(after["planet"], "sun")

    def test_indianapolis_summer_removes_daylight_saving(self):
        # 04:25 EDT is 03:25 EST, still before the exact seventh, so period 1 / B.
        # The civil clock alone would already be in the next period.
        indy = lookup_place("Indianapolis")
        zone = ZoneInfo("America/Indiana/Indianapolis")
        when = datetime(2026, 7, 15, 4, 25, tzinfo=zone)
        self.assertEqual(dst_advance_seconds(when), 3600)
        row = daily_period(when, indy)
        self.assertEqual(row["weekday"], "Wednesday")
        self.assertEqual(row["dst_advance_seconds"], 3600)
        self.assertEqual(row["letter"], "B")
        self.assertEqual(row["color"], "yellow")
        self.assertEqual(row["color_hex"], "#e6c200")
        self.assertEqual(row["planet"], "venus")
        self.assertEqual(row["period"], 1)

    def test_json_period_table_covers_the_neighboring_days(self):
        code, out, err = run(
            "cycle",
            "--json",
            "--city",
            "Indianapolis",
            "2026-01-18T22:23:00-05:00",
        )
        self.assertEqual(code, 0, err)
        payload = json.loads(out)
        self.assertEqual(payload["daily"]["letter"], "F")
        self.assertEqual(payload["daily"]["color"], "blue")
        days = payload["period_days"]
        self.assertEqual([row["date"] for row in days], ["2026-01-17", "2026-01-18", "2026-01-19"])
        sunday = days[1]
        self.assertEqual(sunday["dst_advance_seconds"], 0)
        last = sunday["periods"][6]
        self.assertEqual(last["letter"], "F")
        self.assertEqual(last["planet"], "jupiter")
        self.assertEqual(last["color_hex"], "#1e5ab0")
        self.assertLess(last["start_sec"], 22 * 3600 + 23 * 60)
        self.assertEqual(last["end_sec"], 86400)


class CliCycleTests(unittest.TestCase):
    def test_quiet_stamp(self):
        code, out, err = run("cycle", "-q", "2026-08-29")
        self.assertEqual(code, 0, err)
        self.assertEqual(out.strip(), "3379.162")

    def test_beats_input(self):
        code, out, err = run("cycle", "-q", "2026-07-04T@000", "--city", "Greenwich")
        self.assertEqual(code, 0, err)
        self.assertEqual(out.strip(), "3379.106")

    def test_json_has_cycle(self):
        code, out, err = run("cycle", "--json", "2026-08-29")
        self.assertEqual(code, 0, err)
        payload = json.loads(out)
        self.assertEqual(payload["stamp"], "3379.162")
        self.assertEqual(payload["cycle_1690"]["index"], 0)
        self.assertIn("color", payload["daily"])
        self.assertIn("color_hex", payload["daily"])

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
        self.assertIn("Color:", out)
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
        color = next(ln for ln in out.splitlines() if "Color:" in ln)
        self.assertIn("\033[48;2;", color)


if __name__ == "__main__":
    unittest.main()
