import io
import unittest
from unittest.mock import patch

from timewarp.cli import main
from timewarp.launch import run_command_lines, run_repl, should_auto_repl, tokenize_line
from tests.test_cli import run


class ShellTests(unittest.TestCase):
    def test_quit(self):
        with patch("builtins.input", side_effect=["quit"]):
            self.assertEqual(run_repl(), 0)

    def test_runs_command(self):
        calls: list[list[str]] = []

        def invoke(tokens):
            calls.append(list(tokens))
            return 0

        with patch("builtins.input", side_effect=["weekday 2026-07-04", "q"]):
            self.assertEqual(run_repl(invoke=invoke), 0)
        self.assertEqual(calls, [["weekday", "2026-07-04"]])

    def test_strips_prog_name(self):
        calls: list[list[str]] = []

        def invoke(tokens):
            calls.append(list(tokens))
            return 0

        with patch("builtins.input", side_effect=["timewarp help add", "exit"]):
            self.assertEqual(run_repl(invoke=invoke), 0)
        self.assertEqual(calls, [["help", "add"]])

    def test_cli_shell_quit(self):
        with patch("builtins.input", side_effect=["q"]):
            code, out, err = run("shell")
        self.assertEqual(code, 0, err)
        self.assertIn("TimeWarp", out)
        self.assertIn("Working directory:", out)

    def test_no_args_still_help(self):
        code, out, err = run()
        self.assertEqual(code, 0, err)
        self.assertIn("Command help:", out)
        self.assertNotIn("timewarp>", out)

    def test_auto_repl_not_for_explicit_argv(self):
        self.assertFalse(should_auto_repl(False, []))
        self.assertFalse(should_auto_repl(True, ["sun"]))


class StdinStreamTests(unittest.TestCase):
    def test_tokenize_skips_comments(self):
        self.assertIsNone(tokenize_line("# weekday 2026-07-04"))
        self.assertIsNone(tokenize_line("  "))
        self.assertEqual(tokenize_line("timewarp weekday 2026-07-04"), ["weekday", "2026-07-04"])

    def test_run_lines(self):
        calls: list[list[str]] = []

        def invoke(tokens):
            calls.append(list(tokens))
            return 0

        code = run_command_lines(
            ["# hi", "", "weekday 2026-07-04", "sun --city Indianapolis"],
            invoke=invoke,
        )
        self.assertEqual(code, 0)
        self.assertEqual(
            calls,
            [["weekday", "2026-07-04"], ["sun", "--city", "Indianapolis"]],
        )

    def test_blocks_nested_shell(self):
        code = run_command_lines(["shell"], invoke=lambda t: 0)
        self.assertEqual(code, 2)

    def test_last_nonzero(self):
        def invoke(tokens):
            return 2 if tokens[0] == "b" else 0

        self.assertEqual(run_command_lines(["a", "b", "c"], invoke=invoke), 2)

    def test_dash_reads_stdin(self):
        buf = io.StringIO("weekday 2026-07-04\n")
        with patch("sys.stdin", buf):
            code, out, err = run("-")
        self.assertEqual(code, 0, err)
        self.assertIn("Saturday", out)
