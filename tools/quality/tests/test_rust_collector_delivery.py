"""Delivery preflight failures must occur before execution or re-export."""
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import rust_collector_delivery as delivery
import rust_native_driver as native


class ObservedDeliveryTests(unittest.TestCase):
    def test_wrong_core_bytes_are_rejected_before_executing_them(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / 'core'
            binary.write_bytes(b'untrusted executable')
            with patch.object(delivery, 'version') as version, self.assertRaisesRegex(ValueError, 'Core bytes'):
                delivery.observe(root, binary, {'sha256': '0' * 64})
            version.assert_not_called()

    def test_pinned_metadata_rejects_changed_duplicate_and_symlink_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'metadata.json'
            path.write_text('{"key":1}')
            ref = {'path': str(path), 'sha256': native.file_hash(path)}
            self.assertEqual(delivery.pinned(ref), {'key': 1})
            path.write_text('{"key":2}')
            with self.assertRaisesRegex(ValueError, 'digest mismatch'):
                delivery.pinned(ref)
            path.write_text('{"key":1,"key":2}')
            ref['sha256'] = native.file_hash(path)
            with self.assertRaises(ValueError):
                delivery.pinned(ref)
            link = Path(directory) / 'link'
            link.symlink_to(path)
            with self.assertRaisesRegex(ValueError, 'unsafe'):
                delivery.pinned(dict(ref, path=str(link)))

    def test_unconfigured_delivery_does_not_probe_or_certify(self):
        with patch.object(delivery, 'observe') as observe, patch.object(native, 'certify') as certify:
            with self.assertRaisesRegex(ValueError, 'unknown tested'):
                delivery.preflight(Path('/unused'), {})
            observe.assert_not_called()
            certify.assert_not_called()

    def test_relocated_capture_is_not_series_equivalence(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'capture.json').write_text(json.dumps({'tools': {'rustc': {'path': '/another/rustc'}}}))
            with self.assertRaisesRegex(ValueError, 'no relocation equivalence'):
                delivery.capture_paths(root, root)
