import json
import unittest
from datetime import datetime
from zoneinfo import ZoneInfo

from timewarp.chart import (
    apply_frame,
    ayanamsa_deg,
    compute_chart,
    equal_cusps,
    explain,
    hermetic_lots,
    lon_to_sign,
    rev,
    wrap180,
)
from timewarp.ephem import julian_day
from timewarp.places import lookup_place
from tests.test_cli import run

INDIANAPOLIS = lookup_place("Indianapolis")
NOON = datetime(2026, 7, 4, 12, 0, tzinfo=ZoneInfo("America/Indiana/Indianapolis"))


class SignTests(unittest.TestCase):
    def test_zero_is_aries(self):
        sign, idx, deg = lon_to_sign(0.0)
        self.assertEqual(sign, "Aries")
        self.assertEqual(idx, 0)
        self.assertAlmostEqual(deg, 0.0, places=6)

    def test_cancer(self):
        sign, idx, deg = lon_to_sign(100.5)
        self.assertEqual(sign, "Cancer")
        self.assertEqual(idx, 3)
        self.assertAlmostEqual(deg, 10.5, places=6)

    def test_opposition_wrap(self):
        self.assertAlmostEqual(abs(wrap180(10 - 190)), 180.0, places=5)


class AngleTests(unittest.TestCase):
    def test_dsc_ic_opposites(self):
        chart = compute_chart(NOON, INDIANAPOLIS)
        self.assertAlmostEqual(
            rev(chart.angles["dsc"].lon - chart.angles["asc"].lon), 180.0, places=4
        )
        self.assertAlmostEqual(
            rev(chart.angles["ic"].lon - chart.angles["mc"].lon), 180.0, places=4
        )

    def test_placidus_cusp1_is_asc(self):
        chart = compute_chart(NOON, INDIANAPOLIS, houses="placidus")
        self.assertEqual(chart.house_system, "placidus")
        self.assertAlmostEqual(chart.cusps[1], chart.angles["asc"].lon, places=4)
        self.assertAlmostEqual(chart.cusps[10], chart.angles["mc"].lon, places=4)
        self.assertEqual(chart.angles["mc"].house, 10)
        self.assertEqual(chart.bodies["sun"].house, 10)
        houses = {pos.house for pos in chart.bodies.values()}
        self.assertGreater(len(houses), 2)

    def test_polar_falls_back_to_equal(self):
        from timewarp.places import Place

        polar = Place("Alert", 82.5, -62.3, "America/Toronto")
        chart = compute_chart(NOON.replace(tzinfo=ZoneInfo("America/Toronto")), polar)
        self.assertEqual(chart.house_system, "equal")
        self.assertIsNotNone(chart.house_note)

    def test_equal_cusps_step_30(self):
        cusps = equal_cusps(15.0)
        self.assertAlmostEqual(rev(cusps[2] - cusps[1]), 30.0, places=6)


class LotTests(unittest.TestCase):
    def test_fortune_day_vs_night(self):
        bodies = {
            "sun": 10.0,
            "moon": 40.0,
            "mercury": 20.0,
            "venus": 50.0,
            "mars": 80.0,
            "jupiter": 100.0,
            "saturn": 200.0,
        }
        day = hermetic_lots(0.0, bodies, day=True)
        night = hermetic_lots(0.0, bodies, day=False)
        self.assertAlmostEqual(day["fortune"], 30.0, places=6)
        self.assertAlmostEqual(night["fortune"], 330.0, places=6)
        self.assertAlmostEqual(day["spirit"], 330.0, places=6)


class SiderealTests(unittest.TestCase):
    def test_sidereal_subtracts_ayanamsa(self):
        trop = compute_chart(NOON, INDIANAPOLIS)
        sid = compute_chart(NOON, INDIANAPOLIS, sidereal="lahiri")
        ay = ayanamsa_deg(julian_day(NOON), "lahiri")
        self.assertGreater(ay, 20.0)
        self.assertAlmostEqual(
            apply_frame(trop.bodies["sun"].lon, ay), sid.bodies["sun"].lon, places=3
        )


class ExplainTests(unittest.TestCase):
    def test_geometry_not_delineation(self):
        chart = compute_chart(NOON, INDIANAPOLIS)
        text = " ".join(explain(chart)).lower()
        self.assertIn("moon", text)
        self.assertIn("sun", text)
        self.assertNotIn("marry", text)
        self.assertNotIn("lucky", text)
        self.assertNotIn("destiny", text)


class AstroCliTests(unittest.TestCase):
    def test_quiet(self):
        code, out, err = run("astro", "-q", "--city", "Indianapolis", "2026-07-04")
        self.assertEqual(code, 0, err)
        self.assertIn("tropical", out)
        self.assertIn("ASC", out)
        self.assertIn("Sun", out)
        self.assertIn("Moon", out)

    def test_explain_and_json(self):
        code, out, err = run(
            "astro", "--json", "--explain", "--city", "Indianapolis", "2026-07-04"
        )
        self.assertEqual(code, 0, err)
        payload = json.loads(out)
        self.assertEqual(payload["frame"], "tropical")
        self.assertIn("asc", payload["angles"])
        self.assertIn("sun", payload["bodies"])
        self.assertIn("fortune", payload["lots"])
        self.assertIn("explain", payload)
        self.assertTrue(any("Sun in" in line for line in payload["explain"]))

    def test_needs_place(self):
        code, _, err = run("astro", "2026-07-04")
        self.assertEqual(code, 2)
        self.assertIn("location required", err)
