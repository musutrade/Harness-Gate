"""The native tag route must retain Core's publication trust boundary."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[3]


class NativeReleaseWorkflowTests(unittest.TestCase):
    def test_native_publication_requires_eligibility_and_actual_binary_acceptance(self):
        source = (ROOT / '.github/workflows/native-collector-release.yml').read_text()
        policy, build, publish = source.split('  policy:\n')[1].split('  build:\n')[0], \
            source.split('  build:\n')[1].split('  publish:\n')[0], source.split('  publish:\n')[1]
        self.assertIn('--tag-prefix rust-collector-v', policy)
        self.assertIn('--manifest tools/quality/rust-native-plugin/Cargo.toml', policy)
        self.assertIn('refs/remotes/origin/main', policy)
        self.assertIn("needs.policy.result == 'success'", build)
        self.assertIn("!startsWith(github.ref, 'refs/tags/')", build)
        self.assertIn('python tools/quality/rust-native-plugin/accept.py', build)
        self.assertIn('needs: [policy, build]', publish)
        self.assertIn('environment: release', publish)
        self.assertIn("if: startsWith(github.ref, 'refs/tags/rust-collector-v')", publish)
        self.assertIn('name: native-release-assets-${{ github.run_id }}', publish)
        self.assertNotIn('native-acceptance-', publish)
        for operation in ('generate', 'checksums', 'list', '"${verify_args[@]}"'):
            self.assertIn('tools/release/release_inventory.py ' + operation, publish)
        self.assertIn('--certificate-identity "https://github.com/${GITHUB_REPOSITORY}/.github/workflows/native-collector-release.yml@${GITHUB_REF}"', publish)
        self.assertNotIn('--certificate-identity-regexp', publish)
        self.assertIn('gh attestation verify', publish)
        self.assertIn('refusing to overwrite it', publish)
        self.assertNotIn('publish-crate', source)


if __name__ == '__main__':
    unittest.main()
