import os
import unittest
from pathlib import Path
from unittest.mock import patch

from timewarp.cache import cache_path
from timewarp.holidays import holiday_cache_dir
from timewarp.iana_places import zone1970_tab
from timewarp.jpl import sbdb_cache_dir
from timewarp.paths import (
    cache_subdir,
    config_file,
    configure_stdio,
    swallow_broken_pipe,
    tzdata_zoneinfo_dir,
    user_cache_dir,
    user_config_dir,
)
from timewarp.passes import tle_dir


class UnixDirsTests(unittest.TestCase):
    def test_xdg_config_and_cache(self):
        env = {
            "XDG_CONFIG_HOME": "/tmp/tw-cfg",
            "XDG_CACHE_HOME": "/tmp/tw-cache",
        }
        with patch.dict(os.environ, env, clear=False):
            for key in ("APPDATA", "LOCALAPPDATA", "TIMEWARP_CACHE", "TIMEWARP_HOLIDAY_DIR"):
                os.environ.pop(key, None)
            with patch("timewarp.paths.os.name", "posix"):
                self.assertEqual(user_config_dir(), Path("/tmp/tw-cfg"))
                self.assertEqual(user_cache_dir(), Path("/tmp/tw-cache"))
                self.assertEqual(config_file("TIMEWARP_CACHE", "cache.json"), Path("/tmp/tw-cfg/timewarp/cache.json"))
                self.assertEqual(cache_subdir("TIMEWARP_HOLIDAY_DIR", "holidays"), Path("/tmp/tw-cache/timewarp/holidays"))

    def test_env_override_wins(self):
        with patch.dict(
            os.environ,
            {
                "TIMEWARP_CACHE": "/tmp/custom.json",
                "TIMEWARP_TLE_DIR": "/tmp/tles",
                "TIMEWARP_HOLIDAY_DIR": "/tmp/hols",
                "TIMEWARP_SBDB_DIR": "/tmp/sbdb",
            },
        ):
            self.assertEqual(cache_path(), Path("/tmp/custom.json"))
            self.assertEqual(tle_dir(), Path("/tmp/tles"))
            self.assertEqual(holiday_cache_dir(), Path("/tmp/hols"))
            self.assertEqual(sbdb_cache_dir(), Path("/tmp/sbdb"))


class WindowsDirsTests(unittest.TestCase):
    def test_appdata_when_no_xdg(self):
        env = {
            "APPDATA": r"C:\Users\x\AppData\Roaming",
            "LOCALAPPDATA": r"C:\Users\x\AppData\Local",
        }
        with patch.dict(os.environ, env, clear=False):
            os.environ.pop("XDG_CONFIG_HOME", None)
            os.environ.pop("XDG_CACHE_HOME", None)
            with patch("timewarp.paths.os.name", "nt"):
                self.assertEqual(user_config_dir(), Path(r"C:\Users\x\AppData\Roaming"))
                self.assertEqual(user_cache_dir(), Path(r"C:\Users\x\AppData\Local"))


class StdioTests(unittest.TestCase):
    def test_configure_stdio_is_safe(self):
        configure_stdio()

    def test_swallow_broken_pipe_is_safe(self):
        swallow_broken_pipe()


class TzdataTests(unittest.TestCase):
    def test_zone1970_is_readable(self):
        path = zone1970_tab()
        self.assertIsNotNone(path)
        self.assertTrue(path.is_file())
        text = path.read_text(encoding="utf-8")
        self.assertIn("America/New_York", text)

    def test_tzdata_package_has_zoneinfo(self):
        extra = tzdata_zoneinfo_dir()
        self.assertIsNotNone(extra)
        self.assertTrue((extra / "zone1970.tab").is_file())


if __name__ == "__main__":
    unittest.main()
