import json
import os
import tempfile
import unittest
from datetime import datetime, timezone
from pathlib import Path
from unittest.mock import patch

from timewarp.errors import TimeWarpError
from timewarp.horizons import (
    elements_to_payload,
    load_moon_elements,
    parse_horizons_elements,
    payload_to_elements,
)

DATA = Path(__file__).resolve().parent / "data"
RAW = DATA / "horizons-io.txt"


class ParseHorizonsTests(unittest.TestCase):
    def test_io_elements(self):
        el = parse_horizons_elements(RAW.read_text(encoding="utf-8"), name="io")
        self.assertEqual(el.name, "io")
        self.assertAlmostEqual(el.a, 2.821089906157367e-3, places=12)
        self.assertAlmostEqual(el.e, 3.987940112020895e-3, places=12)
        self.assertAlmostEqual(el.i, 2.206524368851387, places=9)
        self.assertAlmostEqual(el.N, 338.4807022039085, places=8)
        self.assertAlmostEqual(el.w, 255.0813941291826, places=8)
        self.assertAlmostEqual(el.M0, 97.50892856938526, places=8)
        self.assertAlmostEqual(el.n, 203.2349080207157, places=8)
        self.assertAlmostEqual(el.epoch_jd, 2461041.5, places=4)
        self.assertAlmostEqual(el.d_epoch, 2461041.5 - 2451543.5, places=4)

    def test_roundtrip_payload(self):
        el = parse_horizons_elements(RAW.read_text(encoding="utf-8"), name="io")
        again = payload_to_elements(elements_to_payload(el), name="io")
        self.assertAlmostEqual(again.a, el.a, places=12)
        self.assertAlmostEqual(again.M0, el.M0, places=12)

    def test_empty_table(self):
        with self.assertRaises(TimeWarpError):
            parse_horizons_elements("no table here", name="io")


class LoadMoonTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self._old = os.environ.get("TIMEWARP_HORIZONS_DIR")
        os.environ["TIMEWARP_HORIZONS_DIR"] = self.tmp.name

    def tearDown(self):
        if self._old is None:
            os.environ.pop("TIMEWARP_HORIZONS_DIR", None)
        else:
            os.environ["TIMEWARP_HORIZONS_DIR"] = self._old
        self.tmp.cleanup()

    def test_cache_hit(self):
        el = parse_horizons_elements(RAW.read_text(encoding="utf-8"), name="io")
        dest = Path(self.tmp.name) / "io.json"
        dest.write_text(json.dumps(elements_to_payload(el), indent=2) + "\n", encoding="utf-8")
        with patch("timewarp.horizons.fetch_horizons", side_effect=AssertionError("network")):
            got = load_moon_elements("io")
        self.assertIsNotNone(got)
        self.assertAlmostEqual(got.a, el.a, places=12)

    def test_offline_none(self):
        with patch("timewarp.horizons.fetch_horizons", side_effect=TimeWarpError("offline")):
            self.assertIsNone(load_moon_elements("io"))

    def test_unknown_name(self):
        self.assertIsNone(load_moon_elements("ceres"))


class EphemMoonTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self._old = os.environ.get("TIMEWARP_HORIZONS_DIR")
        os.environ["TIMEWARP_HORIZONS_DIR"] = self.tmp.name
        el = parse_horizons_elements(RAW.read_text(encoding="utf-8"), name="io")
        (Path(self.tmp.name) / "io.json").write_text(
            json.dumps(elements_to_payload(el), indent=2) + "\n", encoding="utf-8"
        )

    def tearDown(self):
        if self._old is None:
            os.environ.pop("TIMEWARP_HORIZONS_DIR", None)
        else:
            os.environ["TIMEWARP_HORIZONS_DIR"] = self._old
        self.tmp.cleanup()

    def test_io_position_near_jupiter(self):
        from timewarp.ephem import position

        p = position("io", datetime(2026, 1, 1, tzinfo=timezone.utc))
        self.assertEqual(p.body, "io")
        self.assertGreater(p.heliocentric_au, 4.0)
        self.assertLess(p.heliocentric_au, 6.5)
        j = position("jupiter", datetime(2026, 1, 1, tzinfo=timezone.utc))
        sep = abs(p.heliocentric_au - j.heliocentric_au)
        self.assertLess(sep, 0.02)


if __name__ == "__main__":
    unittest.main()
