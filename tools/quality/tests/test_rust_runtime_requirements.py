"""Dependency compatibility preserves authentication/tool identity, not host sameness."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import rust_runtime_requirements as dependencies
import rust_collector_contract as contract
import rust_collector_entry as entry
from test_rust_collector_contract import fixture


def requirements():
    return {'schema': dependencies.SCHEMA, 'system': 'Linux', 'machine': 'x86_64',
            'libc_min': '2.38', 'libraries': ['libc.so.6', 'libm.so.6'], 'commands': ['sh']}


def portable_fixture():
    manifest, matrix, observed = fixture()
    manifest['schema'] = 'rust-collector-delivery/v3'
    manifest['runtime_requirements'] = requirements()
    identity = manifest['measurement']
    identity.pop('configuration_sha256')
    identity.pop('normalized_series')
    identity['configuration_authority'] = 'quality-trusted-state/v1'
    for abi in (manifest['host_abi'], observed['host_abi'], matrix['tested'][0]['environment']['host_abi']):
        abi['glibc'] = 'glibc 2.43'
    matrix['tested'][0]['manifest_sha256'] = contract.fingerprint(manifest)
    return manifest, matrix, observed


class RuntimeDependencyTests(unittest.TestCase):
    def test_newer_or_different_kernel_and_library_fingerprint_are_not_rejections(self):
        manifest, matrix, observed = portable_fixture()
        observed['host_abi'].update(glibc='glibc 2.39', kernel='6.8.0-security-update',
                                    runtime_dependencies_sha256='e' * 64)
        self.assertEqual(contract.preflight(manifest, matrix, observed), matrix['tested'][0]['receipt'])
        observed['host_abi']['glibc'] = 'glibc 2.100'
        self.assertEqual(contract.preflight(manifest, matrix, observed), matrix['tested'][0]['receipt'])

    def test_unsupported_dependencies_and_architecture_still_fail(self):
        for update in ({'glibc': 'glibc 2.37'}, {'glibc': 'musl 1.2'}, {'target': 'aarch64-unknown-linux-gnu'}):
            manifest, matrix, observed = portable_fixture()
            observed['host_abi'].update(update)
            with self.subTest(update=update), self.assertRaises(ValueError):
                contract.preflight(manifest, matrix, observed)

    def test_manifest_receipt_core_protocol_and_tool_integrity_still_fail_closed(self):
        for mutation in ('tool', 'core', 'protocol', 'manifest', 'missing-receipt', 'duplicate'):
            manifest, matrix, observed = portable_fixture()
            if mutation == 'tool': observed['tools'][0]['sha256'] = 'f' * 64
            elif mutation == 'core': observed['core']['sha256'] = 'f' * 64
            elif mutation == 'protocol': observed['protocol']['request'] = 'unknown'
            elif mutation == 'manifest': manifest['runtime_requirements']['libc_min'] = '2.37'
            elif mutation == 'missing-receipt': matrix['tested'].clear()
            else: matrix['tested'].append(copy.deepcopy(matrix['tested'][0]))
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                contract.preflight(manifest, matrix, observed)

    def test_missing_requirements_do_not_fall_back_to_host_equality(self):
        manifest, matrix, observed = portable_fixture()
        manifest.pop('runtime_requirements')
        with self.assertRaises(ValueError): contract.preflight(manifest, matrix, observed)
        manifest, _, _ = fixture()
        manifest['runtime_requirements'] = requirements()
        with self.assertRaises(ValueError): contract.validate_manifest(manifest)

    def test_version_inventory_ignores_exported_glibc_definitions(self):
        output = '''(NEEDED) Shared library: [libc.so.6]
Version definition section '.gnu.version_d'
Name: GLIBC_2.43
Version needs section '.gnu.version_r'
Name: GLIBC_2.38
Name: GLIBC_2.17
'''
        libraries, versions = dependencies.elf_requirements(output)
        self.assertEqual(libraries, ['libc.so.6'])
        self.assertEqual(versions, ['2.38', '2.17'])
        self.assertEqual(max(versions, key=dependencies.version), '2.38')

    def test_missing_library_and_shell_report_actual_dependency(self):
        with patch.object(dependencies.platform, 'system', return_value='Linux'), \
                patch.object(dependencies.platform, 'machine', return_value='x86_64'), \
                patch.object(dependencies.os, 'confstr', return_value='glibc 2.39'), \
                patch.object(dependencies.shutil, 'which', return_value='/bin/sh'):
            with patch.object(dependencies.ctypes, 'CDLL', side_effect=OSError('not found')):
                with self.assertRaisesRegex(ValueError, 'libc.so.6'):
                    dependencies.probe(requirements())
            with patch.object(dependencies.shutil, 'which', return_value=None):
                with self.assertRaisesRegex(ValueError, 'POSIX sh'):
                    dependencies.probe(requirements())

    def test_runtime_doctor_uses_requirements_but_still_hashes_payload(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'tool').write_bytes(b'original')
            inventory = {'schema': 'rust-collector-runtime/2', 'host': {'kernel': 'other-machine'},
                         'runtime_requirements': requirements(),
                         'payload': {'tool': {'sha256': entry.native.file_hash(root / 'tool')}}}
            (root / 'runtime.json').write_text(json.dumps(inventory))
            with patch.object(dependencies, 'probe', return_value={'kernel': 'new-host'}):
                self.assertTrue(entry.doctor(root)['runtime_complete'])
                (root / 'tool').write_bytes(b'tampered')
                with self.assertRaisesRegex(ValueError, 'missing/modified private runtime'):
                    entry.doctor(root)

    def test_library_patch_changes_capture_fingerprint_without_rejecting_installation(self):
        with tempfile.TemporaryDirectory() as temporary:
            paths = [Path(temporary) / name for name in requirements()['libraries']]
            for path in paths:
                path.write_bytes(b'original library')
            maps = '\n'.join('0-1 r-xp 0 00:00 1 ' + str(path) for path in paths)
            with patch.object(dependencies.ctypes, 'CDLL'), \
                    patch.object(dependencies.Path, 'read_text', return_value=maps), \
                    patch.object(dependencies.os, 'confstr', return_value='glibc 2.39'):
                before = dependencies.probe(requirements())
                paths[0].write_bytes(b'security update, same glibc version')
                after = dependencies.probe(requirements())
            self.assertEqual(before['glibc'], after['glibc'])
            self.assertNotEqual(before['runtime_dependencies_sha256'], after['runtime_dependencies_sha256'])

    def test_empty_unknown_or_malformed_dependency_contracts_are_rejected(self):
        for update in ({'libraries': []}, {'commands': []}, {'libc_min': '2.38;true'},
                       {'libraries': ['../../bad']}, {'schema': 'unknown'}, {'system': 'Windows'}):
            value = requirements() | update
            with self.subTest(update=update), self.assertRaises(ValueError): dependencies.validate(value)


if __name__ == '__main__':
    unittest.main()
