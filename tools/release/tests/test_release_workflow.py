"""Regression checks for the tag-triggered release workflow boundaries."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


WORKFLOW = Path(__file__).parents[3] / ".github" / "workflows" / "release.yml"


class ReleaseWorkflowTests(unittest.TestCase):
    def setUp(self) -> None:
        self.source = WORKFLOW.read_text(encoding="utf-8")

    def test_collector_packages_require_acceptance_and_inventory(self) -> None:
        self.assertIn("needs: [policy, build, quality, collectors]", self.source)
        self.assertIn("package_independent_collectors.py --output collector-dist", self.source)
        for package in (
            "harness-gate-typescript-collector-0.1.0-rc.4.tgz",
            "harness-gate-http-json-contract-collector-0.1.0-rc.5.tgz",
            "harness-gate-rust-source-risk-0.1.0-rc.6-linux-amd64.tar.gz",
        ):
            self.assertIn("--package " + package, self.source)
            self.assertIn("--sbom " + package + ".sbom.cdx.json", self.source)

    def test_publication_download_excludes_policy_evidence(self) -> None:
        """Only build-matrix artifacts may be copied into the release dist."""

        self.assertIn("pattern: release-harness-gate-*", self.source)
        self.assertNotIn("pattern: release-*\n", self.source)
        self.assertIn("name: release-policy-${{ github.run_id }}", self.source)
        self.assertIn("name: release-${{ matrix.asset_name }}", self.source)

        download_block = re.search(
            r"- name: Download release assets(?P<block>.*?)(?=\n\s*- name: Set up Python)",
            self.source,
            flags=re.DOTALL,
        )
        self.assertIsNotNone(download_block)
        assert download_block is not None
        self.assertNotIn("release-policy", download_block.group("block"))


if __name__ == "__main__":
    unittest.main()
