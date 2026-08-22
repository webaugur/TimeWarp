import unittest
from datetime import date

from timewarp.holidays import us_federal_holidays
from timewarp.workdays import add_workdays, count_workdays


class HolidayTests(unittest.TestCase):
    def test_2026_us_observed(self):
        names = {name: d for d, name in us_federal_holidays(2026)}
        self.assertEqual(names["New Year's Day"], date(2026, 1, 1))
        self.assertEqual(names["Martin Luther King Jr. Day"], date(2026, 1, 19))
        self.assertEqual(names["Presidents' Day"], date(2026, 2, 16))
        self.assertEqual(names["Memorial Day"], date(2026, 5, 25))
        self.assertEqual(names["Juneteenth National Independence Day"], date(2026, 6, 19))
        # 4 July 2026 is Saturday → observed Friday 3 July
        self.assertEqual(names["Independence Day"], date(2026, 7, 3))
        self.assertEqual(names["Labor Day"], date(2026, 9, 7))
        self.assertEqual(names["Columbus Day"], date(2026, 10, 12))
        self.assertEqual(names["Veterans Day"], date(2026, 11, 11))
        self.assertEqual(names["Thanksgiving Day"], date(2026, 11, 26))
        self.assertEqual(names["Christmas Day"], date(2026, 12, 25))


class WorkdayCountTests(unittest.TestCase):
    def test_mon_to_fri_exclusive(self):
        # 2026-07-06 Monday to 2026-07-10 Friday: [Mon, Fri) = 4
        s = count_workdays(date(2026, 7, 6), date(2026, 7, 10))
        self.assertEqual(s.workdays, 4)
        self.assertEqual(s.calendar_days, 4)

    def test_mon_to_fri_inclusive(self):
        s = count_workdays(date(2026, 7, 6), date(2026, 7, 10), include_end=True)
        self.assertEqual(s.workdays, 5)

    def test_weekend_span(self):
        s = count_workdays(date(2026, 7, 10), date(2026, 7, 13))  # Fri to Mon
        self.assertEqual(s.workdays, 1)  # Friday only in [Fri, Mon)

    def test_negative(self):
        forward = count_workdays(date(2026, 7, 6), date(2026, 7, 10))
        back = count_workdays(date(2026, 7, 10), date(2026, 7, 6))
        self.assertEqual(back.workdays, -forward.workdays)

    def test_same_day(self):
        self.assertEqual(count_workdays(date(2026, 7, 6), date(2026, 7, 6)).workdays, 0)
        self.assertEqual(
            count_workdays(date(2026, 7, 6), date(2026, 7, 6), include_end=True).workdays, 1
        )


class AddWorkdayTests(unittest.TestCase):
    def test_friday_plus_one(self):
        # 2026-07-03 Friday + 1 workday = Monday 6
        self.assertEqual(add_workdays(date(2026, 7, 3), 1), date(2026, 7, 6))

    def test_plus_ten(self):
        d = add_workdays(date(2026, 7, 6), 10)
        self.assertEqual(d.weekday(), 0)  # a Monday: 6 Jul + 10 weekdays = 20 Jul
        self.assertEqual(d, date(2026, 7, 20))

    def test_negative(self):
        self.assertEqual(add_workdays(date(2026, 7, 6), -1), date(2026, 7, 3))

    def test_skip_us_holiday(self):
        # 2026-07-02 Thursday + 1 workday, skipping Independence Day observed Jul 3
        # without holidays: Friday Jul 3
        self.assertEqual(add_workdays(date(2026, 7, 2), 1), date(2026, 7, 3))
        self.assertEqual(
            add_workdays(date(2026, 7, 2), 1, holiday_country="US"),
            date(2026, 7, 6),
        )

    def test_round_trip(self):
        start = date(2026, 3, 2)
        for n in range(-20, 21):
            end = add_workdays(start, n)
            if n == 0:
                self.assertEqual(end, start)
                continue
            # exclusive count from start to end should equal n for workdays
            counted = count_workdays(start, end).workdays
            self.assertEqual(counted, n, msg=f"n={n} end={end} counted={counted}")


if __name__ == "__main__":
    unittest.main()
