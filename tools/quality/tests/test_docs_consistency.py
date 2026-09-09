from __future__ import annotations

import unittest
from pathlib import Path
import json
import subprocess
import sys
import tempfile
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from docs_consistency import unsupported_sandbox_claims
import docs_consistency as docs


class DocsExecutionTests(unittest.TestCase):
    def exercise(self, failure=""):
        calls = []
        roots = {}

        def execute(argv, **kwargs):
            self.assertEqual(argv[:6], ["cargo", "run", "--quiet", "--locked", "--manifest-path",
                                        str(docs.CRATE / "Cargo.toml")])
            self.assertEqual(argv[6], "--")
            root = Path(argv[argv.index("--project-root") + 1])
            if "init" in argv:
                preset = argv[argv.index("--preset") + 1]
                roots[root] = preset
                operation = f"init:{preset}"
            elif "migrate" in argv:
                operation = "migrate"
                if failure != "missing-secrets":
                    (root / ".harness-gate/secrets.toml").touch()
            elif "export" in argv:
                operation = "schema"
                content = (docs.ROOT / "schema/flow.schema.json").read_bytes()
                (root / "flow.schema.json").write_bytes(b"changed" if failure == "schema-drift" else content)
            else:
                operation = f"check:{roots.get(root, 'migration')}"
            calls.append(operation)
            return subprocess.CompletedProcess(argv, int(operation == failure))

        with tempfile.TemporaryDirectory() as directory, \
                patch.object(docs.subprocess, "run", side_effect=execute), \
                patch.object(docs, "metadata", return_value={}):
            output = Path(directory) / "report.json"
            if failure:
                with self.assertRaises(SystemExit):
                    docs.run(output)
            else:
                self.assertEqual(docs.run(output), 0)
            report = json.loads(output.read_text())
        return calls, report

    def test_all_presets_migration_and_schema_use_locked_cargo(self):
        calls, report = self.exercise()
        # Independent inventory: a newly added preset must also be explicitly reviewed.
        presets = ["angular-only", "angular-rust-postgres", "generic", "rust-api"]
        self.assertEqual(calls, [op for preset in presets for op in (f"init:{preset}", f"check:{preset}")]
                         + ["migrate", "check:migration", "schema"])
        self.assertEqual(report["status"], "pass")
        self.assertEqual(len(report["examples"]), 4)
        for field in ("language_docs_valid", "schema_synced", "machine_schema_valid",
                      "manifest_schema_valid", "registry_schema_valid"):
            self.assertTrue(report[field])
        self.assertEqual(report["link_failures"], [])
        for field in ("migration", "sandbox_wording", "engineering_policy"):
            self.assertEqual(report[field]["status"], "pass")

    def test_each_cli_failure_and_migration_schema_drift_still_blocks(self):
        for failure in ([op for preset in ("angular-only", "angular-rust-postgres", "generic", "rust-api")
                         for op in (f"init:{preset}", f"check:{preset}")]
                        + ["migrate", "check:migration", "missing-secrets", "schema", "schema-drift"]):
            with self.subTest(failure=failure):
                _, report = self.exercise(failure)
                self.assertEqual(report["status"], "fail")


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
