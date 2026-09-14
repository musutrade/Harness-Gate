#!/usr/bin/env python3
"""Real Linux observer acceptance; run after building both Rust binaries."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'rust-stable-collector'))
from validate_stable_candidate import check_trace

HELPER = Path(os.environ['HARNESS_EXEC_AUDIT']).resolve(strict=True)
PROBE = HELPER.with_name('audit-probe')

class ObserverAcceptance(unittest.TestCase):
    def capture(self, command, expected=0):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'events.jsonl'
            result = subprocess.run([str(HELPER), str(path), '--', *map(str, command)], capture_output=True, timeout=60)
            self.assertEqual(result.returncode, expected, result.stderr.decode())
            check_trace(path)
            return [json.loads(line) for line in path.read_text().splitlines()]

    def test_thread_exit_race(self):
        events = self.capture([PROBE, 'race'])
        self.assertEqual(events[-1]['exec_count'], 1)

    def test_descendant_execution(self):
        self.assertEqual(self.capture([PROBE, 'child'])[-1]['exec_count'], 2)

    def test_nonleader_thread_exec(self):
        self.assertEqual(self.capture([PROBE, 'thread-exec'])[-1]['exec_count'], 2)

    def test_exit_status_preserved(self):
        self.assertEqual(self.capture([PROBE, 'exit7'], 7)[-1]['root_exit_code'], 7)

    def test_forbidden_descendant_is_observed_and_rejected(self):
        with self.assertRaises(AssertionError):
            self.capture([PROBE, 'forbidden'])

    def test_execveat_is_observed(self):
        self.assertEqual(self.capture([PROBE, 'execveat'])[-1]['exec_count'], 2)

    def test_arguments_are_not_abbreviated(self):
        argument = 'x' * 32768
        events = self.capture(['/bin/true', argument, 'a"b', 'line\nbreak'])
        self.assertEqual(events[1]['argv'][1:], [argument, 'a"b', 'line\nbreak'])

    def test_incomplete_and_unknown_events_fail(self):
        events = self.capture(['/bin/true'])
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'bad.jsonl'
            cases = [events[:-1], events[1:], [*events, events[-1]],
                     [events[0], {'event':'unknown'}, events[-1]],
                     [events[0], events[1], dict(events[-1], exec_count=2)]]
            for records in cases:
                path.write_text(''.join(json.dumps(row)+'\n' for row in records))
                with self.subTest(records=records), self.assertRaises(AssertionError):
                    check_trace(path)

    def test_existing_log_cannot_be_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'existing'
            path.write_text('retained')
            result = subprocess.run([str(HELPER), str(path), '--', '/bin/true'], capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(path.read_text(), 'retained')

if __name__ == '__main__':
    unittest.main()
