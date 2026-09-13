"""Development checks for the required candidate execution audit, not host evidence."""
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'rust-stable-collector'))
from validate_stable_candidate import check_trace


class ExecutionAuditTests(unittest.TestCase):
    def audit(self, trace):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'execve'
            path.write_text(trace)
            check_trace(path)

    def test_interleaved_resumed_calls_are_checked_with_complete_arguments(self):
        self.audit(
            '123 execve("/rust/bin/rustc", ["rustc", <unfinished ...>\n'
            '124 execve("/rust/bin/cargo", ["cargo"], 0x123 /* 40 vars */ <unfinished ...>\n'
            '123 <... execve resumed>"-C", "instrument-coverage"], 0x0) = 0\n'
            '124 <... execve resumed>) = 0\n'
            '123 +++ exited with 0 +++\n'
        )
        with self.assertRaisesRegex(AssertionError, '-Zdump-mir'):
            self.audit(
                '123 execve("/rust/bin/rustc", ["rustc", <unfinished ...>\n'
                '123 <... execve resumed>"-Zdump-mir=all"], 0x0) = 0\n'
            )

    def test_abbreviated_or_unreadable_records_fail_closed(self):
        for record in (
            'execve("/rust/bin/rustc", ["rustc", "long"...], 0x0) = 0',
            'execve("/rust/bin/rustc", ["rustc", ...], 0x0) = 0',
            'execve("/rust/bin/rustc"..., ["rustc"], 0x0) = 0',
            'execve("/rust/bin/rustc", 0x123, 0x0) = -1 EFAULT (Bad address)',
            'execve("/rust/bin/rustc", ["rustc"], 0x0)',
            'execve("/rust/bin/rustc", ["unterminated], 0x0) = 0',
            'strace: unexpected tracing error',
        ):
            with self.subTest(record=record), self.assertRaises(AssertionError):
                self.audit(record + '\n')

    def test_unmatched_duplicate_and_dangling_calls_fail_closed(self):
        for trace in (
            '123 <... execve resumed>) = 0\n',
            '123 execve("rustc", ["rustc"], 0x0 <unfinished ...>\n',
            '123 execve("rustc", ["rustc"], 0x0 <unfinished ...>\n'
            '124 <... execve resumed>) = 0\n',
            '123 execve("rustc", ["rustc"], 0x0 <unfinished ...>\n'
            '123 execve("cargo", ["cargo"], 0x0) = 0\n',
            '123 execve("rustc", ["rustc"], 0x0 <unfinished ...>\n'
            '123 +++ killed by SIGKILL +++\n',
        ):
            with self.subTest(trace=trace), self.assertRaises(AssertionError):
                self.audit(trace)

    def test_escaped_arguments_cannot_hide_forbidden_tools_or_flags(self):
        for executable, argument in (
            (r'/usr/bin/\x70ython3', 'collector.py'),
            ('/rust/bin/rustc', r'\055Zdump-mir=all'),
            ('/rust/bin/rustc', r'RUSTC_\x42OOTSTRAP=1'),
        ):
            with self.subTest(argument=argument), self.assertRaises(AssertionError):
                self.audit(f'execve("{executable}", ["{argument}"], 0x0) = 0\n')

    def test_literal_ellipsis_failed_path_lookup_and_signal_records(self):
        self.audit(
            '[pid 123] execve("/absent/cargo", ["cargo"], 0x0) = -1 ENOENT (No such file or directory)\n'
            '[pid 123] execve("/rust/bin/cargo", ["cargo", "...", "quoted\\\"value"], 0x0) = 0\n'
            '[pid 123] --- SIGCHLD {si_signo=SIGCHLD, si_code=CLD_EXITED} ---\n'
            '[pid 123] +++ killed by SIGKILL +++\n'
        )

    def test_stable_coverage_arguments_and_rejected_runtime_regressions(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'execve'
            path.write_text('execve("/rust/bin/rustc", ["rustc", "-C", "instrument-coverage", "--check-cfg", "cfg(feature, values(\\"nightly\\"))"], 0x0) = 0\n')
            check_trace(path)
            for command in (
                '"/usr/bin/python3", ["python3", "collector.py"]',
                '"/usr/bin/pypy3", ["pypy3", "collector.py"]',
                '"/rust/bin/rustc", ["rustc", "-Zdump-mir=all"]',
                '"/rust/nightly-x86_64/bin/rustc", ["rustc"]',
                '"/rust/bin/cargo", ["cargo", "+nightly", "build"]',
                '"/rust/bin/rustc_driver", ["rustc_driver"]',
            ):
                with self.subTest(command=command):
                    path.write_text(f'execve({command}, 0x0) = 0\n')
                    with self.assertRaises(AssertionError):
                        check_trace(path)
            path.write_text('')
            with self.assertRaises(AssertionError):
                check_trace(path)


if __name__ == '__main__':
    unittest.main()
