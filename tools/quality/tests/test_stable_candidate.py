"""Development checks for the required candidate execution audit, not host evidence."""
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'rust-stable-collector'))
from validate_stable_candidate import check_trace


class ExecutionAuditTests(unittest.TestCase):
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
