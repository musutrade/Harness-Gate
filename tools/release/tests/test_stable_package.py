"""Development tests for preparation input authentication, not host acceptance."""
import hashlib
import io
import json
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'tools/quality/rust-stable-collector'))
from prepare_release import acceptance, identity, registry_notices, strict_json


class PreparationTests(unittest.TestCase):
    def package(self, root, files=None, extra_member=None):
        files = files if files is not None else {'Cargo.toml': '[package]\nname="example"\n', 'LICENSE': 'test license\n'}
        source = root / 'registry/src/index/example-1.0.0'
        source.mkdir(parents=True)
        archive = root / 'registry/cache/index/example-1.0.0.crate'
        archive.parent.mkdir(parents=True)
        with tarfile.open(archive, 'w:gz') as tar:
            for name, text in files.items():
                (source / name).write_text(text)
                member = tarfile.TarInfo('example-1.0.0/' + name)
                data = text.encode()
                member.size = len(data)
                tar.addfile(member, io.BytesIO(data))
            if extra_member:
                member = tarfile.TarInfo(extra_member)
                tar.addfile(member, io.BytesIO(b''))
        package = {'name': 'example', 'version': '1.0.0', 'id': 'example', 'license': 'MIT',
                   'source': 'registry+https://github.com/rust-lang/crates.io-index',
                   'manifest_path': str(source / 'Cargo.toml')}
        key = (package['name'], package['version'], package['source'])
        return source, archive, package, {key: {'checksum': identity(archive)['sha256']}}

    def test_locked_archive_matches_source_and_notices(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, _, package, lock = self.package(Path(temporary))
            (source / '.cargo-ok').write_text('cache marker')
            notices, record = registry_notices(package, lock)
            self.assertEqual(notices, {'LICENSE': 'test license\n'})
            self.assertEqual(record['notices']['LICENSE'], hashlib.sha256(b'test license\n').hexdigest())
            (source / 'Cargo.toml').write_text('modified source')
            with self.assertRaisesRegex(ValueError, 'cached source differs'):
                registry_notices(package, lock)

    def test_extra_source_and_symlink_fail(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, _, package, lock = self.package(Path(temporary))
            (source / 'undeclared.rs').write_text('new build input')
            with self.assertRaisesRegex(ValueError, 'undeclared cached crate'):
                registry_notices(package, lock)
            (source / 'undeclared.rs').unlink()
            (source / 'alias').symlink_to(source / 'LICENSE')
            with self.assertRaisesRegex(ValueError, 'cached crate symlink'):
                registry_notices(package, lock)

    def test_archive_checksum_and_traversal_fail(self):
        with tempfile.TemporaryDirectory() as temporary:
            _, archive, package, lock = self.package(Path(temporary), extra_member='example-1.0.0/../escape')
            with self.assertRaisesRegex(ValueError, 'unsafe crate member'):
                registry_notices(package, lock)
            archive.write_bytes(b'corrupt')
            with self.assertRaisesRegex(ValueError, 'crate checksum mismatch'):
                registry_notices(package, lock)

    def test_missing_license_and_unknown_origin_fail(self):
        with tempfile.TemporaryDirectory() as temporary:
            _, _, package, lock = self.package(Path(temporary), {'Cargo.toml': 'test'})
            with self.assertRaisesRegex(ValueError, 'missing crate license'):
                registry_notices(package, lock)
            package['source'] = 'git+https://example.invalid/source'
            with self.assertRaisesRegex(ValueError, 'unsupported release dependency'):
                registry_notices(package, lock)

    def test_acceptance_requires_anchor_same_binary_and_passed_checks(self):
        value = json.loads((ROOT / 'docs/quality/stable-rust-candidate-evidence/acceptance-modules.json').read_text())
        binary = value['binary']
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'summary.json'
            def save():
                path.write_text(json.dumps(value))
                return identity(path)['sha256']
            pin = save()
            self.assertEqual(acceptance(path, pin, binary)['acceptance_sha256'], pin)
            with self.assertRaisesRegex(ValueError, 'anchor mismatch'):
                acceptance(path, '0' * 64, binary)
            with self.assertRaisesRegex(ValueError, 'candidate binary'):
                acceptance(path, pin, dict(binary, bytes=0))
            for name in ('partial', 'certified-region-owners', 'duplicate-owner-region',
                         'owner-region-counter-disagreement', 'owner-region-summary-count',
                         'owner-region-summary-covered', 'negative-function-count', 'certified-function-owners',
                         'missing-llvm-owner', 'omitted-source-owners', 'changed-test-exclusions',
                         'total-lines-count', 'total-functions-covered', 'file-summary-disagreement',
                         'external-include-bytes', 'wrong-compiler-cwd', 'omitted-dep-info-producer',
                         'modules', 'certified-module-owners', 'missing-module-owner',
                         'duplicate-module-owner', 'module-parent-count-inheritance',
                         'restored-module-owners', 'module-attribute', 'describe-module-attribute',
                         'module-cfg', 'describe-module-cfg'):
                semantic = next(check for check in value['checks'] if check['name'] == name)
                semantic['name'] = 'unrelated-placeholder'
                with self.assertRaisesRegex(ValueError, 'incomplete or failed'):
                    acceptance(path, save(), binary)
                semantic['name'] = name
            value['checks'][0]['passed'] = False
            with self.assertRaisesRegex(ValueError, 'incomplete or failed'):
                acceptance(path, save(), binary)

    def test_duplicate_and_nonfinite_metadata_fail(self):
        for value in ('{"key":1,"key":2}', '{"key": NaN}'):
            with self.subTest(value=value), self.assertRaises(ValueError):
                strict_json(value)


if __name__ == '__main__':
    unittest.main()
