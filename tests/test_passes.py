import os
import tempfile
import unittest
from datetime import date
from pathlib import Path

from timewarp.passes import (
    export_gpredict,
    load_tle_file,
    normalize_catalog,
    parse_tle_text,
    passes_for_day,
    select_sats,
    standard_magnitude,
    tle_checksum_ok,
    twilight_label,
    visual_magnitude,
)
from timewarp.places import lookup_place

TLE = Path(__file__).resolve().parent / "data" / "iss.tle"
SATCAT = Path(__file__).resolve().parent / "data" / "satcat-iss.csv"

_TLE_DIR = Path(tempfile.gettempdir()) / "timewarp-tests-tle-passes"
_TLE_DIR.mkdir(parents=True, exist_ok=True)
if SATCAT.is_file():
    (_TLE_DIR / "satcat.csv").write_bytes(SATCAT.read_bytes())
os.environ["TIMEWARP_TLE_DIR"] = str(_TLE_DIR)


class TleParseTests(unittest.TestCase):
    def test_fixture_iss(self):
        sats = load_tle_file(TLE)
        self.assertEqual(len(sats), 1)
        self.assertEqual(sats[0].catalog, 25544)
        self.assertIn("ISS", sats[0].name.upper())
        picked = select_sats(sats, "ISS", all_sats=False)
        self.assertEqual(picked[0].catalog, 25544)

    def test_malformed(self):
        from timewarp.errors import TimeWarpError

        with self.assertRaises(TimeWarpError):
            parse_tle_text("not a tle")


class TwilightLabelTests(unittest.TestCase):
    def test_bins(self):
        self.assertEqual(twilight_label(10.0), "day")
        self.assertEqual(twilight_label(-3.0), "civil")
        self.assertEqual(twilight_label(-9.0), "nautical")
        self.assertEqual(twilight_label(-15.0), "astronomical")
        self.assertEqual(twilight_label(-20.0), "night")


class PassGeometryTests(unittest.TestCase):
    def setUp(self):
        import timewarp.passes as passes_mod

        passes_mod._SATCAT = None
        passes_mod._SATCAT_KEY = None

    def test_nyc_2019_12_10_has_a_high_pass(self):
        sats = load_tle_file(TLE)
        place = lookup_place("New York")
        rows = passes_for_day(sats[0], date(2019, 12, 10), place, min_elev=10.0)
        self.assertGreaterEqual(len(rows), 1)
        best = max(rows, key=lambda p: p.max_alt_deg)
        self.assertGreater(best.max_alt_deg, 40.0)
        self.assertLess(best.aos, best.tca)
        self.assertLess(best.tca, best.los)
        self.assertEqual(best.tca.date(), date(2019, 12, 10))
        self.assertIn(best.twilight, {"day", "civil", "nautical", "astronomical", "night"})
        self.assertGreaterEqual(best.moon_sep_deg, 0.0)
        self.assertLessEqual(best.moon_sep_deg, 180.0)
        self.assertIsNotNone(best.magnitude)
        self.assertGreater(best.magnitude, -6.0)
        self.assertLess(best.magnitude, 8.0)


class CatalogTests(unittest.TestCase):
    def test_aliases(self):
        self.assertEqual(normalize_catalog("gps"), "gps-ops")
        self.assertEqual(normalize_catalog("visual"), "visual")

    def test_unknown(self):
        from timewarp.errors import TimeWarpError

        with self.assertRaises(TimeWarpError):
            normalize_catalog("not-a-catalog")


def _checksum(body: str) -> str:
    total = 0
    for ch in body:
        if ch.isdigit():
            total += int(ch)
        elif ch == "-":
            total += 1
    return str(total % 10)


def _retune(line: str, *, catalog: str | None = None, epoch: str | None = None) -> str:
    chars = list(line)
    if catalog is not None:
        chars[2:7] = list(catalog)
    if epoch is not None:
        chars[18:32] = list(epoch)
    body = "".join(chars[:68])
    return body + _checksum(body)


class GpredictExportTests(unittest.TestCase):
    def setUp(self):
        text = TLE.read_text(encoding="utf-8")
        lines = [ln for ln in text.splitlines() if ln[:1] in "12"]
        self.line1 = lines[0]
        self.line2 = lines[1]
        self.assertEqual(len(self.line1), 69)
        self.assertEqual(self.line1[18:32], "19343.69339541")
        self.assertTrue(tle_checksum_ok(self.line1))
        self.assertTrue(tle_checksum_ok(self.line2))

    def _pair(self, catalog: str, epoch: str) -> tuple[str, str]:
        return (
            _retune(self.line1, catalog=catalog, epoch=epoch),
            _retune(self.line2, catalog=catalog),
        )

    def _write(self, directory: Path, name: str, body: str) -> Path:
        path = directory / name
        path.write_text(body, encoding="utf-8")
        return path

    def test_strips_spacetrack_name_and_keeps_newer_epoch(self):
        old1, old2 = self._pair("00005", "19300.00000000")
        new1, new2 = self._pair("    5", "19350.00000000")
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            named = self._write(root, "3le.txt", f"0 VANGUARD 1\n{old1}\n{old2}\n")
            bare = self._write(root, "bulk.txt", f"{new1}\n{new2}\n")
            result = export_gpredict([named, bare])
        self.assertEqual(result.satellites, 1)
        self.assertEqual(result.dropped_older, 1)
        self.assertEqual(result.skipped_alpha5, 0)
        self.assertTrue(result.text.startswith("VANGUARD 1\n"))
        self.assertNotIn("0 VANGUARD", result.text)
        self.assertIn(new1, result.text)
        self.assertNotIn(old1, result.text)

    def test_name_survives_when_newer_set_is_nameless(self):
        old1, old2 = self._pair("00005", "19300.00000000")
        new1, new2 = self._pair("00005", "19350.00000000")
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            named = self._write(root, "3le.txt", f"0 VANGUARD 1\n{old1}\n{old2}\n")
            bare = self._write(root, "bulk.txt", f"{new1}\n{new2}\n")
            result = export_gpredict([bare, named])
        lines = result.text.splitlines()
        self.assertEqual(lines[0], "VANGUARD 1")
        self.assertEqual(lines[1], new1)

    def test_alpha5_omitted_and_analyst_named_by_catalog(self):
        num1, num2 = self._pair("81011", "26242.01450661")
        # Blank the international designator (columns 10-17).
        chars = list(num1)
        chars[9:17] = list("        ")
        body = "".join(chars[:68])
        num1 = body + _checksum(body)
        alpha1, alpha2 = self._pair("A0001", "19343.69339541")
        again1, again2 = self._pair("A0001", "19350.00000000")
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = self._write(
                root,
                "analyst.tle",
                f"{num1}\n{num2}\n{alpha1}\n{alpha2}\n{again1}\n{again2}\n",
            )
            result = export_gpredict([path])
        self.assertEqual(result.satellites, 1)
        self.assertEqual(result.skipped_alpha5, 1)
        self.assertTrue(result.text.startswith("81011\n"))
        self.assertNotIn("A0001", result.text.splitlines()[0])

    def test_bad_checksum_does_not_swallow_the_next_set(self):
        good1, good2 = self._pair("00022", "19343.69339541")
        bad = good1[:-1] + ("0" if good1[-1] != "0" else "1")
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = self._write(root, "mixed.txt", f"{bad}\n0 KEEP\n{good1}\n{good2}\n")
            result = export_gpredict([path])
        self.assertEqual(result.skipped_bad, 1)
        self.assertEqual(result.text.splitlines()[0], "KEEP")
        self.assertIn(good1, result.text)

    def test_year_2000_epoch_beats_1999(self):
        old1, old2 = self._pair("00011", "99300.00000000")
        new1, new2 = self._pair("00011", "00010.00000000")
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = self._write(root, "years.txt", f"{old1}\n{old2}\n{new1}\n{new2}\n")
            result = export_gpredict([path])
        self.assertIn(new1, result.text)
        self.assertNotIn(old1, result.text)
        self.assertEqual(result.dropped_older, 1)

    def test_catalog_order(self):
        a1, a2 = self._pair("00010", "19343.69339541")
        b1, b2 = self._pair("00002", "19343.69339541")
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = self._write(root, "two.txt", f"TEN\n{a1}\n{a2}\nTWO\n{b1}\n{b2}\n")
            result = export_gpredict([path])
        lines = result.text.splitlines()
        self.assertEqual(lines[0], "TWO")
        self.assertEqual(lines[3], "TEN")

    def test_missing_file(self):
        from timewarp.errors import TimeWarpError

        with self.assertRaises(TimeWarpError):
            export_gpredict([Path("/tmp/timewarp-no-such-tle.tle")])


class MagnitudeTests(unittest.TestCase):
    def test_iss_rcs_stdmag(self):
        std = standard_magnitude(399.0524)
        self.assertAlmostEqual(std, 5.0 - 2.5 * __import__("math").log10(399.0524), places=6)
        mag = visual_magnitude(400.0, 90.0, 399.0524)
        self.assertLess(mag, std)


if __name__ == "__main__":
    unittest.main()
