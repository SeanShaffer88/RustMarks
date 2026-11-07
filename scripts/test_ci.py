"""Dependency-free regression tests for CI status and diagnostic publishing."""
import contextlib
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).with_name("ci.py")
spec = importlib.util.spec_from_file_location("ci_runner", SCRIPT)
ci = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ci)


class RunnerTests(unittest.TestCase):
    def invoke(self, source, code=0):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            environment = {
                **os.environ,
                "GITHUB_ACTIONS": "true",
                "GITHUB_STEP_SUMMARY": str(root / "summary.md"),
            }
            result = subprocess.run(
                [sys.executable, str(SCRIPT), "test", sys.executable, "-c", source],
                cwd=root, env=environment, capture_output=True, text=True,
            )
            self.assertEqual(result.returncode, code, result.stderr)
            return result.stdout, (root / "artifacts/test.log").read_text(), (root / "summary.md").read_text()

    def test_success_and_stderr_are_logged_without_error_annotations(self):
        stdout, log, summary = self.invoke("import sys; print('stdout'); print('stderr', file=sys.stderr)")
        self.assertIn("stdout", log)
        self.assertIn("stderr", log)
        self.assertNotIn("::error", stdout)
        self.assertIn("test: passed", summary)

    def test_failure_preserves_exit_code_and_publishes_escaped_tail(self):
        stdout, log, summary = self.invoke("import sys; print('error[E1234]: <bad> 100%'); sys.exit(17)", 17)
        self.assertIn("::error title=test failed::", stdout)
        self.assertIn("100%25", stdout)
        self.assertIn("<bad>", log)
        self.assertIn("&lt;bad&gt;", summary)

    def test_child_output_cannot_inject_workflow_commands(self):
        stdout, log, _ = self.invoke("print('::error::not a real error')")
        self.assertIn("[output] ::error::not a real error", stdout)
        self.assertFalse(any(line.startswith("::error") for line in stdout.splitlines()))
        self.assertIn("::error::not a real error", log)

    def test_tail_is_bounded_but_full_log_is_retained(self):
        stdout, log, _ = self.invoke("import sys; [print('row-' + str(i)) for i in range(250)]; sys.exit(1)", 1)
        annotation = stdout.split("::error title=test failed::", 1)[1]
        self.assertNotIn("row-0%0A", annotation)
        self.assertIn("row-249", annotation)
        self.assertIn("row-0\n", log)

    def test_bad_gate_names_cannot_write_outside_artifacts(self):
        with self.assertRaises(ValueError):
            ci.run("../outside", [sys.executable])

    def test_missing_executable_fails_with_an_annotation(self):
        with tempfile.TemporaryDirectory() as directory:
            previous = Path.cwd()
            try:
                os.chdir(directory)
                output = io.StringIO()
                with patch.dict(os.environ, {"GITHUB_ACTIONS": "true", "GITHUB_STEP_SUMMARY": ""}), contextlib.redirect_stdout(output), contextlib.redirect_stderr(io.StringIO()):
                    with patch.object(subprocess, "Popen", side_effect=FileNotFoundError("missing")):
                        self.assertEqual(ci.run("test", ["missing-executable"]), 1)
                self.assertIn("::error title=test failed::", output.getvalue())
                self.assertIn("Unable to start", Path("artifacts/test.log").read_text())
            finally:
                os.chdir(previous)


if __name__ == "__main__":
    unittest.main()
