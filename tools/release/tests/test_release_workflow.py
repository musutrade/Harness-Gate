"""Regression checks for the tag-triggered release workflow boundaries."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


WORKFLOW = Path(__file__).parents[3] / ".github" / "workflows" / "release.yml"


class ReleaseWorkflowTests(unittest.TestCase):
    def setUp(self) -> None:
        self.source = WORKFLOW.read_text(encoding="utf-8")

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
