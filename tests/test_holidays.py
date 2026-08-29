import os
import tempfile
import unittest
from datetime import date
from pathlib import Path

from timewarp.holidays import holidays_for_year
from timewarp.workdays import add_workdays, count_workdays

FIXTURE = Path(__file__).resolve().parent / "data" / "holidays-GB-2026.json"
DE_FIXTURE = Path(__file__).resolve().parent / "data" / "holidays-DE-2026.json"


class NagerHolidayTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        dest = Path(self.tmp.name) / "GB-2026.json"
        dest.write_bytes(FIXTURE.read_bytes())
        dest_de = Path(self.tmp.name) / "DE-2026.json"
        dest_de.write_bytes(DE_FIXTURE.read_bytes())
        self._old = os.environ.get("TIMEWARP_HOLIDAY_DIR")
        os.environ["TIMEWARP_HOLIDAY_DIR"] = self.tmp.name

    def tearDown(self):
        if self._old is None:
            os.environ.pop("TIMEWARP_HOLIDAY_DIR", None)
        else:
            os.environ["TIMEWARP_HOLIDAY_DIR"] = self._old
        self.tmp.cleanup()

    def test_gb_england_defaults(self):
        rows, note = holidays_for_year(2026, "GB")
        self.assertIsNone(note)
        by_date = {d: n for d, n in rows}
        self.assertIn(date(2026, 12, 25), by_date)
        self.assertIn(date(2026, 12, 28), by_date)  # St. Stephen's (observed)
        self.assertIn(date(2026, 4, 6), by_date)  # Easter Monday (England)
        self.assertNotIn(date(2026, 1, 2), by_date)  # Scotland only

    def test_gb_scotland_region(self):
        rows, _ = holidays_for_year(2026, "GB", region="GB-SCT")
        dates = {d for d, _ in rows}
        self.assertIn(date(2026, 1, 2), dates)
        self.assertNotIn(date(2026, 4, 6), dates)  # Easter Monday not Scotland in fixture
        self.assertEqual(
            holidays_for_year(2026, "GB", region="SCT")[0],
            rows,
        )
        self.assertEqual(
            holidays_for_year(2026, "GB", region="Scotland")[0],
            rows,
        )

    def test_de_bavaria_aliases(self):
        nationwide, _ = holidays_for_year(2026, "DE")
        nat_dates = {d for d, _ in nationwide}
        self.assertNotIn(date(2026, 1, 6), nat_dates)  # Epiphany is regional
        by_code, _ = holidays_for_year(2026, "DE", region="DE-BY")
        by_dates = {d for d, _ in by_code}
        self.assertIn(date(2026, 1, 6), by_dates)
        self.assertIn(date(2026, 1, 1), by_dates)
        self.assertEqual(holidays_for_year(2026, "DE", region="BY")[0], by_code)
        self.assertEqual(holidays_for_year(2026, "DE", region="Bavaria")[0], by_code)
        be, _ = holidays_for_year(2026, "DE", region="Berlin")
        be_dates = {d for d, _ in be}
        self.assertNotIn(date(2026, 1, 6), be_dates)
        self.assertIn(date(2026, 3, 8), be_dates)

    def test_unknown_nager_region(self):
        from timewarp.errors import TimeWarpError

        with self.assertRaises(TimeWarpError) as ctx:
            holidays_for_year(2026, "DE", region="ZZ")
        self.assertIn("DE-BY", str(ctx.exception))

    def test_workdays_skip_boxing_day(self):
        # 2026-12-24 Thu, 25 Fri holiday, 26 Sat, 27 Sun, 28 Mon holiday, 29 Tue
        n = count_workdays(date(2026, 12, 24), date(2026, 12, 29), holiday_country="GB")
        # Thu work, Fri hol, Sat/Sun week, Mon hol → 1 workday in [24, 29)
        self.assertEqual(n.workdays, 1)
        self.assertGreaterEqual(n.holiday_days, 1)

    def test_us_federal_from_python_holidays(self):
        rows, note = holidays_for_year(2026, "US")
        self.assertIsNone(note)
        dates = {d for d, _ in rows}
        self.assertIn(date(2026, 7, 3), dates)  # Independence Day observed (Sat 4th → Fri 3rd)


    def test_us_state_overlay_california(self):
        fed, _ = holidays_for_year(2026, "US")
        fed_dates = {d for d, _ in fed}
        rows, _ = holidays_for_year(2026, "US", region="CA")
        by_date = {d: n for d, n in rows}
        self.assertIn(date(2026, 3, 31), by_date)
        self.assertIn("Chavez", by_date[date(2026, 3, 31)])
        self.assertNotIn(date(2026, 3, 31), fed_dates)
        rows_in, _ = holidays_for_year(2026, "US", region="Indiana")
        self.assertEqual(rows_in, holidays_for_year(2026, "US", region="US-IN")[0])

    def test_unknown_us_region(self):
        from timewarp.errors import TimeWarpError

        with self.assertRaises(TimeWarpError):
            holidays_for_year(2026, "US", region="ZZ")


class UsHolidayRegression(unittest.TestCase):
    def test_add_workdays_still_skips_us_federal(self):
        # Friday 2026-07-03 observed Independence Day
        d = add_workdays(date(2026, 7, 2), 1, holiday_country="US")
        self.assertEqual(d, date(2026, 7, 6))
