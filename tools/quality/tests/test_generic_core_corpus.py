"""The frozen oracle is replayable and cannot silently accept changed goldens."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
FIXTURE = ROOT / 'tools/quality/fixtures/generic-core'
spec = importlib.util.spec_from_file_location('generic_core_replay', FIXTURE / 'replay.py')
replay = importlib.util.module_from_spec(spec)
spec.loader.exec_module(replay)


class FrozenCorpusTests(unittest.TestCase):
    def test_portable_errors_preserve_logical_paths_and_non_reason_strings(self):
        value = {'reason': "missing '/tmp/case/head/source/a.rs'",
                 'artifacts': [{'path': '/tmp/case/head/source/a.rs'}]}
        actual = replay.portable_errors(value, Path('/tmp/case'))
        self.assertEqual(actual['reason'], "missing '$CASE_ROOT/head/source/a.rs'")
        self.assertEqual(actual['artifacts'], value['artifacts'])

    def test_every_frozen_case_matches_full_python_output(self):
        result = replay.replay()
        manifest = json.loads((FIXTURE / 'manifest.json').read_text())
        self.assertEqual(result['passed'], len(manifest['cases']))
        self.assertGreaterEqual(result['passed'], 33)
        self.assertFalse(result['authoritative'])
        states = {case['state'] for case in result['cases']}
        self.assertTrue({'pass', 'fail', 'measurement_error', 'blocked', 'rejected'} <= states)

    def test_changed_expected_output_is_rejected_before_evaluation(self):
        manifest = json.loads((FIXTURE / 'manifest.json').read_text())
        name = manifest['cases'][0]['expected']
        manifest = copy.deepcopy(manifest)
        manifest['files'] = {name: manifest['files'][name]}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'manifest.json').write_text(json.dumps(manifest))
            (root / name).write_bytes(b'{}\n')
            with self.assertRaisesRegex(ValueError, 'changed corpus file'):
                replay.replay(root)

    def test_inventory_covers_all_production_python_modules(self):
        inventory = json.loads((ROOT / 'docs/quality/gh-146/python-boundary.json').read_text())
        rows = inventory['modules']
        self.assertEqual({row['module'] for row in rows}, {
            str(p.relative_to(ROOT)) for p in (ROOT / 'tools/quality').glob('*.py')})
        self.assertEqual(len(rows), len({row['module'] for row in rows}))
        for row in rows:
            self.assertIn(row['category'], 'ABCD')
            for field in ('purpose', 'callers', 'ci_role', 'release_impact',
                          'authoritative_status', 'final_disposition'):
                self.assertTrue(row[field], (row['module'], field))
