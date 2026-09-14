#!/usr/bin/env python3
"""Mandatory real-binary acceptance. Missing tools or skipped tests fail the run."""
import copy
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

HERE = Path(__file__).resolve().parent
QUALITY = HERE.parent
sys.path[:0] = [str(QUALITY), str(QUALITY / 'tests')]
import rust_native_driver as native
from test_rust_collector_project import binding_for, request_for
from rust_collector_config_fixture import sign


class ExternalPluginTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.binary = Path(os.environ['NATIVE_PLUGIN_BINARY']).resolve(strict=True)
        cls.sysroot = Path(os.environ['NATIVE_DRIVER_SYSROOT']).resolve(strict=True)
        cls.core = Path(os.environ['HARNESS_GATE_NATIVE_POLICY_BINARY']).resolve(strict=True)
        cls.version = subprocess.check_output([str(cls.binary), '--version'], text=True).split()[1]
        parent = QUALITY.parents[1] / 'target/native-acceptance'
        parent.mkdir(parents=True, exist_ok=True)
        cls.work = Path(tempfile.mkdtemp(dir=parent))
        # The copied executable runs outside the source tree with isolated Python.
        cls.binary = Path(shutil.copy2(cls.binary, native.executable(cls.work, 'collector')))
        cls.environment = os.environ | {'XDG_CACHE_HOME': str(cls.work / 'cache'),
                                        'HARNESS_GATE_PYTHON': sys.executable}
        cls.environment.pop('HARNESS_GATE_NATIVE_CACHE', None)
        cls.head = cls.work / 'head'
        result = cls.command('fixture', [str(cls.binary), 'fixture', '--sysroot', str(cls.sysroot),
            '--source', str(QUALITY / 'fixtures/rust-native/driver_complete.rs'), '--output', str(cls.head)])
        if result.returncode:
            raise RuntimeError(result.stderr)
        cls.anchor = json.loads(result.stdout)['anchor']
        result = cls.command('certify', [str(cls.binary), 'certify', '--sysroot', str(cls.sysroot),
            '--evidence', str(cls.head), '--anchor', cls.anchor])
        if result.returncode:
            raise RuntimeError(result.stderr)
        cls.report = json.loads(result.stdout)['measurement']
        cls.payload = next((cls.work / 'cache/harness-gate/native').iterdir())

    @classmethod
    def command(cls, name, args, data=None, env=None):
        result = subprocess.run(args, cwd=cls.work, env=env or cls.environment, input=data,
                                capture_output=True, text=True, timeout=300)
        (cls.work / (name + '.stdout')).write_text(result.stdout)
        (cls.work / (name + '.stderr')).write_text(result.stderr)
        return result

    def entry(self, name, *args, env=None):
        return self.command(name, [str(self.binary), *map(str, args)], env=env)

    def test_old_engine_equivalence_and_generated_owners(self):
        old = native.certify(self.head, self.anchor)
        old.pop('passed', None)
        for row in old['functions']:
            row.pop('passed', None)
        self.assertEqual(self.report, old)
        rows = {row['name']: row for row in self.report['functions']}
        self.assertEqual(rows['legacy_debt']['crap_exact'], [56, 1])
        self.assertEqual(rows['future']['blocks'][0], 1)
        self.assertEqual(rows['future::{closure#0}']['blocks'][0], 0)
        self.assertGreater(rows['closure::{closure#0}']['blocks'][0], 0)
        self.assertTrue(any('clone' in name for name in rows))
        self.assertNotEqual(rows['first']['definition']['expansion'], rows['second']['definition']['expansion'])
        self.assertEqual(len(rows['generic']['instances']), 2)
        self.assertFalse(self.report['backend_complete'])
        self.assertTrue(self.report['mapping_complete_for_declared_scope'])

    def test_metadata_needs_no_python_or_cache(self):
        cache = self.work / 'metadata-must-not-create-cache'
        environment = self.environment | {'HARNESS_GATE_NATIVE_CACHE': str(cache),
                                           'HARNESS_GATE_PYTHON': str(self.work / 'absent-python')}
        for option in ('--version', '--licenses'):
            result = self.entry('metadata-' + option[2:], option, env=environment)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertTrue(result.stdout)
            self.assertFalse(cache.exists())

    def test_real_cargo_capture_and_features(self):
        captures = []
        for features in ([], ['--feature', 'extra']):
            name = 'cargo-extra' if features else 'cargo'
            result = self.entry(name, 'capture', '--sysroot', self.sysroot,
                '--source', QUALITY / 'fixtures/rust-native/cargo-complete/Cargo.toml',
                '--sample', 'contract', '--output', self.work / name, *features)
            self.assertEqual(result.returncode, 0, result.stderr)
            capture = json.loads(result.stdout)
            captures.append(capture)
            result = self.entry(name + '-certify', 'certify', '--sysroot', self.sysroot,
                '--evidence', capture['capture'], '--anchor', capture['anchor'])
            self.assertEqual(result.returncode, 0, result.stderr)
            report = json.loads(result.stdout)['measurement']
            self.assertTrue(report['backend_complete'])
            names = {row['name'] for row in report['functions']}
            self.assertIn('production', names)
            self.assertNotIn('project_owned_contract', names)
            self.assertEqual(any(n.endswith('cfg_inactive') for n in names), bool(features))
        common = ['classify', '--sysroot', self.sysroot, '--evidence', captures[0]['capture'],
                  '--anchor', captures[0]['anchor']]
        result = self.entry('classify-incomplete', *common)
        self.assertEqual(result.returncode, 1, result.stderr)
        report = json.loads(result.stdout)
        self.assertFalse(report['classification_complete'])
        self.assertTrue(any(row['reason'] == 'source_not_loaded_without_selection_proof' for row in report['files']))
        result = self.entry('classify-witness', *common, '--witness', captures[1]['capture'],
                            '--witness-anchor', captures[1]['anchor'])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(json.loads(result.stdout)['classification_complete'])

    def test_wrong_missing_tools_and_overrides_block(self):
        cases = [('no-sysroot', ['doctor'], 'select external Rust'),
                 ('missing-sysroot', ['doctor', '--sysroot', str(self.work / 'absent')], 'measurement_error'),
                 ('wrong-sysroot', ['doctor', '--sysroot', str(self.work)], 'missing external rustc')]
        for name, args, message in cases:
            env = self.environment.copy()
            env.pop('HARNESS_GATE_RUST_SYSROOT', None)
            result = self.entry(name, *args, env=env)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(message, result.stderr)
        result = self.entry('bad-python', 'doctor', env=self.environment | {'HARNESS_GATE_PYTHON': '/absent/python'})
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('external Python', result.stderr)
        result = self.entry('override', 'capture', '--sysroot', self.sysroot,
            '--source', QUALITY / 'fixtures/rust-native/cargo-complete/Cargo.toml',
            '--sample', 'contract', '--output', self.work / 'override',
            env=self.environment | {'RUSTFLAGS': '-C opt-level=3'})
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('unset inherited compiler override', result.stderr)
        self.assertFalse((self.work / 'override').exists())

    def test_packaged_evaluate_retains_core_debt_rules(self):
        context = {'target': 'native', 'run': 'packaged-policy', 'commit': 'a' * 40, 'base_commit': 'b' * 40}
        for name, value in (('base-context', context),
                            ('head-context', context | {'commit': 'c' * 40, 'base_commit': 'a' * 40}),
                            ('hotspots', [])):
            (self.work / (name + '.json')).write_text(json.dumps(value))
        output = self.work / 'packaged-policy'
        result = self.entry('packaged-policy', 'evaluate', '--sysroot', self.sysroot,
            '--base', self.head, '--base-anchor', self.anchor, '--head', self.head, '--head-anchor', self.anchor,
            '--output', output, '--harness-gate', self.core, '--project', 'native-fixture',
            '--base-context', self.work / 'base-context.json', '--head-context', self.work / 'head-context.json',
            '--hotspots', self.work / 'hotspots.json')
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(json.loads(result.stdout)['state'], 'fail')
        self.assertNotIn('measurement_error', result.stderr)

    def test_modified_cache_is_never_used_or_repaired(self):
        path = self.payload / 'app/native_external.py'
        original = path.read_bytes()
        for mode in ('bytes', 'extra', 'symlink', 'permissions'):
            with self.subTest(mode=mode):
                extra = self.payload / 'app/unexpected.py'
                if mode == 'bytes':
                    path.write_bytes(original + b'\n')
                elif mode == 'extra':
                    extra.write_text('raise RuntimeError("must never run")\n')
                elif mode == 'symlink':
                    extra.symlink_to(path)
                else:
                    path.chmod(0o444)
                try:
                    result = self.entry('bad-cache-' + mode, 'doctor', '--sysroot', self.sysroot)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn('native collector:', result.stderr)
                finally:
                    path.chmod(0o644)
                    path.write_bytes(original)
                    extra.unlink(missing_ok=True)

    def test_missing_corrupt_evidence_and_wrong_anchor_block(self):
        path = self.head / 'native-export.stdout'
        original = path.read_bytes()
        for mode in ('anchor', 'missing', 'corrupt'):
            if mode == 'missing':
                path.unlink()
            elif mode == 'corrupt':
                path.write_bytes(original + b' ')
            try:
                result = self.entry('bad-evidence-' + mode, 'certify', '--sysroot', self.sysroot,
                    '--evidence', self.head, '--anchor', '0' * 64 if mode == 'anchor' else self.anchor)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('measurement_error', result.stderr)
                self.assertEqual(result.stdout, '')
            finally:
                path.write_bytes(original)
        native.verified_files(self.head, self.anchor)
        output = self.head / 'unsealed-report.json'
        result = self.entry('output-inside-evidence', 'certify', '--sysroot', self.sysroot,
            '--evidence', self.head, '--anchor', self.anchor, '--output', output)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('outside retained evidence', result.stderr)
        self.assertFalse(output.exists())
        native.verified_files(self.head, self.anchor)

    def test_core_authenticates_collect_and_rejects_tamper_and_replay(self):
        output = self.work / 'core'
        output.mkdir()
        artifacts = output / 'artifacts'
        artifacts.mkdir()
        binding = binding_for(self.report, self.head, artifacts, self.version)
        request, _ = request_for(binding, output / 'binding.json')
        request['args'] += ['--sysroot', str(self.sysroot)]
        request['adapter'].update(executable=str(self.binary), source_digest=native.file_hash(self.binary),
            signature={'algorithm': 'ed25519', 'key_id': 'configured-test', 'value': ''})
        request.update(nonce='native-external-positive', timeout_ms=120000, issued_at_ms=int(time.time() * 1000))
        request['expires_at_ms'] = request['issued_at_ms'] + 240000
        # Core reserves HARNESS_GATE_*; its authenticated environment uses
        # standard PATH/XDG options and its own invocation marker variables.
        request['environment'] = {'PATH': str(Path(sys.executable).parent) + os.pathsep + os.environ['PATH'],
                                  'XDG_CACHE_HOME': str(self.work / 'cache')}
        if os.name == 'nt':
            request['environment']['SystemRoot'] = os.environ['SystemRoot']
        request['capabilities']['environment'] = sorted(request['environment'])
        trusted = sign(self, output, request)
        single = output / 'trusted-key.json'
        single.write_text(json.dumps(json.loads(trusted.read_text())[0]))
        command = [str(self.core), 'adapter', 'run', '--trusted-key', str(single)]
        for name in request['environment']:
            command += ['--allow-environment', name]
        path = output / 'request.json'
        tampered = copy.deepcopy(request)
        tampered['args'][-1] = '/untrusted/toolchain'
        path.write_text(json.dumps(tampered))
        result = self.command('core-tamper', command + ['--request', str(path)])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('signature verification failed', result.stderr)
        self.assertEqual(list(artifacts.iterdir()), [])
        path.write_text(json.dumps(request))
        result = self.command('core-collect', command + ['--request', str(path)])
        self.assertEqual(result.returncode, 0, result.stderr)
        response = json.loads(result.stdout)
        self.assertTrue(response['collection']['evidence'])
        self.assertEqual(len(response['collection']['evidence']), len(self.report['functions']))
        result = self.command('core-replay', command + ['--request', str(path)])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('nonce has already been used', result.stderr)


if __name__ == '__main__':
    for name in ('NATIVE_PLUGIN_BINARY', 'NATIVE_DRIVER', 'NATIVE_DRIVER_SYSROOT', 'HARNESS_GATE_NATIVE_POLICY_BINARY'):
        if not os.environ.get(name):
            raise SystemExit('required acceptance input is missing: ' + name)
    if not shutil.which('openssl'):
        raise SystemExit('openssl is required for real Core signature verification')
    os.environ['HARNESS_GATE_LEGACY_EXPERIMENT'] = '1'
    from test_rust_native_driver import NativeDriverTests
    suite = unittest.TestSuite(unittest.defaultTestLoader.loadTestsFromTestCase(case)
        for case in (ExternalPluginTests, NativeDriverTests))
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    print('Retained acceptance artifacts:', ExternalPluginTests.work)
    raise SystemExit(0 if result.wasSuccessful() and not result.skipped else 1)
