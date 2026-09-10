import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location('arc_selfhost', ROOT / 'docs/dogfood/arc-admin/cost/selfhost_trial.py')
trial = importlib.util.module_from_spec(spec)
spec.loader.exec_module(trial)


class SelfhostCostTests(unittest.TestCase):
    def test_failed_command_keeps_exit_status_log_and_elapsed_time(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            result = trial.run(root, 'failed', [sys.executable, '-c', 'print("failed gate"); raise SystemExit(7)'], root, {})
            self.assertEqual(result['exit_code'], 7)
            self.assertGreater(result['elapsed_seconds'], 0)
            self.assertIn('failed gate', (root / 'failed.log').read_text())
            self.assertEqual(json.loads((root / 'failed.json').read_text()), result)

    def test_summary_requires_all_pairs_and_never_converts_failure_to_acceptance(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / 'target/trial'
            evidence = output / 'evidence'
            evidence.mkdir(parents=True)
            argv = ['trial', 'summarize', '--output', str(output)]
            with patch.object(trial, 'ROOT', root), patch.object(sys, 'argv', argv):
                with self.assertRaises(AssertionError):
                    trial.main()
                for sample in (1, 2, 3):
                    for mode, duration, code in [('before', 10, 0), ('shadow', 14, 1)]:
                        directory = evidence / f'{sample}-{mode}'
                        directory.mkdir()
                        trial.write(directory / 'sample.json', {'sample': sample, 'mode': mode,
                            'elapsed_seconds': duration, 'commands': [{'exit_code': code}]})
                trial.main()
            summary = json.loads((evidence / 'summary.json').read_text())
            self.assertFalse(summary['authority_transfer_permitted'])
            for pair in summary['pairs']:
                self.assertEqual(pair['added_seconds'], 4)
                self.assertTrue(pair['before_passed'])
                self.assertFalse(pair['shadow_passed'])
