from __future__ import annotations

import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from docs_consistency import unsupported_sandbox_claims


class DocsConsistencySandboxWordingTests(unittest.TestCase):
    def test_rejects_positive_os_sandbox_claim(self) -> None:
        failures = unsupported_sandbox_claims(
            "Harness-Gate enforces an operating-system sandbox for adapters "
            "and blocks all host network access."
        )
        self.assertTrue(failures)

    def test_rejects_complete_descendant_claim(self) -> None:
        failures = unsupported_sandbox_claims(
            "The host guarantees complete descendant isolation for every adapter."
        )
        self.assertTrue(failures)

    def test_accepts_current_bounded_wording(self) -> None:
        failures = unsupported_sandbox_claims(
            "The capability allowlist is a protocol-level declaration check, "
            "not an operating-system network, filesystem, resource, or process "
            "sandbox; process cleanup is best effort and is not proof of "
            "complete descendant containment. An OS-enforced sandbox is "
            "deferred to a separate platform-specific decision."
        )
        self.assertEqual(failures, [])


if __name__ == "__main__":
    unittest.main()
