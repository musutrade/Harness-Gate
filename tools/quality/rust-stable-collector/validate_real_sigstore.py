#!/usr/bin/env python3
"""Actual pinned cosign cryptography plus candidate wrong-payload rejection.

The positive is an upstream cosign release, NOT a signed collector release.
No signing service is invoked. Candidate RSA is a local lifecycle test key.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess

from prepare_release import identity, write
from validate_stable_candidate import check_trace, trace_command


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('binary', 'cosign', 'trusted-root', 'upstream-bundle', 'lifecycle', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    binary, cosign, trusted_root, bundle, lifecycle = [getattr(args, n).resolve(strict=True)
        for n in ('binary', 'cosign', 'trusted_root', 'upstream_bundle', 'lifecycle')]
    output = args.output.absolute()
    output.mkdir(parents=True, exist_ok=False)
    # These are the independently retained provisioning pins, not fetched trust.
    assert identity(cosign)['sha256'] == '4629c757b7618056f8ddd7e2625ae9fdd94c0372a65049520bc7d9df9efc7f71'
    assert identity(trusted_root)['sha256'] == '6494e21ea73fa7ee769f85f57d5a3e6a08725eae1e38c755fc3517c9e6bc0b66'
    assert identity(bundle)['sha256'] == 'e16547fbee348eb23bd7e5a4d542b540395faea2e7bb1d18da01bbc3cc74d57d'
    records = []

    def run(name, argv, success):
        trace = output / (name + '.execve')
        result = subprocess.run(trace_command(trace, argv), capture_output=True, timeout=80)
        (output / (name + '.stdout')).write_bytes(result.stdout)
        (output / (name + '.stderr')).write_bytes(result.stderr)
        check_trace(trace)
        assert (result.returncode == 0) == success, (name, result.stderr[-3000:])
        records.append({'name': name, 'argv': list(map(str, argv)), 'exit_code': result.returncode,
                        'trace': identity(trace), 'stderr': identity(output / (name + '.stderr'))})

    def upstream(path, identity_name='keyless@projectsigstore.iam.gserviceaccount.com'):
        return [str(cosign), 'verify-blob', '--bundle', str(path), '--trusted-root', str(trusted_root),
                '--offline', '--certificate-identity', identity_name, '--certificate-oidc-issuer',
                'https://accounts.google.com', str(cosign)]

    run('upstream-positive', upstream(bundle), True)
    run('wrong-identity', upstream(bundle, 'wrong@example.invalid'), False)
    original = json.loads(bundle.read_bytes())
    for name in ('tampered-signature', 'missing-transparency'):
        modified = json.loads(bundle.read_bytes())
        if name == 'tampered-signature':
            signature = modified['messageSignature']['signature']
            modified['messageSignature']['signature'] = ('A' if signature[0] != 'A' else 'B') + signature[1:]
        else:
            modified['verificationMaterial']['tlogEntries'] = []
            modified['verificationMaterial'].pop('timestampVerificationData', None)
        path = output / (name + '.json')
        write(path, modified)
        run(name, upstream(path), False)

    current = (lifecycle / 'installation/current').resolve(strict=True)
    current_pin = identity(current / binary.name)
    candidate = output / 'candidate-with-unrelated-sigstore'
    shutil.copytree(current, candidate)
    signatures = json.loads((candidate / 'release-inventory.sig').read_bytes())
    signatures['sigstore_bundle'] = original
    (candidate / 'release-inventory.sig').write_text(json.dumps(signatures) + '\n')
    # The original candidate inventory and its real test RSA signature stay exact.
    host = lifecycle / 'test-only-host-trust'
    trust = json.loads((host / 'trust.json').read_bytes())
    trust.update(cosign=str(cosign), cosign_sha256=identity(cosign)['sha256'],
                 trusted_root=str(trusted_root), trusted_root_sha256=identity(trusted_root)['sha256'])
    trust_path = output / 'test-rsa-real-cosign-trust.json'
    write(trust_path, trust)
    run('candidate-rejects-unrelated-signed-blob', [str(binary), 'install', str(candidate), str(trust_path),
        identity(trust_path)['sha256'], str(lifecycle / 'installation'), str(output / 'candidate-log')], False)
    assert (lifecycle / 'installation/current').resolve(strict=True) == current
    assert identity(current / binary.name) == current_pin
    events = [json.loads(line) for line in (output / 'candidate-rejects-unrelated-signed-blob.execve').read_text().splitlines()]
    assert any(event.get('executable') == str(cosign) for event in events), 'candidate did not invoke real pinned verifier'
    write(output / 'summary.json', {'schema': 'rust-stable-real-sigstore-acceptance/v1',
        'binary': identity(binary), 'cosign': identity(cosign), 'trusted_root': identity(trusted_root),
        'upstream_bundle': identity(bundle), 'checks': records,
        'positive_scope': 'upstream cosign release identity only',
        'candidate_scope': 'real test RSA plus real cosign reject a signature over another payload; current preserved',
        'candidate_production_signature': False, 'release_ready': False})


if __name__ == '__main__':
    main()
