import io
import json
import unittest
from contextlib import redirect_stderr, redirect_stdout
from datetime import date

from timewarp.cli import main


def run(*argv: str) -> tuple[int, str, str]:
    out = io.StringIO()
    err = io.StringIO()
    with redirect_stdout(out), redirect_stderr(err):
        code = main(list(argv))
    return code, out.getvalue(), err.getvalue()


class CliPhase1Tests(unittest.TestCase):
    def test_add_example(self):
        code, out, err = run("add", "-q", "2026-07-04", "7", "months", "6", "days")
        self.assertEqual(code, 0, err)
        self.assertEqual(out.strip(), "2027-02-10")

    def test_add_iso_duration(self):
        code, out, _ = run("add", "-q", "2026-07-04", "P7M6D")
        self.assertEqual(code, 0)
        self.assertEqual(out.strip(), "2027-02-10")

    def test_between_negative(self):
        code, out, err = run("between", "-q", "2026-05-31", "2025-04-30")
        self.assertEqual(code, 0, err)
        self.assertEqual(out.strip(), "-P1Y1M")

    def test_between_json_negative(self):
        code, out, err = run("count", "--json", "2026-05-31", "2025-04-30")
        self.assertEqual(code, 0, err)
        payload = json.loads(out)
        self.assertEqual(payload["sign"], -1)
        self.assertEqual(payload["total_days"], -396)
        self.assertEqual(payload["iso8601"], "-P1Y1M")

    def test_invalid_april_31(self):
        code, _, err = run("between", "2026-05-31", "2025-04-31")
        self.assertEqual(code, 2)
        self.assertIn("April 2025 has 30 days", err)

    def test_weekday(self):
        code, out, _ = run("weekday", "-q", "2026-07-04")
        self.assertEqual(code, 0)
        self.assertEqual(out.strip(), "2026-07-04 Saturday")

    def test_week(self):
        code, out, _ = run("week", "-q", "2026-07-04")
        self.assertEqual(code, 0)
        self.assertEqual(out.strip(), "2026-W27-6")

    def test_add_workdays(self):
        code, out, _ = run("add-workdays", "-q", "2026-07-03", "1")
        self.assertEqual(code, 0)
        self.assertEqual(out.strip(), "2026-07-06")

    def test_sub(self):
        code, out, _ = run("sub", "-q", "2027-02-10", "7", "months", "6", "days")
        self.assertEqual(code, 0)
        self.assertEqual(out.strip(), "2026-07-04")


class CliPhase2Tests(unittest.TestCase):
    def test_calendar_2026(self):
        code, out, err = run("calendar", "2026")
        self.assertEqual(code, 0, err)
        self.assertIn("Calendar 2026", out)
        self.assertIn("2026-01-19", out)
        self.assertIn("Martin Luther King Jr. Day", out)

    def test_eclipse_2026(self):
        code, out, err = run("eclipse", "2026")
        self.assertEqual(code, 0, err)
        self.assertIn("2026-08-12", out)
        self.assertIn("solar", out)
        self.assertIn("2026-08-27/2026-08-28", out)
        self.assertIn("lunar", out)

    def test_moon(self):
        code, out, err = run("moon", "2026-08-28")
        self.assertEqual(code, 0, err)
        self.assertIn("Phase:", out)

    def test_sun_city(self):
        code, out, err = run("sun", "--city", "New York", "2026-07-04")
        self.assertEqual(code, 0, err)
        self.assertIn("Sunrise:", out)
        self.assertIn("Sunset:", out)

    def test_moonrise(self):
        code, out, err = run("moonrise", "--city", "New York", "2026-07-04")
        self.assertEqual(code, 0, err)
        self.assertIn("Body:  moon", out)
        self.assertIn("Rise:", out)

    def test_rise_venus_json(self):
        code, out, err = run("rise", "venus", "--city", "London", "--json", "2026-07-04")
        self.assertEqual(code, 0, err)
        payload = json.loads(out)
        self.assertEqual(payload["body"], "venus")
        self.assertEqual(payload["date"], "2026-07-04")

    def test_rise_all_date_after_flags(self):
        code, out, err = run("rise", "--all", "--city", "London", "-q", "2026-07-04")
        self.assertEqual(code, 0, err)
        self.assertIn("moon", out)
        self.assertIn("venus", out)
        self.assertIn("jupiter", out)

    def test_countdown_json(self):
        code, out, err = run("countdown", "--json", "2099-01-01")
        self.assertEqual(code, 0, err)
        payload = json.loads(out)
        self.assertGreaterEqual(payload["sign"], 0)
        self.assertTrue(payload["iso8601"].startswith("P"))


class TodayKeyword(unittest.TestCase):
    def test_weekday_today(self):
        code, out, err = run("weekday", "today")
        self.assertEqual(code, 0, err)
        self.assertIn(date.today().isoformat(), out)


if __name__ == "__main__":
    unittest.main()
