"""Pinned setup must reject missing, failed, and incompatible tool evidence."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
ACTION = ROOT / '.github/actions/install-ci-tool'
PINS = {'cargo-nextest': '0.9.143', 'cargo-llvm-cov': '0.9.0', 'cargo-audit': '0.22.2'}


class ToolSetupTests(unittest.TestCase):
    def invoke(self, mode, tool, output='', status=0):
        with tempfile.TemporaryDirectory() as temp:
            cargo = Path(temp) / 'cargo'
            cargo.write_text('#!/bin/sh\nprintf "%s\\n" "$TEST_VERSION"\nexit "$TEST_STATUS"\n')
            cargo.chmod(0o755)
            env = dict(os.environ, PATH=temp + os.pathsep + os.environ['PATH'],
                       TEST_VERSION=output, TEST_STATUS=str(status))
            return subprocess.run(['bash', str(ACTION / 'tool.sh'), mode, tool],
                                  env=env, capture_output=True, text=True)

    def test_explicit_pins_and_effective_versions(self):
        for tool, version in PINS.items():
            with self.subTest(tool=tool):
                resolved = self.invoke('resolve', tool)
                self.assertEqual(resolved.returncode, 0, resolved.stderr)
                self.assertEqual(resolved.stdout, f'spec={tool}@{version}\n')
                result = self.invoke('verify', tool, f'{tool} {version} (build info)\nrelease: {version}')
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn(version, result.stdout)
        self.assertEqual(self.invoke('verify', 'cargo-audit', 'cargo-audit-audit 0.22.2').returncode, 0)

    def test_invalid_or_failed_version_evidence_blocks(self):
        for tool, version in PINS.items():
            for output, status in [('', 127), ('', 0), (f'{tool} 99.0.0', 0),
                                   (f'{tool} {version}-dev', 0),
                                   (f'wrong-tool {version}', 0),
                                   (f'{tool} {version}', 1)]:
                with self.subTest(tool=tool, output=output, status=status):
                    self.assertNotEqual(self.invoke('verify', tool, output, status).returncode, 0)

    def test_unknown_tool_or_operation_blocks(self):
        self.assertNotEqual(self.invoke('resolve', 'unknown').returncode, 0)
        self.assertNotEqual(self.invoke('unknown', 'cargo-nextest').returncode, 0)

    def test_workflows_require_verified_installation(self):
        action = (ACTION / 'action.yml').read_text()
        self.assertRegex(action, r'uses: taiki-e/install-action@[0-9a-f]{40}')
        self.assertIn("checksum: 'true'", action)
        self.assertIn('fallback: cargo-install', action)
        self.assertIn('steps.pin.outputs.spec', action)
        self.assertIn('tool.sh" verify "$CI_TOOL"', action)
        self.assertNotIn('continue-on-error', action)
        self.assertNotIn('if:', action)
        for name, count in [('ci.yml', 6), ('release.yml', 1), ('quality-baseline-refresh.yml', 1)]:
            workflow = (ROOT / '.github/workflows' / name).read_text()
            self.assertEqual(workflow.count('uses: ./.github/actions/install-ci-tool'), count)
            self.assertNotRegex(workflow, r'cargo install cargo-(nextest|llvm-cov|audit)')
        ci = (ROOT / '.github/workflows/ci.yml').read_text()
        self.assertIn('run: cargo audit --deny warnings', ci)
        self.assertIn('working-directory: tools/harness-gate', ci)
        self.assertNotIn('Cache cargo-audit', ci)
        release = (ROOT / '.github/workflows/release.yml').read_text()
        self.assertIn('cargo audit --file tools/harness-gate/Cargo.lock --deny warnings', release)
