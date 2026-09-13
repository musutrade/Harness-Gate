"""Real RSA lifecycle tests; synthetic payloads do not certify native portability."""
import copy
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import test_collector_delivery as fixtures
import collector_assets as assets
import build_user_installer as builder
import install_collector as lifecycle
from test_rust_runtime_requirements import requirements


class DependencyInstallTests(unittest.TestCase):
    setUpClass = classmethod(fixtures.DeliveryTests.setUpClass.__func__)
    tearDownClass = classmethod(fixtures.DeliveryTests.tearDownClass.__func__)
    setUp = fixtures.DeliveryTests.setUp
    make_archive = fixtures.DeliveryTests.make_archive
    sign = fixtures.DeliveryTests.sign
    resign_inventory = fixtures.DeliveryTests.resign_inventory

    def portable(self):
        self.manifest['schema'] = 'rust-collector-delivery/v3'
        self.manifest['runtime_requirements'] = requirements()
        measurement = self.manifest['measurement']
        measurement.pop('configuration_sha256')
        measurement.pop('normalized_series')
        measurement['configuration_authority'] = 'quality-trusted-state/v1'
        self.manifest['host_abi'].update(glibc='glibc 2.43', kernel='publisher-only')
        self.sign()

    def test_signed_install_accepts_compatible_different_host_and_preserves_payload_checks(self):
        self.portable()
        with patch.object(assets, 'probe_host', side_effect=AssertionError('legacy host probe used')), \
                patch.object(assets.contract.dependencies.os, 'confstr', return_value='glibc 2.39'):
            lifecycle.install(self.root, self.release, self.trust, self.tag)
            tool = self.root / 'versions/0.1.0-rc.1' / self.manifest['tools'][0]['path']
            tool.write_bytes(b'tampered')
            with self.assertRaises(ValueError):
                lifecycle.select(self.root, '0.1.0-rc.1', self.trust)

    def test_signed_requirements_cannot_be_changed_without_authentication(self):
        self.portable()
        changed = copy.deepcopy(self.manifest)
        changed['runtime_requirements']['libc_min'] = '2.17'
        assets.write(self.release / 'manifest.json', changed)
        with self.assertRaises(ValueError):
            lifecycle.install(self.root, self.release, self.trust, self.tag)
        self.assertFalse((self.root / 'current.json').exists())

    def test_missing_dependency_stops_activation(self):
        self.portable()
        with patch.object(assets.contract.dependencies.ctypes, 'CDLL', side_effect=OSError('missing libc')):
            with self.assertRaisesRegex(ValueError, 'dependency missing/incompatible'):
                lifecycle.install(self.root, self.release, self.trust, self.tag)
        self.assertFalse((self.root / 'current.json').exists())

    def test_shell_preflight_uses_version_bounds_without_c_or_openssl(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            probe = directory / 'getconf'
            script = 'offline=1; policy=deny\n' + builder.dependency_preflight(requirements())
            for version, ok in (('2.39', True), ('2.100', True), ('2.37', False)):
                probe.write_text('#!/bin/sh\necho "glibc ' + version + '"\n')
                probe.chmod(0o755)
                result = subprocess.run(['/bin/bash', '-c', script], text=True, capture_output=True,
                                        env={**os.environ, 'PATH': str(directory) + ':/usr/bin:/bin'})
                with self.subTest(version=version):
                    self.assertEqual(result.returncode == 0, ok, result.stderr)
                    if not ok:
                        self.assertIn('glibc >= 2.38', result.stderr)
            self.assertNotIn('command -v openssl', script)
            self.assertNotIn('command -v cc', script)


if __name__ == '__main__':
    unittest.main()
