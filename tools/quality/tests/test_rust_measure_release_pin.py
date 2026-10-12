"""The core risk gate consumes a pinned, verifiable measurer release artifact."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import ci_quality

ROOT = Path(__file__).resolve().parents[3]


class MeasurerPinTests(unittest.TestCase):
    def test_pin_matches_the_plugin_identity_and_asset_name(self):
        pin = ci_quality.load_measurer_pin()
        plugin = json.loads((ROOT / 'tools/quality/rust-measure/plugin.json').read_text())
        for field in ('analyzer', 'version', 'rule', 'executable'):
            self.assertEqual(pin[field], plugin[field], field)
        self.assertEqual(pin['asset'],
                         'harness-gate-rust-measure-' + pin['version'] + '-linux-amd64.tar.gz')

    def test_verify_measurer_asset_accepts_only_the_pinned_digest(self):
        pin = ci_quality.load_measurer_pin()
        with tempfile.TemporaryDirectory() as directory:
            asset = Path(directory) / pin['asset']
            asset.write_bytes(b'pinned measurer bytes')
            with self.assertRaisesRegex(ValueError, 'digest mismatch'):
                ci_quality.verify_measurer_asset(asset, pin)
            pin['sha256'] = ci_quality.sha256(asset)
            ci_quality.verify_measurer_asset(asset, pin)

    def test_core_risk_stage_no_longer_builds_the_measurer_from_source(self):
        source = (ROOT / 'tools/quality/ci_quality.py').read_text()
        self.assertNotIn('rust-measure/Cargo.toml', source)
        self.assertNotIn("'analyzer-build'", source)
        self.assertIn("'measurer-download'", source)


if __name__ == '__main__':
    unittest.main()
