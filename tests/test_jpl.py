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
    _read_catalog_file,
    _reset_catalog_memo,
    catalog_objects_from_payload,
    fetch_sbdb,
    install_catalog,
    load_catalog,
    load_elements,
    load_query,
    lookup_catalog,
    mean_anomaly,
    parse_sbdb,
    query_slug,
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

    def test_query_slug(self):
        self.assertEqual(query_slug("433"), "433")
        self.assertEqual(query_slug("67P/C-G"), "67p-c-g")

    def test_load_query_from_cache(self):
        dest = Path(self.tmp.name) / "433.json"
        dest.write_bytes(CERES.read_bytes())
        with patch("timewarp.jpl.fetch_sbdb", side_effect=AssertionError("network")):
            el = load_query("433")
        self.assertAlmostEqual(el.a, 2.765552595034094, places=9)

    def test_load_query_required_without_cache(self):
        with patch("timewarp.jpl.fetch_sbdb", side_effect=TimeWarpError("offline")):
            with self.assertRaises(TimeWarpError):
                load_query("433")

    def test_ambiguous_list(self):
        with self.assertRaises(TimeWarpError) as ctx:
            parse_sbdb(
                {
                    "code": "300",
                    "message": "specified query matched more than one object",
                    "list": [{"pdes": "1"}, {"pdes": "2"}],
                },
                name="x",
            )
        self.assertIn("matches:", str(ctx.exception))


class CatalogTests(unittest.TestCase):
    def setUp(self):
        self._old = os.environ.get("TIMEWARP_SBDB_CATALOG")
        os.environ["TIMEWARP_SBDB_CATALOG"] = str(DATA / "sbdb-catalog-h11.json")
        _reset_catalog_memo()

    def tearDown(self):
        _reset_catalog_memo()
        if self._old is None:
            os.environ.pop("TIMEWARP_SBDB_CATALOG", None)
        else:
            os.environ["TIMEWARP_SBDB_CATALOG"] = self._old

    def test_lookup_by_name_and_number(self):
        el = lookup_catalog("Iris")
        self.assertIsNotNone(el)
        self.assertAlmostEqual(el.a, 2.385, places=3)
        self.assertEqual(lookup_catalog("7").name, el.name)

    def test_skips_hyperbolic(self):
        self.assertIsNone(lookup_catalog("Hyper"))
        self.assertIsNone(lookup_catalog("99999"))

    def test_resolve_body_skips_network(self):
        from timewarp.ephem import resolve_body

        with patch("timewarp.jpl.fetch_sbdb", side_effect=AssertionError("network")):
            with patch("timewarp.jpl.fetch_catalog", side_effect=AssertionError("network")):
                self.assertEqual(resolve_body("Iris"), "iris")
                self.assertEqual(resolve_body("7"), "iris")

    def test_reads_jpl_fields_data_table(self):
        payload = {
            "fields": [
                "pdes",
                "name",
                "full_name",
                "epoch",
                "a",
                "e",
                "i",
                "om",
                "w",
                "ma",
                "n",
                "H",
                "kind",
            ],
            "data": [
                [
                    "7",
                    "Iris",
                    "7 Iris",
                    "2461200.5",
                    "2.385",
                    "0.231",
                    "5.52",
                    "259.7",
                    "145.4",
                    "10.0",
                    "0.268",
                    "5.51",
                    "an",
                ]
            ],
        }
        rows = catalog_objects_from_payload(payload)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["name"], "Iris")


class InstallCatalogTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self._old_dir = os.environ.get("TIMEWARP_SBDB_DIR")
        self._old_cat = os.environ.get("TIMEWARP_SBDB_CATALOG")
        os.environ["TIMEWARP_SBDB_DIR"] = self.tmp.name
        os.environ.pop("TIMEWARP_SBDB_CATALOG", None)
        _reset_catalog_memo()

    def tearDown(self):
        _reset_catalog_memo()
        if self._old_dir is None:
            os.environ.pop("TIMEWARP_SBDB_DIR", None)
        else:
            os.environ["TIMEWARP_SBDB_DIR"] = self._old_dir
        if self._old_cat is None:
            os.environ.pop("TIMEWARP_SBDB_CATALOG", None)
        else:
            os.environ["TIMEWARP_SBDB_CATALOG"] = self._old_cat
        self.tmp.cleanup()

    def test_install_objects_dump(self):
        src = Path(self.tmp.name) / "download.json"
        src.write_bytes((DATA / "sbdb-catalog-h11.json").read_bytes())
        rows = install_catalog(src)
        self.assertGreaterEqual(len(rows), 2)
        el = lookup_catalog("Iris")
        self.assertIsNotNone(el)
        dest = Path(self.tmp.name) / "catalog-h11.json"
        self.assertTrue(dest.is_file())
        on_disk = json.loads(dest.read_text(encoding="utf-8"))
        self.assertIsInstance(on_disk.get("objects"), list)

    def test_install_jpl_table(self):
        src = Path(self.tmp.name) / "jpl.json"
        src.write_text(
            json.dumps(
                {
                    "fields": [
                        "pdes",
                        "name",
                        "full_name",
                        "epoch",
                        "a",
                        "e",
                        "i",
                        "om",
                        "w",
                        "ma",
                        "n",
                        "H",
                        "kind",
                    ],
                    "data": [
                        [
                            "7",
                            "Iris",
                            "7 Iris",
                            "2461200.5",
                            "2.385",
                            "0.231",
                            "5.52",
                            "259.7",
                            "145.4",
                            "10.0",
                            "0.268",
                            "5.51",
                            "an",
                        ]
                    ],
                }
            ),
            encoding="utf-8",
        )
        install_catalog(src)
        self.assertEqual(lookup_catalog("7").name, "iris")

    def test_load_raw_table_from_catalog_path(self):
        dest = Path(self.tmp.name) / "catalog-h11.json"
        dest.write_text(
            json.dumps(
                {
                    "fields": ["pdes", "name", "epoch", "a", "e", "i", "om", "w", "ma", "n"],
                    "data": [
                        [
                            "7",
                            "Iris",
                            "2461200.5",
                            "2.385",
                            "0.231",
                            "5.52",
                            "259.7",
                            "145.4",
                            "10.0",
                            "0.268",
                        ]
                    ],
                }
            ),
            encoding="utf-8",
        )
        rows = _read_catalog_file(dest)
        self.assertIsNotNone(rows)
        self.assertEqual(rows[0]["name"], "Iris")
        self.assertIn("iris", load_catalog())

    def test_empty_dump_errors(self):
        src = Path(self.tmp.name) / "empty.json"
        src.write_text(json.dumps({"objects": []}), encoding="utf-8")
        with self.assertRaises(TimeWarpError) as ctx:
            install_catalog(src)
        self.assertIn("no objects", str(ctx.exception))

    def test_missing_file_errors(self):
        with self.assertRaises(TimeWarpError):
            install_catalog(Path(self.tmp.name) / "missing.json")


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
