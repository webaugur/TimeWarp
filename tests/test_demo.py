import unittest
from unittest.mock import patch

from timewarp.demo import pause, run_demo, scenes
from tests.test_cli import run


class DemoTests(unittest.TestCase):
    def test_pause_zero_waits_for_key(self):
        with patch("timewarp.demo.time.sleep") as slept:
            with patch("timewarp.demo.wait_for_key") as key:
                pause(0)
                slept.assert_not_called()
                key.assert_called_once()

    def test_pause_sleeps(self):
        with patch("timewarp.demo.time.sleep") as slept:
            pause(5)
            slept.assert_called_once_with(5)

    def test_cli_pause_zero(self):
        calls: list[list[str]] = []

        def invoke(tokens):
            calls.append(list(tokens))
            print("ok", tokens[0])
            return 0

        with patch("timewarp.demo.time.sleep") as slept:
            with patch("timewarp.demo.wait_for_key") as key:
                code = run_demo(pause_s=0, invoke=invoke)
        self.assertEqual(code, 0)
        slept.assert_not_called()
        self.assertGreaterEqual(key.call_count, 16)
        self.assertGreaterEqual(len(calls), 16)
        self.assertEqual(calls[0], ["count", "2026-07-04", "2026-12-25"])
        self.assertTrue(any(c[0] == "astro" for c in calls))
        self.assertTrue(any(c[0] == "month" for c in calls))

    def test_cli_demo_pause_zero(self):
        with patch("timewarp.demo.run_demo", return_value=0) as demo:
            code, out, err = run("demo", "--pause", "0")
        self.assertEqual(code, 0, err)
        demo.assert_called_once()
        self.assertEqual(demo.call_args.kwargs.get("pause_s"), 0.0)

    def test_scenes_include_core(self):
        names = [t[0] for t in scenes()]
        self.assertIn("Sun", names)
        self.assertIn("Chart", names)
