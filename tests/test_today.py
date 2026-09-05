import json
import unittest
from datetime import date
from pathlib import Path

from timewarp.places import lookup_place
from timewarp.today import format_quiet, snapshot
from tests.test_cli import run

ISS = Path(__file__).resolve().parent / "data" / "iss.tle"
INDIANAPOLIS = lookup_place("Indianapolis")


class TodaySnapshotTests(unittest.TestCase):
    def test_july_4_2026_holiday_and_rc(self):
        view = snapshot(date(2026, 7, 4), INDIANAPOLIS, tle_path=ISS)
        self.assertEqual(view.date, date(2026, 7, 4))
        self.assertEqual(view.weekday, "Saturday")
        self.assertIn("Independence", view.holiday or "")
        self.assertEqual(view.stamp, "3379.106")
        self.assertEqual(len(view.daily["letter"]), 1)
        self.assertIsNotNone(view.sun.sunrise)
        self.assertIsNotNone(view.moonrise or view.moonset)
        self.assertIsNone(view.eclipse)
        self.assertIsNone(view.season)
        quiet = format_quiet(view)
        self.assertIn("2026-07-04", quiet)
        self.assertIn("3379.106", quiet)
        self.assertIn(view.daily["letter"], quiet)

    def test_equinox_line(self):
        view = snapshot(date(2026, 3, 20), INDIANAPOLIS, tle_path=ISS)
        self.assertIsNotNone(view.season)
        self.assertIn("equinox", view.season.name.lower())

    def test_eclipse_line(self):
        view = snapshot(date(2026, 8, 12), INDIANAPOLIS, tle_path=ISS)
        self.assertIsNotNone(view.eclipse)
        self.assertEqual(view.eclipse.kind, "solar")


class TodayCliTests(unittest.TestCase):
    def test_today_indianapolis(self):
        code, out, err = run("today", "--city", "Indianapolis", "2026-07-04", "--tle", str(ISS))
        self.assertEqual(code, 0, err)
        self.assertIn("Saturday 2026-07-04", out)
        self.assertIn("Independence", out)
        self.assertIn("Sunrise:", out)
        self.assertIn("Civil dawn:", out)
        self.assertNotIn("Astronomical dusk", out)
        self.assertIn("Moonrise:", out)
        self.assertIn("Star Date:", out)
        self.assertIn("3379.106", out)
        self.assertIn("Note:", out)
        self.assertIn("Color:", out)
        self.assertNotIn("1690", out)
        self.assertNotIn("Lewis", out)

    def test_today_quiet(self):
        code, out, err = run("today", "-q", "--city", "Indianapolis", "2026-07-04", "--tle", str(ISS))
        self.assertEqual(code, 0, err)
        line = out.strip()
        self.assertIn("2026-07-04 Saturday", line)
        self.assertIn("3379.106", line)

    def test_today_json(self):
        code, out, err = run("today", "--json", "--city", "Indianapolis", "2026-07-04", "--tle", str(ISS))
        self.assertEqual(code, 0, err)
        payload = json.loads(out)
        self.assertEqual(payload["date"], "2026-07-04")
        self.assertIn("Independence", payload["holiday"] or "")
        self.assertEqual(payload["rc"]["stamp"], "3379.106")
        self.assertIn("sunrise", payload["sun"])
        self.assertNotIn("astronomical_dusk", payload["sun"])

    def test_today_needs_place(self):
        code, out, err = run("today", "2026-07-04")
        self.assertEqual(code, 2)
        self.assertIn("location required", err)

    def test_today_iss_fixture(self):
        code, out, err = run(
            "today",
            "--city",
            "New York",
            "2019-12-10",
            "--tle",
            str(ISS),
        )
        self.assertEqual(code, 0, err)
        self.assertIn("ISS", out)
        self.assertRegex(out, r"\d{2}:\d{2}[A-Z]")
