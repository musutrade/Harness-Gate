import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ci_quality as gate
from quality_common import ROOT, sha256


class AggregateTests(unittest.TestCase):
    def test_every_required_result_fails_closed_on_both_events(self):
        for event in ('push', 'pull_request'):
            needs = {name: {'result': 'success'} for name in gate.COMMON + gate.PUSH_ONLY}
            self.assertEqual(gate.aggregate(event, needs), [])
            required = gate.COMMON + (gate.PUSH_ONLY if event == 'push' else ())
            for name in required:
                for result in ('failure', 'cancelled', 'skipped', '', 'unknown'):
                    with self.subTest(event=event, name=name, result=result):
                        bad = copy.deepcopy(needs)
                        bad[name]['result'] = result
                        self.assertIn(name, gate.aggregate(event, bad))
                missing = copy.deepcopy(needs)
                del missing[name]
                self.assertIn(name, gate.aggregate(event, missing))

    def test_cli_rejects_negative_fixtures(self):
        for result in ('failure', 'cancelled', 'skipped'):
            needs = {name: {'result': 'success'} for name in gate.COMMON}
            needs['quality-coverage']['result'] = result
            command = [sys.executable, str(Path(gate.__file__)), 'aggregate', '--event',
                       'pull_request', '--needs', json.dumps(needs)]
            self.assertNotEqual(subprocess.run(command, capture_output=True).returncode, 0)

    def test_only_explicit_push_jobs_may_skip_in_pr(self):
        needs = {name: {'result': 'success'} for name in gate.COMMON}
        needs.update({name: {'result': 'skipped'} for name in gate.PUSH_ONLY})
        self.assertEqual(gate.aggregate('pull_request', needs), [])
        self.assertEqual(set(gate.aggregate('push', needs)), set(gate.PUSH_ONLY))
        with self.assertRaises(ValueError):
            gate.aggregate('workflow_dispatch', needs)

    def test_workflow_keeps_collection_required_and_upload_unconditional(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        job = workflow.split('  quality-coverage:\n')[1].split('  quality-contracts:\n')[0]
        self.assertNotIn("github.event_name == 'push'", job)
        self.assertIn('fetch-depth: 0', job)
        self.assertIn('ci_quality.py collect', job)
        upload = job.split('- name: Upload quality evidence')[1]
        self.assertIn('if: ${{ always() }}', upload)
        self.assertIn('target/quality/candidate', upload)
        aggregate = workflow.split('  quality-required:\n')[1]
        self.assertIn('if: ${{ always() }}', aggregate)
        self.assertIn('ci_quality.py aggregate', aggregate)
        self.assertIn('NEEDS_JSON: ${{ toJSON(needs) }}', aggregate)
        needs = aggregate.split('needs:')[1].split('runs-on:')[0]
        for name in gate.COMMON + gate.PUSH_ONLY:
            self.assertIn(name, needs)


class CandidateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / 'raw.json').write_text('{"raw":true}')
        for name in gate.REQUIRED_ARTIFACTS:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('fixture raw evidence')
        self.report = {'schema_version': 1, 'candidate': True, 'commit': 'a' * 40,
                       'base_sha': 'b' * 40, 'run_id': 'fresh-run',
                       'stages': {name: {'status': 'success'} for name in gate.STAGES},
                       'artifacts': {name: sha256(self.root / name)
                                     for name in gate.REQUIRED_ARTIFACTS | {'raw.json'}}}

    def verify(self):
        path = self.root / 'candidate.json'
        path.write_text(json.dumps(self.report))
        return gate.verify(path, 'a' * 40, 'b' * 40, 'fresh-run')

    def test_fresh_candidate_passes(self):
        self.assertTrue(self.verify()['candidate'])

    def test_stale_commit_base_run_or_raw_evidence_rejected(self):
        for key in ('commit', 'base_sha', 'run_id'):
            original = self.report[key]
            self.report[key] = 'stale'
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.verify()
            self.report[key] = original
        (self.root / 'raw.json').write_text('stale profiles')
        with self.assertRaises(ValueError):
            self.verify()
        (self.root / 'raw.json').unlink()
        with self.assertRaises(OSError):
            self.verify()

    def test_partial_failure_cancellation_and_skip_rejected(self):
        for stage in gate.STAGES:
            for status in ('failure', 'cancelled', 'skipped', 'running'):
                self.report['stages'][stage]['status'] = status
                with self.subTest(stage=stage, status=status), self.assertRaises(ValueError):
                    self.verify()
            self.report['stages'][stage]['status'] = 'success'
        del self.report['stages']['risk']
        with self.assertRaises(ValueError):
            self.verify()

    def test_missing_artifacts_and_path_escape_rejected(self):
        original = self.report['artifacts'].copy()
        for name in gate.REQUIRED_ARTIFACTS:
            self.report['artifacts'] = original.copy()
            del self.report['artifacts'][name]
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.verify()
        self.report['artifacts'] = {}
        with self.assertRaises(ValueError):
            self.verify()
        self.report['artifacts'] = original | {'../foreign.json': 'digest'}
        with self.assertRaises(ValueError):
            self.verify()


class CollectionTests(unittest.TestCase):
    def test_failure_retains_evidence_and_runs_remaining_stages(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'fresh'
            with patch.object(gate, 'metadata', return_value={}), \
                    patch.object(gate, 'git_sha', return_value='a' * 40), \
                    patch.object(gate, 'require_committed_sources'):
                collector = gate.Collector(output, 'b' * 40, 'a' * 40, 'test')
                calls = []

                def run(stage):
                    calls.append(stage)
                    (output / f'{stage}.log').write_text('raw stage evidence')
                    if stage == 'legacy':
                        raise ValueError('negative gate fixture')

                for stage in gate.STAGES:
                    setattr(collector, stage, lambda stage=stage: run(stage))
                with self.assertRaises(ValueError):
                    collector.collect()
                self.assertEqual(calls, list(gate.STAGES))
                report = json.loads((output / 'candidate.json').read_text())
                self.assertEqual(report['stages']['legacy']['status'], 'failure')
                self.assertEqual(report['stages']['matrix']['status'], 'success')
                self.assertIn('legacy.log', report['artifacts'])

    def test_existing_candidate_is_never_reused_or_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            marker = Path(directory) / 'original'
            marker.write_text('retained')
            with self.assertRaises(ValueError):
                gate.Collector(Path(directory), 'b' * 40, 'a' * 40, 'test')
            self.assertEqual(marker.read_text(), 'retained')


if __name__ == '__main__':
    unittest.main()
