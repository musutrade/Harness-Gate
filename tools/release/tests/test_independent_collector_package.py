"""Reject incomplete distribution bytes before any source-tree fallback."""
import importlib.util
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    'collector_package', Path(__file__).parents[1] / 'package_independent_collectors.py')
package = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(package)


class ExtractedPackageTests(unittest.TestCase):
    def test_missing_runtime_lockfile_blocks_the_archive(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            payload = root / 'collector-fixture'
            payload.mkdir()
            for name in ('plugin.py', 'measure.py', 'capture.py', 'plugin.json',
                         'inventory', 'LICENSE', 'THIRD_PARTY_NOTICES.txt'):
                (payload / name).write_text('fixture')
            archive = root / 'collector-fixture.tar.gz'
            with tarfile.open(archive, 'w:gz') as stream:
                stream.add(payload, arcname=payload.name)
            with patch.object(package, 'run') as run:
                with self.assertRaisesRegex(RuntimeError, 'ast/Cargo.lock'):
                    package.accept_rust_archive(archive, root, root / 'consumer')
                run.assert_not_called()
