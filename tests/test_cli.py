import io
import json
import os
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from datetime import date
from pathlib import Path

from timewarp.cli import main

# Do not pick up the developer's ~/.config/timewarp/cache.json.
_TEST_CACHE = Path(tempfile.gettempdir()) / "timewarp-tests-cache.json"
_TEST_CACHE.unlink(missing_ok=True)
os.environ["TIMEWARP_CACHE"] = str(_TEST_CACHE)

# Named asteroid/comet elements from fixtures, not a live JPL fetch.
_TEST_SBDB = Path(tempfile.gettempdir()) / "timewarp-tests-sbdb"
_TEST_SBDB.mkdir(parents=True, exist_ok=True)
_CERES_FIXTURE = Path(__file__).resolve().parent / "data" / "sbdb-ceres.json"
if _CERES_FIXTURE.is_file():
    (_TEST_SBDB / "ceres.json").write_bytes(_CERES_FIXTURE.read_bytes())
    (_TEST_SBDB / "433.json").write_bytes(_CERES_FIXTURE.read_bytes())
os.environ["TIMEWARP_SBDB_DIR"] = str(_TEST_SBDB)

_TEST_TLE = Path(tempfile.gettempdir()) / "timewarp-tests-tle"
_TEST_TLE.mkdir(parents=True, exist_ok=True)
_SATCAT = Path(__file__).resolve().parent / "data" / "satcat-iss.csv"
if _SATCAT.is_file():
    (_TEST_TLE / "satcat.csv").write_bytes(_SATCAT.read_bytes())
os.environ["TIMEWARP_TLE_DIR"] = str(_TEST_TLE)

_TEST_HORIZONS = Path(tempfile.gettempdir()) / "timewarp-tests-horizons"
_TEST_HORIZONS.mkdir(parents=True, exist_ok=True)
os.environ["TIMEWARP_HORIZONS_DIR"] = str(_TEST_HORIZONS)


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

    def test_add_offset_assumes_today(self):
        from timewarp.duration import apply_offset, parse_offset
        from timewarp.iso import format_instant

        today = date.today().isoformat()
        code, out, err = run("add", "-q", "P7M6D")
        self.assertEqual(code, 0, err)
        expected = apply_offset(date.today(), parse_offset("P7M6D"))
        self.assertEqual(out.strip(), format_instant(expected))
        self.assertIn(today, err)
        code, out, err = run("add", "-q", "7", "years", "6", "months")
        self.assertEqual(code, 0, err)
        expected = apply_offset(date.today(), parse_offset("7 years 6 months"))
        self.assertEqual(out.strip(), format_instant(expected))
        self.assertIn(today, err)
        code, out, err = run("add", "-q", "7-6-13")
        self.assertEqual(code, 0, err)
        expected = apply_offset(date.today(), parse_offset("7-6-13"))
        self.assertEqual(out.strip(), format_instant(expected))

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

    def test_holidays_gb_from_cache(self):
        src = Path(__file__).resolve().parent / "data" / "holidays-GB-2026.json"
        tmp = tempfile.TemporaryDirectory()
        try:
            dest = Path(tmp.name) / "GB-2026.json"
            dest.write_bytes(src.read_bytes())
            old = os.environ.get("TIMEWARP_HOLIDAY_DIR")
            os.environ["TIMEWARP_HOLIDAY_DIR"] = tmp.name
            code, out, err = run("holidays", "2026", "--country", "GB")
            self.assertEqual(code, 0, err)
            self.assertIn("Christmas Day", out)
            self.assertIn("2026-12-28", out)
        finally:
            if old is None:
                os.environ.pop("TIMEWARP_HOLIDAY_DIR", None)
            else:
                os.environ["TIMEWARP_HOLIDAY_DIR"] = old
            tmp.cleanup()

    def test_add_workdays(self):
        code, out, _ = run("add-workdays", "-q", "2026-07-03", "1")
        self.assertEqual(code, 0)
        self.assertEqual(out.strip(), "2026-07-06")

    def test_sub(self):
        code, out, _ = run("sub", "-q", "2027-02-10", "7", "months", "6", "days")
        self.assertEqual(code, 0)
        self.assertEqual(out.strip(), "2026-07-04")


class CliPhase2Tests(unittest.TestCase):
    def test_month_sheet_july_2026(self):
        code, out, err = run("month", "2026-07", "--city", "Indianapolis")
        self.assertEqual(code, 0, err)
        self.assertIn("2026-07-01", out)
        self.assertIn("2026-07-31", out)
        self.assertIn("dawn", out)
        self.assertIn("moon↑", out)
        self.assertRegex(out, r"2026-07-04\s+\w{2}\s+\d{2}:\d{2}Q")
        self.assertNotIn("adawn", out)
        code, out, err = run("month", "2026-07", "--city", "Indianapolis", "--twilight")
        self.assertEqual(code, 0, err)
        self.assertIn("adawn", out)

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
        self.assertIn("1900", out)

    def test_eclipse_1919(self):
        code, out, err = run("eclipse", "1919")
        self.assertEqual(code, 0, err)
        self.assertIn("solar", out)

    def test_rise_ceres(self):
        code, out, err = run("rise", "ceres", "--city", "London", "2026-07-04")
        self.assertEqual(code, 0, err)
        self.assertIn("ceres", out)

    def test_rise_sbdb_number(self):
        code, out, err = run("rise", "433", "--city", "London", "2026-07-04")
        self.assertEqual(code, 0, err)
        self.assertIn("433", out)

    def test_moon(self):
        code, out, err = run("moon", "2026-08-28")
        self.assertEqual(code, 0, err)
        self.assertIn("Phase:", out)
        self.assertIn("Next full:", out)
        self.assertRegex(out, r"Next full:\s+\d{2}:\d{2}[A-Z]")

    def test_seasons_2026(self):
        code, out, err = run("seasons", "2026")
        self.assertEqual(code, 0, err)
        self.assertIn("March equinox", out)
        self.assertIn("December solstice", out)

    def test_passes_iss_fixture(self):
        tle = Path(__file__).resolve().parent / "data" / "iss.tle"
        code, out, err = run(
            "passes",
            "ISS",
            "--city",
            "New York",
            "--tle",
            str(tle),
            "2019-12-10",
        )
        self.assertEqual(code, 0, err)
        self.assertIn("ISS", out)
        self.assertIn("sky", out)
        self.assertRegex(out, r"\d{2}:\d{2}[A-Z]")

    def test_global_color_before_subcommand(self):
        code, out, err = run("--color", "sun", "--city", "New York", "2026-07-04")
        self.assertEqual(code, 0, err)
        self.assertIn("Sunrise:", out)

    def test_sun_city(self):
        code, out, err = run("sun", "--city", "New York", "2026-07-04")
        self.assertEqual(code, 0, err)
        self.assertIn("Sunrise:", out)
        self.assertIn("Sunset:", out)
        self.assertRegex(out, r"Sunrise:\s+\d{2}:\d{2}Q")
        self.assertRegex(out, r"Sunset:\s+\d{2}:\d{2}Q")
        self.assertRegex(out, r"Solar noon:\s+\d{2}:\d{2}Q")
        self.assertIn("Civil dawn", out)
        self.assertIn("Astronomical dusk", out)
        self.assertRegex(out, r"Sunrise:.*NE")
        self.assertRegex(out, r"Sunset:.*NW")
        self.assertNotIn("2026-07-04T", out)

    def test_moonrise(self):
        code, out, err = run("moonrise", "--city", "New York", "2026-07-04")
        self.assertEqual(code, 0, err)
        self.assertIn("Body:  ☾ moon", out)
        self.assertIn("Rise:", out)

    def test_rise_venus_json(self):
        code, out, err = run("rise", "venus", "--city", "London", "--json", "2026-07-04")
        self.assertEqual(code, 0, err)
        payload = json.loads(out)
        self.assertEqual(payload["body"], "venus")
        self.assertEqual(payload["symbol"], "♀")
        self.assertEqual(payload["date"], "2026-07-04")

    def test_rise_sorted_by_time(self):
        code, out, err = run("rise", "--city", "New York", "-q", "2026-07-04")
        self.assertEqual(code, 0, err)
        stamps = []
        for line in out.splitlines():
            iso = line.split()[-1]
            if iso != "none":
                stamps.append(iso)
        self.assertEqual(stamps, sorted(stamps))
        self.assertGreaterEqual(len(stamps), 2)

    def test_rise_13_33_columns(self):
        code, out, err = run(
            "rise", "sun", "--city", "Indianapolis", "--13", "--33", "2026-07-04"
        )
        self.assertEqual(code, 0, err)
        self.assertIn("+13°", out)
        self.assertIn("+33°", out)
        self.assertIn("13°", out)
        self.assertIn("33°", out)
        code, out, err = run("rise", "sun", "--city", "Indianapolis", "--13", "2026-07-04")
        self.assertEqual(code, 0, err)
        self.assertIn("+13°", out)
        self.assertNotIn("+33°", out)
        code, out, err = run("set", "--city", "Indianapolis", "--13", "--33", "2026-07-04")
        self.assertEqual(code, 0, err)
        header = next(line for line in out.splitlines() if "set" in line and "rise" in line)
        self.assertLess(header.find("set"), header.find("13°"))
        self.assertLess(header.find("13°"), header.find("33°"))
        self.assertLess(header.find("33°"), header.find("rise"))

    def test_rise_clock_uses_zone_letter(self):
        code, out, err = run("rise", "sun", "--city", "Indianapolis", "2026-07-04")
        self.assertEqual(code, 0, err)
        self.assertRegex(out, r"\b\d{2}:\d{2}Q\b")
        self.assertNotIn("2026-07-04T", out)
        code, out, err = run("rise", "sun", "--city", "Indianapolis", "2026-12-21")
        self.assertEqual(code, 0, err)
        self.assertRegex(out, r"\b\d{2}:\d{2}R\b")
        code, out, err = run(
            "rise", "sun", "--lat", "0", "--lon", "0", "--tz", "UTC", "2026-07-04"
        )
        self.assertEqual(code, 0, err)
        self.assertRegex(out, r"\b\d{2}:\d{2}Z\b")

    def test_assumed_today_is_yellow_on_cli(self):
        today = date.today().isoformat()
        code, out, err = run("rise", "sun", "--city", "Indianapolis", "--color")
        self.assertEqual(code, 0, err)
        self.assertIn(today, err)
        self.assertIn("\033[38;2;255;220;0m", err)
        self.assertNotIn(today + "T", out)

    def test_rise_defaults_to_visible_bodies(self):
        code, out, err = run("rise", "--city", "New York", "-q", "2026-07-04")
        self.assertEqual(code, 0, err)
        self.assertIn("sun", out)
        self.assertIn("moon", out)
        self.assertIn("venus", out)
        self.assertIn("jupiter", out)

    def test_set_lists_set_times(self):
        code, out, err = run("set", "--city", "New York", "-q", "2026-07-04")
        self.assertEqual(code, 0, err)
        self.assertIn("sun", out)
        self.assertIn("moon", out)
        # New York 2026-07-04 moonset ~09:53
        self.assertRegex(out, r"moon\s+2026-07-04T09:53:")

    def test_moonset_alias(self):
        code, out, err = run("moonset", "--city", "New York", "2026-07-04")
        self.assertEqual(code, 0, err)
        self.assertIn("Body:  ☾ moon", out)
        self.assertIn("Set:", out)

    def test_rise_period(self):
        code, out, err = run(
            "rise", "--city", "New York", "-q", "2026-07-04", "2026-07-05"
        )
        self.assertEqual(code, 0, err)
        self.assertIn("2026-07-04", out)
        self.assertIn("2026-07-05", out)

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


class CacheTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.cache = Path(self._tmp.name) / "cache.json"
        self._old = os.environ.get("TIMEWARP_CACHE")
        os.environ["TIMEWARP_CACHE"] = str(self.cache)

    def tearDown(self):
        if self._old is None:
            os.environ.pop("TIMEWARP_CACHE", None)
        else:
            os.environ["TIMEWARP_CACHE"] = self._old
        self._tmp.cleanup()

    def test_save_and_reuse_city(self):
        code, out, err = run("save", "--city", "Indianapolis")
        self.assertEqual(code, 0, err)
        self.assertIn("timewarp --city Indianapolis", out)
        self.assertTrue(self.cache.is_file())
        code, out, err = run("rise", "-q", "sun", "2026-07-04")
        self.assertEqual(code, 0, err)
        self.assertIn("timewarp --city Indianapolis rise -q sun 2026-07-04", err)
        self.assertRegex(out, r"2026-07-04T")

    def test_load_scriptable(self):
        run("save", "--city", "Indianapolis")
        code, out, err = run("load")
        self.assertEqual(code, 0, err)
        self.assertEqual(out.strip(), "timewarp --city Indianapolis")
        code, out, err = run("load", "-q")
        self.assertEqual(code, 0, err)
        self.assertEqual(out.strip(), "--city Indianapolis")
        code, out, err = run("cache", "load")
        self.assertEqual(code, 0, err)
        self.assertEqual(out.strip(), "timewarp --city Indianapolis")

    def test_unload_city_only(self):
        run("save", "--city", "Indianapolis")
        code, _, err = run("unload", "--city")
        self.assertEqual(code, 0, err)
        self.assertIn("unloaded --city", err)
        code, _, err = run("rise", "-q", "sun", "2026-07-04")
        self.assertEqual(code, 2)
        self.assertIn("location required", err)

    def test_unload_all(self):
        run("save", "--city", "San Jose")
        code, _, err = run("unload")
        self.assertEqual(code, 0, err)
        self.assertIn("cache unloaded", err)
        self.assertFalse(self.cache.is_file())


class TodayKeyword(unittest.TestCase):
    def test_weekday_today(self):
        code, out, err = run("weekday", "today")
        self.assertEqual(code, 0, err)
        self.assertIn(date.today().isoformat(), out)


class HelpAndErrorTests(unittest.TestCase):
    def test_help_command(self):
        code, out, err = run("help")
        self.assertEqual(code, 0, err)
        self.assertIn("timewarp add", out)
        self.assertIn("Command help:", out)

    def test_help_topic(self):
        code, out, err = run("help", "add")
        self.assertEqual(code, 0, err)
        self.assertIn("usage:", out)
        self.assertIn("offset", out)

    def test_long_help_flag(self):
        code, out, err = run("--help")
        self.assertEqual(code, 0, err)
        self.assertIn("usage:", out)
        self.assertIn("help", out)

    def test_no_args_shows_help(self):
        code, out, err = run()
        self.assertEqual(code, 0, err)
        self.assertIn("Command help:", out)

    def test_unknown_command(self):
        code, out, err = run("frobnicate")
        self.assertEqual(code, 2)
        self.assertIn("timewarp:", err)
        self.assertNotIn("Traceback", err)

    def test_month_bad_value(self):
        code, _, err = run("month", "2026-13", "--city", "Indianapolis")
        self.assertEqual(code, 2)
        self.assertIn("month", err.lower())

    def test_help_unknown_topic(self):
        code, _, err = run("help", "frobnicate")
        self.assertEqual(code, 2)
        self.assertIn("no help for", err)

    def test_unknown_city(self):
        code, _, err = run("rise", "--city", "NotARealCity")
        self.assertEqual(code, 2)
        self.assertIn("unknown city", err)

    def test_bad_latitude(self):
        code, _, err = run("sun", "--lat", "99", "--lon", "0")
        self.assertEqual(code, 2)
        self.assertIn("latitude", err)


if __name__ == "__main__":
    unittest.main()
