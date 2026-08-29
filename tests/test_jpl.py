import json
import os
import tempfile
import unittest
from datetime import datetime, timezone
from pathlib import Path
from unittest.mock import patch

from timewarp.errors import TimeWarpError
from timewarp.jpl import (
    SBDB_QUERY,
    fetch_sbdb,
    load_elements,
    mean_anomaly,
    parse_sbdb,
)

DATA = Path(__file__).resolve().parent / "data"
CERES = DATA / "sbdb-ceres.json"
COMET = DATA / "sbdb-67p.json"


class ParseSbdbTests(unittest.TestCase):
    def test_ceres_elements(self):
        payload = json.loads(CERES.read_text(encoding="utf-8"))
        el = parse_sbdb(payload, name="ceres")
        self.assertEqual(el.name, "ceres")
        self.assertAlmostEqual(el.a, 2.765552595034094, places=9)
        self.assertAlmostEqual(el.e, 0.07969229514816586, places=9)
        self.assertAlmostEqual(el.i, 10.58802780183462, places=6)
        self.assertAlmostEqual(el.N, 80.24862682043221, places=6)
        self.assertAlmostEqual(el.w, 73.29421453021587, places=6)
        self.assertAlmostEqual(el.M0, 274.4193463761342, places=6)
        self.assertAlmostEqual(el.n, 0.21430445064843, places=9)
        self.assertAlmostEqual(el.epoch_jd, 2461200.5, places=4)
        self.assertAlmostEqual(el.d_epoch, 2461200.5 - 2451543.5, places=4)
        self.assertIn("Ceres", el.designation or "")

    def test_mean_anomaly_at_epoch_is_ma(self):
        el = parse_sbdb(json.loads(CERES.read_text(encoding="utf-8")), name="ceres")
        self.assertAlmostEqual(mean_anomaly(el, el.d_epoch), el.M0, places=9)

    def test_67p_is_elliptical(self):
        el = parse_sbdb(json.loads(COMET.read_text(encoding="utf-8")), name="67p")
        self.assertLess(el.e, 1.0)
        self.assertAlmostEqual(el.a, 3.462249489765068, places=6)
        self.assertAlmostEqual(el.epoch_jd, 2457305.5, places=4)

    def test_object_not_found(self):
        with self.assertRaises(TimeWarpError) as ctx:
            parse_sbdb(
                {"code": "200", "message": "specified object was not found"},
                name="ceres",
            )
        self.assertIn("not found", str(ctx.exception))

    def test_missing_element(self):
        payload = json.loads(CERES.read_text(encoding="utf-8"))
        payload["orbit"]["elements"] = [
            row for row in payload["orbit"]["elements"] if row["name"] != "a"
        ]
        with self.assertRaises(TimeWarpError) as ctx:
            parse_sbdb(payload, name="ceres")
        self.assertIn("missing element a", str(ctx.exception))

    def test_hyperbolic_rejected(self):
        payload = json.loads(CERES.read_text(encoding="utf-8"))
        for row in payload["orbit"]["elements"]:
            if row["name"] == "e":
                row["value"] = "1.02"
        with self.assertRaises(TimeWarpError) as ctx:
            parse_sbdb(payload, name="ceres")
        self.assertIn("elliptical", str(ctx.exception))

    def test_n_from_a_when_missing(self):
        payload = json.loads(CERES.read_text(encoding="utf-8"))
        payload["orbit"]["elements"] = [
            row for row in payload["orbit"]["elements"] if row["name"] != "n"
        ]
        el = parse_sbdb(payload, name="ceres")
        expected = 0.9856076686 / (el.a**1.5)
        self.assertAlmostEqual(el.n, expected, places=6)


class LoadElementsTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self._old = os.environ.get("TIMEWARP_SBDB_DIR")
        os.environ["TIMEWARP_SBDB_DIR"] = self.tmp.name

    def tearDown(self):
        if self._old is None:
            os.environ.pop("TIMEWARP_SBDB_DIR", None)
        else:
            os.environ["TIMEWARP_SBDB_DIR"] = self._old
        self.tmp.cleanup()

    def test_cache_hit_skips_network(self):
        dest = Path(self.tmp.name) / "ceres.json"
        dest.write_bytes(CERES.read_bytes())
        with patch("timewarp.jpl.fetch_sbdb", side_effect=AssertionError("network")):
            el = load_elements("ceres")
        self.assertIsNotNone(el)
        self.assertAlmostEqual(el.a, 2.765552595034094, places=9)

    def test_fetch_writes_cache(self):
        payload = json.loads(CERES.read_text(encoding="utf-8"))
        with patch("timewarp.jpl.fetch_sbdb", return_value=payload) as fetch:
            el = load_elements("ceres")
        fetch.assert_called_once_with(SBDB_QUERY["ceres"])
        self.assertIsNotNone(el)
        cached = Path(self.tmp.name) / "ceres.json"
        self.assertTrue(cached.is_file())
        again = parse_sbdb(json.loads(cached.read_text(encoding="utf-8")), name="ceres")
        self.assertAlmostEqual(again.a, el.a, places=12)

    def test_stale_cache_when_fetch_fails(self):
        dest = Path(self.tmp.name) / "ceres.json"
        dest.write_bytes(CERES.read_bytes())
        old = datetime(2000, 1, 1, tzinfo=timezone.utc).timestamp()
        os.utime(dest, (old, old))
        with patch("timewarp.jpl.fetch_sbdb", side_effect=TimeWarpError("offline")):
            el = load_elements("ceres")
        self.assertIsNotNone(el)
        self.assertAlmostEqual(el.a, 2.765552595034094, places=9)

    def test_unknown_name_is_none(self):
        self.assertIsNone(load_elements("io"))

    def test_fetch_fail_without_cache_is_none(self):
        with patch("timewarp.jpl.fetch_sbdb", side_effect=TimeWarpError("offline")):
            self.assertIsNone(load_elements("ceres"))


class FetchSbdbTests(unittest.TestCase):
    def test_http_error(self):
        from email.message import Message
        from io import BytesIO
        from urllib.error import HTTPError

        err = HTTPError(
            "https://ssd-api.jpl.nasa.gov/sbdb.api",
            500,
            "fail",
            Message(),
            BytesIO(),
        )
        try:
            with patch("urllib.request.urlopen", side_effect=err):
                with self.assertRaises(TimeWarpError) as ctx:
                    fetch_sbdb("Ceres")
            self.assertIn("HTTP 500", str(ctx.exception))
        finally:
            err.close()


class EphemUsesSbdbTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self._old = os.environ.get("TIMEWARP_SBDB_DIR")
        os.environ["TIMEWARP_SBDB_DIR"] = self.tmp.name
        (Path(self.tmp.name) / "ceres.json").write_bytes(CERES.read_bytes())

    def tearDown(self):
        if self._old is None:
            os.environ.pop("TIMEWARP_SBDB_DIR", None)
        else:
            os.environ["TIMEWARP_SBDB_DIR"] = self._old
        self.tmp.cleanup()

    def test_position_at_epoch_distance(self):
        import math
        from datetime import timedelta

        from timewarp.ephem import day_number, eccentric_anomaly, position

        el = parse_sbdb(json.loads(CERES.read_text(encoding="utf-8")), name="ceres")
        # day_number(2000-01-01 00:00 UT) = 1.0; shift so d = SBDB epoch.
        when = datetime(2000, 1, 1, tzinfo=timezone.utc) + timedelta(days=el.d_epoch - 1.0)
        self.assertAlmostEqual(day_number(when), el.d_epoch, places=6)
        p = position("ceres", when)
        e = min(el.e, 0.99)
        ecc = math.radians(eccentric_anomaly(el.M0, e))
        r = el.a * (1.0 - e * math.cos(ecc))
        self.assertAlmostEqual(p.heliocentric_au, r, places=4)
        self.assertTrue(2.5 < p.heliocentric_au < 3.1)

    def test_fallback_table_when_offline(self):
        from timewarp.ephem import position

        os.environ["TIMEWARP_SBDB_DIR"] = self.tmp.name
        (Path(self.tmp.name) / "ceres.json").unlink()
        with patch("timewarp.jpl.fetch_sbdb", side_effect=TimeWarpError("offline")):
            p = position("ceres", datetime(1990, 4, 19, tzinfo=timezone.utc))
        self.assertEqual(p.body, "ceres")
        self.assertGreater(p.distance, 0)
        self.assertTrue(0 <= p.ra_deg < 360)


if __name__ == "__main__":
    unittest.main()
