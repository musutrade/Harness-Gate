from __future__ import annotations

import contextlib
import io
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import benchmarks
import benchmark_fixture_probe as probe


class BenchmarkEvidenceTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.raw_root = Path(self.temporary.name) / "benchmark-runs"

    def command(self, root, *, exit_code=0, report="valid", state="valid", stdout="out", stderr="err"):
        """Fake only the timed CLI boundary; archive and validation stay real."""
        def completed(command, cwd, env):
            self.assertEqual(command[-2:], ["verify", "--all"])
            self.assertEqual(env["HARNESS_GATE_BENCHMARK_SERVICE_URL"], "http://127.0.0.1:43123")
            reports = root / ".harness-gate" / "reports"
            logs = reports / "logs"
            logs.mkdir(parents=True)
            (logs / "benchmark_diff.log").write_text("worker traceback retained\n")
            (logs / "benchmark_status.log").write_text("second worker evidence\n")
            document = {
                "passed": report != "failed",
                "steps": [
                    {"label": f"step-{index}", "duration_ms": 100, "passed": True,
                     "log": str(logs / "benchmark_diff.log")}
                    for index in range(4)
                ],
            }
            if report != "missing":
                (reports / "test_result.json").write_text(
                    "{invalid report" if report == "malformed" else json.dumps(document)
                )
            state_path = root / benchmarks.STATE_RELATIVE
            if state == "missing":
                state_path.unlink()
            elif state == "malformed":
                state_path.write_text("{invalid state")
            else:
                observed = {"current": 0, "observed_peak": 2, "workers": 2} if state == "valid" else state
                state_path.write_text(json.dumps(observed))
            return 1.25, subprocess.CompletedProcess(command, exit_code, stdout, stderr)
        return completed

    def invoke(self, **kwargs):
        # Raw output intentionally lives outside the fixture's cleanup scope.
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture_root = root
            with patch.object(benchmarks, "timed", side_effect=self.command(root, **kwargs)):
                return benchmarks.verification_sample(Path("unused-binary"), root, self.raw_root, 1, "parallel", 2)

    @property
    def archive(self) -> Path:
        return self.raw_root / "parallel" / "sample-1"

    def assert_retained(self, *, exit_code=0, report=True, state=True):
        self.assertFalse(self.fixture_root.exists(), "the source fixture must already be removed")
        self.assertEqual((self.archive / "logs" / "benchmark_diff.log").read_text(), "worker traceback retained\n")
        self.assertEqual((self.archive / "logs" / "benchmark_status.log").read_text(), "second worker evidence\n")
        self.assertEqual((self.archive / "test_result.json").is_file(), report)
        self.assertEqual((self.archive / "parallel-state.json").is_file(), state)
        result = json.loads((self.archive / "command-result.json").read_text())
        self.assertEqual(result["exit_code"], exit_code)
        self.assertEqual(result["seconds"], 1.25)
        return result

    def test_nonzero_cli_still_fails_with_valid_report_and_retained_evidence(self):
        error = io.StringIO()
        with contextlib.redirect_stderr(error), self.assertRaises(SystemExit) as raised:
            self.invoke(exit_code=23)
        self.assertEqual(raised.exception.code, 1)
        self.assertIn(str(self.archive), error.getvalue())
        result = self.assert_retained(exit_code=23)
        self.assertEqual(result["stdout_tail"], "out")
        self.assertEqual(result["stderr_tail"], "err")
        self.assertFalse(result["stdout_truncated"])
        self.assertFalse(result["stderr_truncated"])

    def test_missing_report_retains_worker_logs_and_state(self):
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as raised:
            self.invoke(report="missing")
        self.assertEqual(raised.exception.code, 1)
        self.assert_retained(report=False)

    def test_missing_reports_directory_still_retains_command_and_state(self):
        with self.assertRaises(SystemExit) as raised, contextlib.redirect_stderr(io.StringIO()):
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                result = subprocess.CompletedProcess([], 9, "", "failed before reports")
                with patch.object(benchmarks, "timed", return_value=(1.25, result)):
                    benchmarks.verification_sample(Path("unused-binary"), root, self.raw_root, 1, "parallel", 2)
        self.assertEqual(raised.exception.code, 1)
        self.assertFalse(root.exists())
        self.assertFalse((self.archive / "test_result.json").exists())
        self.assertEqual(json.loads((self.archive / "command-result.json").read_text())["exit_code"], 9)
        self.assertEqual(json.loads((self.archive / "parallel-state.json").read_text()), {"current": 0, "observed_peak": 0, "workers": 0})

    def test_malformed_report_remains_a_failure_and_is_retained_verbatim(self):
        with self.assertRaises(json.JSONDecodeError):
            self.invoke(report="malformed")
        self.assert_retained()
        self.assertEqual((self.archive / "test_result.json").read_text(), "{invalid report")

    def test_failed_report_remains_a_failure_and_is_retained(self):
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            self.invoke(report="failed")
        self.assert_retained()
        self.assertFalse(json.loads((self.archive / "test_result.json").read_text())["passed"])

    def test_invalid_state_remains_a_failure_and_is_retained(self):
        states = [
            "missing", "malformed",
            {"current": 1, "observed_peak": 2, "workers": 2},
            {"current": 0, "observed_peak": "2", "workers": 2},
            {"current": 0, "observed_peak": 2, "workers": "2"},
            {"current": 0, "observed_peak": 2, "workers": 1},
            {"current": 0, "observed_peak": 0, "workers": 2},
            {"current": 0, "observed_peak": 3, "workers": 2},
        ]
        for index, state in enumerate(states):
            with self.subTest(state=state):
                self.raw_root = Path(self.temporary.name) / f"invalid-state-{index}"
                exception = json.JSONDecodeError if state == "malformed" else SystemExit
                with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(exception):
                    self.invoke(state=state)
                self.assert_retained(state=state != "missing")
                if isinstance(state, dict):
                    self.assertEqual(json.loads((self.archive / "parallel-state.json").read_text()), state)

    def test_command_output_retains_bounded_character_tails(self):
        limit = 64 * 1024
        stdout = "prefix" + "\u03bb" * limit
        stderr = "prefix" + "e" * (limit - 8) + "THE-END!"
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            self.invoke(exit_code=7, stdout=stdout, stderr=stderr)
        result = self.assert_retained(exit_code=7)
        self.assertEqual(result["stdout_tail"], stdout[-limit:])
        self.assertEqual(result["stderr_tail"], stderr[-limit:])
        self.assertTrue(result["stdout_truncated"])
        self.assertTrue(result["stderr_truncated"])

    def test_output_at_limit_is_not_marked_truncated(self):
        self.invoke(stdout="x" * (64 * 1024), stderr="")
        result = self.assert_retained()
        self.assertEqual(len(result["stdout_tail"]), 64 * 1024)
        self.assertFalse(result["stdout_truncated"])
        self.assertEqual(result["stderr_tail"], "")
        self.assertFalse(result["stderr_truncated"])

    def test_success_keeps_command_timing_and_existing_sample_fields(self):
        result = self.invoke()
        self.assert_retained()
        self.assertEqual(result["sample"], 1)
        self.assertEqual(result["mode"], "parallel")
        self.assertEqual(result["seconds"], 1.25)
        self.assertAlmostEqual(result["scheduler_overhead_seconds"], 0.85)
        self.assertEqual(result["configured_limit"], 2)
        self.assertEqual(result["observed_peak"], 2)
        self.assertEqual(len(result["steps"]), 4)
        self.assertTrue(all(step["passed"] for step in result["steps"]))
        self.assertEqual(result["service_startup_reuse"], {"service": None, "node_uses": 0, "starts": 0, "reuses": 0})
        self.assertEqual(result["report"], str(Path("benchmark-runs/parallel/sample-1/test_result.json")))
        self.assertTrue(all(step["log"] == str(Path("benchmark-runs/parallel/sample-1/logs/benchmark_diff.log")) for step in result["steps"]))

    def test_probe_runs_five_samples_through_the_same_fixture_and_validation(self):
        with patch.object(benchmarks, "init_fixture") as initialize, \
                patch.object(benchmarks, "configure_interpreter") as interpreter, \
                patch.object(benchmarks, "configure_execution", return_value=2) as configure, \
                patch.object(benchmarks, "verification_sample", return_value={}) as sample:
            self.assertEqual(len(probe.run(Path("unused-binary"))), 5)
        root = initialize.call_args.args[0]
        interpreter.assert_called_once_with(root)
        configure.assert_called_once_with(root, parallel=True)
        self.assertEqual([call.args[3] for call in sample.call_args_list], [1, 2, 3, 4, 5])
        self.assertTrue(all(call.args[1] == root and call.args[4:] == ("parallel", 2) for call in sample.call_args_list))

    def test_probe_prints_worker_traceback_and_evidence_before_cleanup_on_failure(self):
        original_sample = benchmarks.verification_sample

        def initialize(root):
            self.fixture_root = root

        def fail_sample(binary, root, raw_root, number, mode, limit):
            self.raw_root = raw_root
            with patch.object(benchmarks, "timed", side_effect=self.command(root, exit_code=19)):
                return original_sample(binary, root, raw_root, number, mode, limit)

        output = io.StringIO()
        with patch.object(benchmarks, "init_fixture", side_effect=initialize), \
                patch.object(benchmarks, "configure_interpreter"), \
                patch.object(benchmarks, "configure_execution", return_value=2), \
                patch.object(benchmarks, "verification_sample", side_effect=fail_sample) as sample, \
                contextlib.redirect_stderr(output), self.assertRaises(SystemExit) as raised:
            probe.run(Path("unused-binary"))
        self.assertEqual(raised.exception.code, 1)
        sample.assert_called_once()
        self.assertFalse(self.fixture_root.exists())
        self.assertFalse(self.raw_root.exists())
        for expected in ("worker traceback retained", "second worker evidence", "parallel-state.json", "command-result.json", '"exit_code": 19', "test_result.json"):
            self.assertIn(expected, output.getvalue())

    def test_probe_diagnostics_are_bounded_and_keep_worker_log_tails(self):
        self.archive.mkdir(parents=True)
        limit = probe.DIAGNOSTIC_TAIL_BYTES
        (self.archive / "worker.log").write_text("discard-me" + "x" * limit + "worker traceback end")
        output = io.StringIO()
        with contextlib.redirect_stderr(output):
            probe.print_retained_evidence(self.archive)
        self.assertIn("worker traceback end", output.getvalue())
        self.assertNotIn("discard-me", output.getvalue())
        self.assertIn(f"last {limit}", output.getvalue())
        self.assertLess(len(output.getvalue()), limit + 2048)


if __name__ == "__main__":
    unittest.main()
