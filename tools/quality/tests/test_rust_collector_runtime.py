"""Standalone positives use real private tools; failures keep their original bytes."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

QUALITY = Path(__file__).resolve().parents[1]
ROOT = QUALITY.parents[1]
sys.path.insert(0, str(QUALITY))
import build_rust_collector as builder
import rust_native_policy as policy


class BuildInputTests(unittest.TestCase):
    def test_changed_source_and_unsafe_destination_reject_before_assembly(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / 'source'
            source.write_text('original')
            lock = {'schema': 'rust-collector-build-inputs/1', 'build_inputs': {},
                    'files': {'../escape': {'source': str(source), 'sha256': builder.sha(source), 'mode': 0o644}}}
            with self.assertRaisesRegex(ValueError, 'unsafe payload'):
                builder.build(lock, root / 'output')
            self.assertFalse((root / 'output').exists())
            lock['files'] = {'file': lock['files']['../escape']}
            source.write_text('changed')
            with self.assertRaisesRegex(ValueError, 'changed build input'):
                builder.build(lock, root / 'output')
            self.assertFalse((root / 'output').exists())


class StandaloneNativeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        runtime = os.environ.get('RUST_COLLECTOR_RUNTIME')
        if not runtime:
            raise unittest.SkipTest('RUST_COLLECTOR_RUNTIME required: assembled private Linux runtime')
        cls.runtime = Path(runtime).resolve(strict=True)
        base = ROOT / 'target/gh-228/runtime-tests'
        base.mkdir(parents=True, exist_ok=True)
        cls.work = Path(tempfile.mkdtemp(dir=base))
        (cls.work / 'tmp').mkdir()
        (cls.work / 'cargo-home').mkdir()
        cls.environment = {'PATH': str(cls.runtime / 'bin'),
                           'LD_LIBRARY_PATH': str(cls.runtime / 'lib') + ':' + str(cls.runtime / 'rust/lib'),
                           'TMPDIR': str(cls.work / 'tmp'), 'CARGO_HOME': str(cls.work / 'cargo-home'),
                           'CARGO_NET_OFFLINE': 'true'}
        shutil.copyfile(QUALITY / 'fixtures/rust-native/driver_complete.rs', cls.work / 'fixture.rs')
        cls.anchors = {}
        for name in ('base', 'head'):
            code = ('import sys; from pathlib import Path; r=Path(sys.argv[1]); '
                    'sys.path.insert(0,str(r/"app")); import rust_native_driver as n; '
                    'print(n.collect_fixture(Path(sys.argv[2]),Path(sys.argv[3]),'
                    'r/"bin/harness-gate-rust-native-driver",r/"rust"))')
            result = cls.command(name + '-capture', [str(cls.runtime / 'python/bin/python3'), '-I', '-S', '-B',
                '-c', code, str(cls.runtime), str(cls.work / 'fixture.rs'), str(cls.work / name)])
            if result.returncode:
                raise AssertionError(result.stderr)
            cls.anchors[name] = result.stdout.strip()
        (cls.work / 'anchors.json').write_text(json.dumps(cls.anchors))

    @classmethod
    def command(cls, name, args, data=None):
        result = subprocess.run(args, cwd=cls.work, env=cls.environment,
                                input=data, text=True, capture_output=True)
        (cls.work / (name + '.stdout')).write_text(result.stdout)
        (cls.work / (name + '.stderr')).write_text(result.stderr)
        (cls.work / (name + '.command.json')).write_text(json.dumps({
            'command': args, 'cwd': str(cls.work), 'environment': cls.environment,
            'exit_code': result.returncode}, indent=2))
        return result

    def entry(self, name, *args, data=None):
        return self.command(name, [str(self.runtime / 'bin/harness-gate-rust-collector'), *args], data)

    def measurement(self, name):
        result = self.entry(name + '-certify', 'certify', '--evidence', name, '--anchor', self.anchors[name])
        self.assertEqual(result.returncode, 0, result.stderr)
        response = json.loads(result.stdout)
        self.assertIsNone(response['error'])
        return response['measurement']

    def test_real_low_coverage_is_completion_and_legacy_exit_is_preserved(self):
        report = self.measurement('base')
        self.assertNotIn('passed', report)
        self.assertEqual(report['coverage']['lines'], {'count': 35, 'covered': 26})
        debt = next(f for f in report['functions'] if f['name'] == 'legacy_debt')
        self.assertEqual(debt['crap_exact'], [56, 1])
        self.assertNotIn('passed', debt)
        result = self.command('legacy-certify', [str(self.runtime / 'python/bin/python3'), '-I', '-S', '-B',
            '-c', 'import sys; sys.path.insert(0,sys.argv.pop(1)); import rust_native_driver as n; sys.exit(n.main())',
            str(self.runtime / 'app'), 'certify', '--evidence', str(self.work / 'base'),
            '--anchor', self.anchors['base'], '--output', str(self.work / 'legacy.json')])
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertFalse(json.loads((self.work / 'legacy.json').read_text())['passed'])

    def test_doctor_and_unknown_collect_never_sample(self):
        result = self.entry('doctor', 'doctor', '--json')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(json.loads(result.stdout)['runtime_complete'])
        result = self.entry('malformed-collect', 'collect', data='{"schema":1,"schema":2}')
        self.assertEqual(result.returncode, 1)
        self.assertEqual(json.loads(result.stdout)['evidence'], [])
        self.assertIsNotNone(json.loads(result.stdout)['error'])
        record = json.loads((QUALITY / 'fixtures/harness-evidence/polyglot.json').read_text())[0]
        project = json.loads((QUALITY / 'fixtures/project-model/base.json').read_text())
        output = self.work / 'must-not-sample'
        request = {'schema': 'harness-collector-request/v1', 'project': project['id'],
                   'component': record['component'], 'collector': record['collector'],
                   'context': record['context'], 'requested_capabilities': ['coverage.line'],
                   'workspace_root': str(self.work), 'output_root': str(output), 'parameters': {}}
        result = self.entry('unknown-collect', 'collect', data=json.dumps(request))
        self.assertEqual(result.returncode, 1)
        self.assertIn('unknown tested Core/protocol/ABI', json.loads(result.stdout)['error']['message'])
        self.assertEqual(json.loads(result.stdout)['evidence'], [])
        self.assertFalse(output.exists())

    def test_wrong_anchor_has_no_measurement(self):
        result = self.entry('wrong-anchor', 'certify', '--evidence', 'base', '--anchor', '0' * 64)
        self.assertEqual(result.returncode, 1)
        self.assertIsNone(json.loads(result.stdout)['measurement'])

    def test_private_cargo_capture_preserves_incomplete_classification(self):
        vendor = Path(os.environ['RUST_COLLECTOR_TEST_VENDOR']).resolve(strict=True)
        (self.work / 'cargo-home/config.toml').write_text(
            '[source.crates-io]\nreplace-with = "fixture-inputs"\n'
            '[source.fixture-inputs]\ndirectory = ' + json.dumps(str(vendor)) + '\n')
        project = self.work / 'project'
        shutil.copytree(QUALITY / 'fixtures/rust-native/file-classification', project)
        code = ('import sys; from pathlib import Path; r=Path(sys.argv[1]); '
                'sys.path.insert(0,str(r/"app")); import rust_native_driver as n; '
                'print(n.collect_cargo(Path(sys.argv[2]),Path(sys.argv[3]),'
                'r/"bin/harness-gate-rust-native-driver",r/"rust",["contract"],runtime=r))')
        result = self.command('cargo-capture', [str(self.runtime / 'python/bin/python3'), '-I', '-S', '-B', '-c',
            code, str(self.runtime), str(project / 'Cargo.toml'), str(self.work / 'cargo')])
        self.assertEqual(result.returncode, 0, result.stderr)
        anchor = result.stdout.strip()
        result = self.entry('classify', 'classify', '--evidence', 'cargo/raw', '--anchor', anchor)
        # This existing negative fixture deliberately includes orphan.rs and an
        # unloaded conditional module. Successful capture cannot waive either.
        self.assertEqual(result.returncode, 1)
        self.assertIsNone(json.loads(result.stdout)['measurement'])
        self.assertIn('incomplete classification', json.loads(result.stdout)['error']['message'])

    def test_complete_fixture_classifies_with_private_reexport(self):
        result = self.entry('complete-classify', 'classify', '--evidence', 'base',
                            '--anchor', self.anchors['base'])
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)['measurement']
        self.assertTrue(report['classification_complete'])
        self.assertNotIn('measurement_passed', report)
        self.assertNotIn('baseline_accepted', report)

    def test_released_core_evaluates_typed_private_measurements(self):
        binary = os.environ.get('HARNESS_GATE_NATIVE_POLICY_BINARY')
        if not binary:
            self.skipTest('HARNESS_GATE_NATIVE_POLICY_BINARY required for real Rust Core evaluation')
        base, head = self.measurement('base'), self.measurement('head')
        reports = (base, head)
        contexts = ({'target': 'native', 'run': 'private-runtime', 'commit': 'a' * 40, 'base_commit': 'b' * 40},
                    {'target': 'native', 'run': 'private-runtime', 'commit': 'c' * 40, 'base_commit': 'a' * 40})
        output = self.work / 'core'
        output.mkdir()
        projections = []
        for name, report, context in zip(('base', 'head'), reports, contexts):
            sources = output / (name + '-sources')
            sources.mkdir()
            shutil.copyfile(self.work / name / 'fixture.rs', sources / 'fixture.rs')
            projections.append(policy.project(report, output / name, sources, context, 'private-runtime', []))
        declared, mappings, _ = policy.policy_and_lineage(base, head, *projections, [])
        command = [str(Path(binary).resolve()), 'quality', 'evaluate', '--output', str(output / 'report.json')]
        for name, value in [('policy', declared), ('mappings', mappings)]:
            path = output / (name + '.json')
            path.write_text(json.dumps(value))
            command.extend(['--' + name, str(path)])
        for prefix, projection in zip(('base-', ''), projections):
            for name, value in projection.items():
                path = value if name.endswith('-root') else str(output / (prefix + name + '.json'))
                if not name.endswith('-root'):
                    Path(path).write_text(json.dumps(value))
                command.extend(['--' + prefix + name, path])
        result = self.command('core-evaluate', command)
        self.assertEqual(result.returncode, 1, result.stderr)
        report = json.loads((output / 'report.json').read_text())
        self.assertEqual(report['aggregate']['state'], 'fail')
        self.assertNotIn('measurement_error', {r['state'] for r in report['gates'].values()})
        self.assertTrue(any(r['record']['head'].get('numerator') == 56 for r in report['gates'].values()))
