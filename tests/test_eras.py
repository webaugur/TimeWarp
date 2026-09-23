import unittest
from datetime import date

from timewarp.eras import (
    coptic_from_gregorian,
    egyptian_from_gregorian,
    french_from_gregorian,
    hebrew_from_gregorian,
    hijri_from_gregorian,
    julian_from_gregorian,
)
from tests.test_cli import run


class EraMathTests(unittest.TestCase):
    def test_hebrew_2026_09_22(self):
        year, month, day, name = hebrew_from_gregorian(date(2026, 9, 22))
        self.assertEqual((year, month, day, name), (5787, 7, 11, "Tishri"))

    def test_rosh_hashanah_5787(self):
        year, month, day, _name = hebrew_from_gregorian(date(2026, 9, 12))
        self.assertEqual((year, month, day), (5787, 7, 1))

    def test_coptic_after_new_year(self):
        year, month, day, name = coptic_from_gregorian(date(2026, 9, 22))
        self.assertEqual((year, month, day, name), (1743, 1, 12, "Thout"))

    def test_julian_2026_09_22(self):
        self.assertEqual(julian_from_gregorian(date(2026, 9, 22)), (2026, 9, 9))

    def test_french_epoch_and_nivose(self):
        self.assertEqual(french_from_gregorian(date(1792, 9, 22))[:3], (1, 1, 1))
        self.assertEqual(french_from_gregorian(date(1792, 9, 22))[3], "Vendémiaire")
        year, month, day, name = french_from_gregorian(date(1793, 1, 1))
        self.assertEqual((year, month, day, name), (1, 4, 12, "Nivôse"))

    def test_egyptian_epoch_and_modern(self):
        # 26 Feb 747 BCE Julian is not a Python date; day-count 0 is 1 Thoth.
        from timewarp.eras import _NABONASSAR_JDN, gregorian_jdn

        self.assertEqual(gregorian_jdn(2026, 9, 22) - _NABONASSAR_JDN >= 0, True)
        year, month, day, name = egyptian_from_gregorian(date(2026, 9, 22))
        self.assertGreater(year, 2000)
        self.assertIn(name, ("Thoth", "Phaophi", "Athyr", "Choiak", "Tybi", "Mechir", "Phamenoth", "Pharmuthi", "Pachon", "Payni", "Epiphi", "Mesore", "Epagomenal"))
        self.assertGreaterEqual(day, 1)

    def test_hijri_is_positive(self):
        year, month, day, name = hijri_from_gregorian(date(2026, 9, 22))
        self.assertGreater(year, 1400)
        self.assertTrue(name)
        self.assertGreaterEqual(day, 1)


class ErasCliTests(unittest.TestCase):
    def test_quiet_and_human(self):
        code, out, err = run("eras", "-q", "2026-09-22")
        self.assertEqual(code, 0, err)
        self.assertIn("2026-09-22", out)
        self.assertIn("AL 6026", out)
        self.assertIn("AM 5787", out)
        self.assertIn("Tishri", out)
        code, out, err = run("eras", "--no-color", "2026-09-22")
        self.assertEqual(code, 0, err)
        self.assertIn("Anno Lucis:", out)
        self.assertIn("Coptic:", out)
        self.assertIn("Panchanga:", out)
        self.assertIn("Thout", out)
        self.assertIn("Julian:", out)
        self.assertIn("2026-09-09", out)
        self.assertIn("French:", out)
        self.assertIn("an 234", out)
        self.assertIn("Egyptian:", out)
        self.assertIn("yoga", out)
        self.assertIn("Liturgy:", out)
        self.assertIn("Roman:", out)
        self.assertIn("a.d. X Kal. Oct.", out)
        self.assertIn("Maya:", out)
        self.assertIn("Cherokee:", out)
        self.assertIn("ᏚᎵᏍᏗ", out)
