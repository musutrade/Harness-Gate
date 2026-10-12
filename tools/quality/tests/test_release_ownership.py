"""Every release script has one owning repository; shared tools have one source."""
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
MANIFEST = ROOT / "docs/quality/release-ownership.json"
RELEASE = ROOT / "tools/release"


class ReleaseOwnershipTests(unittest.TestCase):
    def setUp(self):
        self.manifest = json.loads(MANIFEST.read_text())
        self.owners = {name: list(paths) for name, paths in self.manifest["owners"].items()}
        self.shared = {str(k): str(v) for k, v in self.manifest["shared"].items()}

    def test_every_entry_has_exactly_one_owner(self):
        seen = {}
        for owner, paths in self.owners.items():
            for path in paths:
                self.assertNotIn(path, seen, path)
                seen[path] = owner
        for path, owner in seen.items():
            self.assertIn(owner, {"gatehouse", "gateprobes"})
            self.assertTrue((ROOT / path).exists(), path)
        for path, owner in self.shared.items():
            self.assertNotIn(path, seen, path)
            self.assertEqual(owner, "gatehouse", path)
            self.assertTrue((ROOT / path).exists(), path)

    def test_every_release_file_is_classified(self):
        classified = {path for paths in self.owners.values() for path in paths}
        classified |= set(self.shared)
        classified = {path for path in classified if path.startswith("tools/release/")}
        actual = {
            child.relative_to(ROOT).as_posix()
            for child in RELEASE.iterdir()
            if child.is_file() and not child.name.startswith(".")
        }
        self.assertEqual(classified, actual)

    def test_core_release_workflow_and_install_entry_stay_with_gatehouse(self):
        self.assertIn(".github/workflows/release.yml", self.owners["gatehouse"])
        self.assertIn("install.sh", self.owners["gatehouse"])

    def test_plugin_identity_and_shared_copy_rules_are_declared(self):
        self.assertTrue(self.manifest["plugin_release_identity"].strip())
        self.assertTrue(self.manifest["shared_copy_rule"].strip())
        self.assertEqual(self.manifest["generated_copies"], {})


if __name__ == "__main__":
    unittest.main()
