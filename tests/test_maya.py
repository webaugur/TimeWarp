import unittest
from datetime import date

from timewarp.cherokee import cherokee_month
from timewarp.maya import maya_from_gregorian
from tests.test_cli import run


class MayaTests(unittest.TestCase):
    def test_2012_baktun(self):
        maya = maya_from_gregorian(date(2012, 12, 21))
        self.assertEqual(maya.long_count(), "13.0.0.0.0")
        self.assertEqual(maya.tzolkin_number, 4)
        self.assertEqual(maya.tzolkin_name, "Ajaw")
        self.assertEqual((maya.haab_day, maya.haab_month), (3, "Kankin"))

    def test_cli(self):
        code, out, err = run("maya", "-q", "2012-12-21")
        self.assertEqual(code, 0, err)
        self.assertIn("13.0.0.0.0", out)
        self.assertIn("4 Ajaw", out)


class CherokeeTests(unittest.TestCase):
    def test_september_syllabary_without_fake_gloss(self):
        gloss, name, syll = cherokee_month(date(2026, 9, 22))
        self.assertEqual(gloss, "")
        self.assertEqual(name, "dulisdi")
        self.assertEqual(syll, "ᏚᎵᏍᏗ")

    def test_july_has_gloss_and_syllabary(self):
        gloss, name, syll = cherokee_month(date(2026, 7, 4))
        self.assertEqual(gloss, "ripe-corn moon")
        self.assertEqual(name, "kuyegwona")
        self.assertEqual(syll, "ᎫᏰᏉᎾ")
