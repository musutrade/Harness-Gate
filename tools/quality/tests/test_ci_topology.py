"""Frozen event contract independent of the aggregate implementation constants."""
import copy
import contextlib
import io
import json
from pathlib import Path
import re
import subprocess
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ci_quality
from quality_common import ROOT

CONTRACT = json.loads((ROOT / 'tools/quality/fixtures/ci-topology.json').read_text())


class TopologyTests(unittest.TestCase):
    def test_all_frozen_required_children_fail_closed(self):
        for event in CONTRACT['events']:
            required = [key for key, job in CONTRACT['jobs'].items()
                        if event in job['required_events']]
            needs = {key: {'result': 'success' if key in required else 'skipped'}
                     for key in CONTRACT['jobs']['quality-required']['needs']}
            self.assertEqual(ci_quality.aggregate(event, needs), [])
            for child in required:
                for state in ('missing', 'missing-result', 'failure', 'cancelled', 'skipped',
                              '', 'unknown', None, True, ['success'], {'status': 'success'}):
                    with self.subTest(event=event, child=child, state=state):
                        bad = copy.deepcopy(needs)
                        if state == 'missing':
                            del bad[child]
                        elif state == 'missing-result':
                            bad[child] = {}
                        else:
                            bad[child]['result'] = state
                        self.assertEqual(ci_quality.aggregate(event, bad), [child])

    def test_push_only_children_are_ignored_only_on_pull_requests(self):
        push_only = [key for key, job in CONTRACT['jobs'].items()
                     if job['required_events'] == ['push']]
        self.assertEqual(set(push_only), set(ci_quality.PUSH_ONLY))
        for child in push_only:
            for state in ('missing', 'skipped', 'failure', 'cancelled', 'success'):
                with self.subTest(child=child, state=state):
                    needs = {key: {'result': 'success'}
                             for key in CONTRACT['jobs']['quality-required']['needs']}
                    if state == 'missing':
                        del needs[child]
                    else:
                        needs[child]['result'] = state
                    self.assertEqual(ci_quality.aggregate('pull_request', needs), [])
                    self.assertEqual(ci_quality.aggregate('push', needs),
                                     [] if state == 'success' else [child])

    def test_malformed_required_child_fails_closed(self):
        for event in CONTRACT['events']:
            required = [key for key, job in CONTRACT['jobs'].items()
                        if event in job['required_events']]
            for child in required:
                for value in (None, [], 'success', True, 1):
                    with self.subTest(event=event, child=child, value=value):
                        needs = {key: {'result': 'success'} for key in required}
                        needs[child] = value
                        self.assertEqual(ci_quality.aggregate(event, needs), [child])

    def test_cli_only_evaluates_results_without_collection_or_processes(self):
        needs = {key: {'result': 'success'} for key, job in CONTRACT['jobs'].items()
                 if 'pull_request' in job['required_events']}
        cases = [('pull_request', json.dumps(needs), 0),
                 ('push', json.dumps(needs), 1), ('workflow_dispatch', json.dumps(needs), 1)]
        cases.extend(('pull_request', value, 1)
                     for value in ('{}', 'null', '[]', 'true', '1', '"success"', '{'))
        for event, payload, expected in cases:
            with self.subTest(event=event, payload=payload), contextlib.ExitStack() as stack:
                for target in ('ci_quality.Collector', 'ci_quality.verify',
                               'ci_quality.metadata', 'ci_quality.write_json',
                               'subprocess.Popen'):
                    stack.enter_context(patch(target, side_effect=AssertionError(target)))
                stack.enter_context(patch.dict('os.environ',
                                              EVENT_NAME=event, NEEDS_JSON=payload))
                stack.enter_context(patch.object(sys, 'argv', ['ci_quality.py', 'aggregate']))
                stdout, stderr = io.StringIO(), io.StringIO()
                stack.enter_context(contextlib.redirect_stdout(stdout))
                stack.enter_context(contextlib.redirect_stderr(stderr))
                self.assertEqual(ci_quality.main(), expected)
                self.assertEqual(stdout.getvalue(), '')
                self.assertLess(len(stderr.getvalue()), 1024)
                self.assertEqual(bool(stderr.getvalue()), bool(expected))

    def test_aggregate_step_allowlist_prohibits_heavy_work(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        body = re.split(r'^  [a-z][a-z-]+:\n',
                        workflow.split('  quality-required:\n')[1], maxsplit=1, flags=re.M)[0]
        steps = body.split('    steps:\n', 1)[1]
        # Compare all step content: extra actions, multiline commands, conditional
        # evaluation and shell error suppression must all require policy review.
        expected = '''
      - uses: actions/checkout@v5
      - name: Install Python
        uses: actions/setup-python@v7
        with:
          python-version: '3.x'
      - name: Require every quality gate
        env:
          EVENT_NAME: ${{ github.event_name }}
          NEEDS_JSON: ${{ toJSON(needs) }}
        run: python3 tools/quality/ci_quality.py aggregate
'''
        def significant_lines(value):
            return [line for line in value.splitlines()
                    if line.strip() and not line.lstrip().startswith('#')]
        self.assertEqual(significant_lines(steps), significant_lines(expected))

    def test_cross_platform_pr_failure_blocks_cli(self):
        needs = {key: {'result': 'success'} for key, job in CONTRACT['jobs'].items()
                 if 'pull_request' in job['required_events']}
        needs['test-cross-platform']['result'] = 'failure'
        result = subprocess.run([sys.executable, str(Path(ci_quality.__file__)),
                                 'aggregate', '--event', 'pull_request', '--needs',
                                 json.dumps(needs)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 1)
        self.assertIn('test-cross-platform', result.stderr)

    def test_workflow_matches_frozen_event_and_check_contract(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        self.assertIn('name: CI\n', workflow)
        self.assertIn('  push:\n    branches: [ main ]', workflow)
        self.assertIn('  pull_request:\n    branches: [ main ]', workflow)
        parts = re.split(r'^  ([a-z][a-z-]+):\n', workflow.split('jobs:\n')[1], flags=re.M)
        jobs = dict(zip(parts[1::2], parts[2::2]))
        self.assertEqual(set(jobs), set(CONTRACT['jobs']))
        for key, expected in CONTRACT['jobs'].items():
            with self.subTest(job=key):
                body = jobs[key]
                self.assertEqual(re.search(r'^    name: (.+)$', body, re.M)[1], expected['name'])
                condition = re.search(r'^    if: (.+)$', body, re.M)
                expected_condition = ("${{ github.event_name == 'push' }}"
                                      if expected['events'] == ['push'] else
                                      '${{ always() }}' if key in
                                      ('quality-required', 'quality-generic-shadow') else None)
                self.assertEqual(condition[1] if condition else None, expected_condition)
                needs = re.search(r'^    needs:\s*\[([^]]+)\]', body, re.M)
                self.assertEqual(re.findall(r'[a-z][a-z-]+', needs[1]) if needs else [],
                                 expected['needs'])
                matrix = re.search(r'^        os: \[(.+)\]', body, re.M)
                labels = matrix[1].split(', ') if matrix else [
                    re.search(r'^    runs-on: (.+)$', body, re.M)[1]]
                self.assertEqual(labels, expected['runner_labels'])
                if expected['required_events']:
                    self.assertNotIn('continue-on-error:', body)
        native = jobs['test-cross-platform']
        self.assertIn('fail-fast: false', native)
        self.assertIn('run: cargo nextest run --manifest-path tools/harness-gate/Cargo.toml '
                      '--locked --no-fail-fast', native)
        self.assertNotRegex(native, re.compile(r'^\s+(?:-\s+)?if:', re.M),
                            msg='full native tests cannot be conditional')
        aggregate = jobs['quality-required']
        self.assertEqual(re.findall(r'^        run: (.+)$', aggregate, re.M),
                         ['python3 tools/quality/ci_quality.py aggregate'])
        self.assertIn('EVENT_NAME: ${{ github.event_name }}', aggregate)
        self.assertIn('NEEDS_JSON: ${{ toJSON(needs) }}', aggregate)


if __name__ == '__main__':
    unittest.main()
