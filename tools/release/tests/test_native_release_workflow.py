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
        self.assertIn('pattern: native-release-assets-${{ github.run_id }}-*', publish)
        self.assertIn('merge-multiple: true', publish)
        self.assertNotIn('native-acceptance-', publish)
        for operation in ('generate', 'checksums', 'list', '"${verify_args[@]}"'):
            self.assertIn('tools/release/release_inventory.py ' + operation, publish)
        self.assertIn('--certificate-identity "https://github.com/${GITHUB_REPOSITORY}/.github/workflows/native-collector-release.yml@${GITHUB_REF}"', publish)
        self.assertNotIn('--certificate-identity-regexp', publish)
        self.assertIn('gh attestation verify', publish)
        self.assertIn('refusing to overwrite it', publish)
        self.assertNotIn('publish-crate', source)

    def test_each_core_platform_requires_its_own_native_acceptance_and_sbom(self):
        source = (ROOT / '.github/workflows/native-collector-release.yml').read_text()
        build = source.split('  build:\n')[1].split('  publish:\n')[0]
        publish = source.split('  publish:\n')[1]
        for target, asset in (
            ('x86_64-unknown-linux-gnu', 'harness-gate-rust-collector-linux-amd64'),
            ('x86_64-apple-darwin', 'harness-gate-rust-collector-macos-amd64'),
            ('aarch64-apple-darwin', 'harness-gate-rust-collector-macos-arm64'),
            ('x86_64-pc-windows-msvc', 'harness-gate-rust-collector-windows-amd64.exe'),
        ):
            self.assertIn('target: ' + target, build)
            self.assertIn('asset: ' + asset, build)
            self.assertIn('--binary ' + asset, publish)
            self.assertIn('--sbom ' + asset.removesuffix('.exe') + '.sbom.cdx.json', publish)
        self.assertIn('fail-fast: false', build)
        self.assertIn('--target ${{ matrix.target }}', build)
        self.assertIn('native-acceptance-${{ github.run_id }}-${{ matrix.target }}', build)


if __name__ == '__main__':
    unittest.main()
