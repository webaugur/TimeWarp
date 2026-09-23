import unittest
from datetime import date, timezone

from timewarp.astro import moon_info
from timewarp.panchanga import compute_panchanga, format_quiet, tithi_from_elong
from tests.test_cli import run


class TithiMathTests(unittest.TestCase):
    def test_elong_zero_is_pratipada(self):
        n, frac, paksha, name = tithi_from_elong(0.0)
        self.assertEqual(n, 1)
        self.assertEqual(paksha, "Shukla")
        self.assertEqual(name, "Pratipada")
        self.assertAlmostEqual(frac, 0.0)

    def test_purnima_and_amavasya(self):
        n, _f, paksha, name = tithi_from_elong(179.0)
        self.assertEqual(n, 15)
        self.assertEqual(paksha, "Shukla")
        self.assertEqual(name, "Purnima")
        n, _f, paksha, name = tithi_from_elong(350.0)
        self.assertEqual(n, 30)
        self.assertEqual(paksha, "Krishna")
        self.assertEqual(name, "Amavasya")


class PanchangaInstantTests(unittest.TestCase):
    def test_near_new_moon(self):
        info = moon_info(date(2026, 7, 1))
        p = compute_panchanga(info.next_new)
        self.assertIn(p.tithi, (1, 30))
        self.assertTrue(p.nakshatra)
        self.assertTrue(p.masa)
        self.assertGreater(p.kali_year, 5000)

    def test_near_full_moon(self):
        info = moon_info(date(2026, 7, 1))
        p = compute_panchanga(info.next_full)
        self.assertEqual(p.tithi, 15)
        self.assertEqual(p.paksha, "Shukla")
        self.assertEqual(p.tithi_name, "Purnima")


class PanchangaCliTests(unittest.TestCase):
    def test_quiet(self):
        code, out, err = run("panchanga", "-q", "2026-07-04")
        self.assertEqual(code, 0, err)
        line = out.strip()
        self.assertGreater(len(line.split()), 3)
        self.assertTrue(any(x in line for x in ("Shukla", "Krishna")))

    def test_json_and_explain(self):
        code, out, err = run("panchanga", "--json", "--explain", "2026-07-04")
        self.assertEqual(code, 0, err)
        self.assertIn("tithi", out)
        self.assertIn("nakshatra", out)
        self.assertIn("yoga", out)
        self.assertIn("explain", out)
        self.assertIn("Schlyter", out)
