#!/usr/bin/env python3
"""Repository-only lifecycle execution: real RSA, explicitly mocked Sigstore.

Never package this driver or its generated test keys/verifier. It cannot establish
production signing, Sigstore cryptography, cross-host compatibility or release readiness.
"""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import time

from validate_stable_candidate import check_trace
from validate_upgrade import build_programs

PROGRAM = 'harness-gate-rust-stable-collector'
IDENTITY = 'https://github.com/musutrade/Harness-Gate/.github/workflows/rust-collector-release.yml@refs/heads/main'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--package', type=Path, required=True, help='prepared four-file unsigned candidate package')
    parser.add_argument('--trace', action='store_true', help='require real execve tracing of candidate commands')
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    prepared = args.package.resolve(strict=True)
    assert {p.name for p in prepared.iterdir()} == {PROGRAM, 'LICENSE', 'support.json', 'release-inventory.json'}
    assert all(p.is_file() and not p.is_symlink() for p in prepared.iterdir())
    assert sha(prepared / PROGRAM) == sha(binary)
    args.output.mkdir(parents=True, exist_ok=False)
    output = args.output.resolve()
    host = output / 'test-only-host-trust'
    host.mkdir()
    root = output / 'installation'
    root.mkdir(mode=0o755)
    private = host / 'test-only-private.pem'
    public = host / 'test-only-public.pem'
    commands = []
    checks = []

    def automation(argv, timeout=60, env=None):
        result = subprocess.run(list(map(str, argv)), capture_output=True, timeout=timeout, env=env)
        commands.append({'argv': list(map(str, argv)), 'exit_code': result.returncode,
                         'stdout': result.stdout.decode(), 'stderr': result.stderr.decode()})
        write(output / 'automation.json', commands)
        assert result.returncode == 0, commands[-1]
        return result.stdout.decode()

    version, upgrade_version, upgrade_binary, fixture_programs = build_programs(output, automation, sha)
    assert sha(binary) != sha(upgrade_binary)
    assert automation([binary, '--version']) == f'{PROGRAM} {version}\n'
    assert automation([upgrade_binary, '--version']) == f'{PROGRAM} {upgrade_version}\n'

    automation(['openssl', 'genpkey', '-algorithm', 'RSA', '-pkeyopt', 'rsa_keygen_bits:2048', '-out', private])
    automation(['openssl', 'pkey', '-in', private, '-pubout', '-out', public])
    verifier = host / 'test-only-mock-cosign'
    verifier.write_text(f'''#!/bin/sh
# TEST ONLY: checks invocation, does NOT verify a Sigstore signature.
[ "$#" = 11 ] && [ "$1" = verify-blob ] && [ "$2" = --bundle ] &&
[ "$4" = --trusted-root ] && [ "$6" = --offline ] &&
[ "$7" = --certificate-identity ] && [ "$8" = '{IDENTITY}' ] &&
[ "$9" = --certificate-oidc-issuer ] && [ "${{10}}" = https://token.actions.githubusercontent.com ] || exit 31
exit 0
''')
    verifier.chmod(0o755)
    trust_root = host / 'test-only-trusted-root.json'
    write(trust_root, {'test_only_mock': True})
    trust = host / 'trust.json'
    trust_value = {'schema': 'rust-stable-release-trust/v1',
                   'public_key': str(public), 'public_key_sha256': sha(public),
                   'cosign': str(verifier), 'cosign_sha256': sha(verifier),
                   'trusted_root': str(trust_root), 'trusted_root_sha256': sha(trust_root)}
    write(trust, trust_value)

    def bundle(version, executable):
        directory = output / f'package-{version}'
        directory.mkdir()
        shutil.copyfile(executable, directory / PROGRAM)
        shutil.copyfile(prepared / 'LICENSE', directory / 'LICENSE')
        support = json.loads((prepared / 'support.json').read_text())
        support['release_version'] = version
        support['program'] = {'sha256': sha(executable), 'bytes': executable.stat().st_size}
        write(directory / 'support.json', support)
        files = {name: {'sha256': sha(directory / name), 'bytes': (directory / name).stat().st_size}
                 for name in (PROGRAM, 'LICENSE', 'support.json')}
        write(directory / 'release-inventory.json', {'schema': 'rust-stable-release-inventory/v1',
              'version': version, 'target': 'x86_64-unknown-linux-gnu', 'files': files})
        signature = host / f'{version}.sig'
        automation(['openssl', 'dgst', '-sha256', '-sign', private, '-out', signature, directory / 'release-inventory.json'])
        write(directory / 'release-inventory.sig', {'schema': 'rust-collector-signatures/v2',
              'rsa_signature': base64.b64encode(signature.read_bytes()).decode(),
              'sigstore_bundle': {'test_only_mock': True}})
        return directory

    first, second = bundle(version, binary), bundle(upgrade_version, upgrade_binary)
    first_id, second_id = sha(first / 'release-inventory.json'), sha(second / 'release-inventory.json')

    def run(name, argv, success=True):
        command = [str(binary), *map(str, argv), str(output / f'log-{name}')]
        trace = output / f'{name}.execve'
        if args.trace:
            command = ['strace', '-f', '-q', '-s', '16384', '-e', 'trace=execve', '-o', str(trace), *command]
        result = subprocess.run(command, capture_output=True, timeout=80)
        if args.trace:
            check_trace(trace)
        (output / f'{name}.stdout').write_bytes(result.stdout)
        (output / f'{name}.stderr').write_bytes(result.stderr)
        assert (result.returncode == 0) == success, (name, result.returncode, result.stderr.decode())
        checks.append({'name': name, 'exit_code': result.returncode, 'passed': True})
        return json.loads(result.stdout) if success else result.stderr.decode()

    def install(name, directory, success=True, trust_path=trust):
        return run(name, ['install', directory, trust_path, sha(trust_path), root], success)

    run('unsigned-package-rejected', ['release-verify', prepared, trust, sha(trust)], False)
    verified = run('verify', ['release-verify', first, trust, sha(trust)])
    assert verified['inventory_sha256'] == first_id
    installed = install('install', first)
    assert installed['current'] == first_id and installed['previous'] is None
    assert automation([root / 'current' / PROGRAM, '--version']) == f'{PROGRAM} {version}\n'
    # The installed old program performs the actual upgrade; the installed new
    # program performs rollback. Invocation through the original binary alone
    # would not exercise lifecycle compatibility between the two executables.
    launcher = binary
    binary = root / 'current' / PROGRAM
    upgraded = install('upgrade', second)
    assert upgraded['current'] == second_id and upgraded['previous'] == first_id
    assert sha(root / 'current' / PROGRAM) == sha(upgrade_binary)
    assert automation([root / 'current' / PROGRAM, '--version']) == f'{PROGRAM} {upgrade_version}\n'
    rolled_back = run('rollback', ['rollback', root, first_id, trust, sha(trust)])
    binary = launcher
    assert rolled_back['current'] == first_id and rolled_back['previous'] == second_id
    assert (root / 'versions' / second_id / PROGRAM).is_file()

    def unchanged():
        assert os.readlink(root / 'current') == f'versions/{first_id}'
        assert sha(root / 'current' / PROGRAM) == sha(binary)

    def resign(directory):
        support = json.loads((directory / 'support.json').read_text())
        support['program'] = {'sha256': sha(directory / PROGRAM), 'bytes': (directory / PROGRAM).stat().st_size}
        write(directory / 'support.json', support)
        inventory = json.loads((directory / 'release-inventory.json').read_text())
        inventory['files'] = {name: {'sha256': sha(directory / name), 'bytes': (directory / name).stat().st_size}
                              for name in (PROGRAM, 'LICENSE', 'support.json')}
        write(directory / 'release-inventory.json', inventory)
        signature = host / 'resigned.sig'
        automation(['openssl', 'dgst', '-sha256', '-sign', private, '-out', signature, directory / 'release-inventory.json'])
        envelope = json.loads((directory / 'release-inventory.sig').read_text())
        envelope['rsa_signature'] = base64.b64encode(signature.read_bytes()).decode()
        write(directory / 'release-inventory.sig', envelope)

    for name, executable, expected_error in (
        ('program-version-mismatch', upgrade_binary, 'version mismatch'),
        ('program-nonzero', fixture_programs['program-nonzero'], 'exit status: 23'),
        ('program-timeout', fixture_programs['program-timeout'], 'timeout'),
        ('program-mutates-stage', fixture_programs['program-mutates-stage'], 'staged payload changed'),
        ('program-invalid-elf', None, 'launch check failed'),
    ):
        broken = output / name
        shutil.copytree(first, broken)
        if executable is None:
            header = bytearray(64)
            header[:6] = b'\x7fELF\x02\x01'
            header[18] = 62
            (broken / PROGRAM).write_bytes(header)
        else:
            shutil.copyfile(executable, broken / PROGRAM)
        resign(broken)
        started = time.monotonic()
        error = install(name, broken, False)
        assert expected_error in error, error
        if name == 'program-timeout':
            assert 60 <= time.monotonic() - started < 70
        unchanged()

    for name, mutation in (
        ('bad-rsa', lambda p: write(p / 'release-inventory.sig', {'schema': 'rust-collector-signatures/v2', 'rsa_signature': base64.b64encode(bytes(256)).decode(), 'sigstore_bundle': {'test_only_mock': True}})),
        ('missing-sigstore', lambda p: write(p / 'release-inventory.sig', {'schema': 'rust-collector-signatures/v2', 'rsa_signature': json.loads((p / 'release-inventory.sig').read_text())['rsa_signature'], 'sigstore_bundle': {}})),
        ('tampered-program', lambda p: (p / PROGRAM).write_bytes(b'forged')),
        ('mixed-inventory', lambda p: shutil.copyfile(second / 'release-inventory.json', p / 'release-inventory.json')),
        ('extra-asset', lambda p: (p / 'archive.tar').write_text('not needed')),
        ('symlink-asset', lambda p: ((p / PROGRAM).unlink(), (p / PROGRAM).symlink_to(binary))),
        ('duplicate-json', lambda p: (p / 'release-inventory.json').write_text('{"schema":"a","schema":"b"}')),
    ):
        broken = output / name
        shutil.copytree(first, broken)
        mutation(broken)
        install(name, broken, False)
        if name in ('bad-rsa', 'missing-sigstore'):
            assert not list((output / f'log-{name}/commands').glob('*.json')), 'unauthenticated program executed'
        unchanged()

    # These are re-signed with the real test RSA key, so failure proves the
    # Rust support contract check, rather than a stale payload/signature hash.
    for name, mutation in (
        ('support-schema', lambda v: v.update(schema='arbitrary/v1')),
        ('support-release', lambda v: v.update(release_version='different')),
        ('support-target', lambda v: v.update(target='aarch64-unknown-linux-gnu')),
        ('support-program', lambda v: v['program'].update(sha256='0' * 64)),
        ('support-license', lambda v: v['license'].update(bytes=0)),
        ('support-crap', lambda v: v.update(function_crap='supported')),
        ('support-published', lambda v: v.update(release_status='production-ready')),
        ('support-no-observations', lambda v: v.update(observations=[])),
        ('support-duplicate-observations', lambda v: v['observations'].append(v['observations'][0])),
        ('support-unknown-field', lambda v: v.update(host_kernel='publisher-fingerprint')),
    ):
        broken = output / name
        shutil.copytree(first, broken)
        support = json.loads((broken / 'support.json').read_text())
        mutation(support)
        write(broken / 'support.json', support)
        inventory = json.loads((broken / 'release-inventory.json').read_text())
        inventory['files']['support.json'] = {'sha256': sha(broken / 'support.json'), 'bytes': (broken / 'support.json').stat().st_size}
        write(broken / 'release-inventory.json', inventory)
        signature = host / f'{name}.sig'
        automation(['openssl', 'dgst', '-sha256', '-sign', private, '-out', signature, broken / 'release-inventory.json'])
        envelope = json.loads((broken / 'release-inventory.sig').read_text())
        envelope['rsa_signature'] = base64.b64encode(signature.read_bytes()).decode()
        write(broken / 'release-inventory.sig', envelope)
        error = install(name, broken, False)
        assert 'support' in error or 'candidate' in error or 'unknown field' in error, error
        unchanged()

    saved = verifier.read_bytes()
    verifier.write_text('#!/bin/sh\nexit 0\n')
    install('changed-verifier-pin', second, False)
    unchanged()
    verifier.write_bytes(saved)
    # A deliberately trusted failing test verifier proves nonzero propagation,
    # not Sigstore verification. Production has no mock/unsigned fallback flag.
    verifier.write_text('#!/bin/sh\nexit 23\n')
    rejected_trust = host / 'rejecting-trust.json'
    write(rejected_trust, dict(trust_value, cosign_sha256=sha(verifier)))
    assert 'Sigstore' in install('verifier-nonzero', second, False, rejected_trust)
    assert len(list((output / 'log-verifier-nonzero/commands').glob('*.json'))) == 1, 'program executed after failed Sigstore check'
    unchanged()
    verifier.write_bytes(saved)

    verifier.write_text('#!/bin/sh\nexec /bin/sleep 75\n')
    timeout_trust = host / 'timeout-trust.json'
    write(timeout_trust, dict(trust_value, cosign_sha256=sha(verifier)))
    started = time.monotonic()
    assert 'timeout' in install('verifier-timeout', second, False, timeout_trust)
    assert 60 <= time.monotonic() - started < 70
    unchanged()
    verifier.write_bytes(saved)

    missing_trust = host / 'missing-tool-trust.json'
    write(missing_trust, dict(trust_value, cosign=str(host / 'missing-cosign')))
    assert 'missing-cosign' in install('missing-verifier', second, False, missing_trust)
    unchanged()
    alias = output / 'root-alias'
    alias.symlink_to(root, target_is_directory=True)
    run('symlink-root', ['install', second, trust, sha(trust), alias], False)
    unchanged()
    old = root / 'versions' / second_id / PROGRAM
    original = old.read_bytes()
    old.write_bytes(b'corrupt rollback target')
    run('corrupt-rollback', ['rollback', root, second_id, trust, sha(trust)], False)
    unchanged()
    old.write_bytes(original)
    old.chmod(0o644)
    run('nonexecutable-rollback', ['rollback', root, second_id, trust, sha(trust)], False)
    unchanged()
    old.chmod(0o755)

    # Stop during a real verification transaction; the child announces its PID
    # so the test can also clean up after intentionally killing the parent.
    waiting = host / 'waiting-pid'
    verifier.write_text(f'#!/bin/sh\necho $$ > "{waiting}"\nexec /bin/sleep 120\n')
    waiting_trust = host / 'waiting-trust.json'
    write(waiting_trust, dict(trust_value, cosign_sha256=sha(verifier)))
    with (output / 'interrupted.stdout').open('wb') as out, (output / 'interrupted.stderr').open('wb') as err:
        process = subprocess.Popen([str(binary), 'install', str(second), str(waiting_trust), sha(waiting_trust), str(root), str(output / 'log-interrupted')], stdout=out, stderr=err)
        try:
            deadline = time.monotonic() + 15
            while not waiting.exists():
                assert process.poll() is None and time.monotonic() < deadline, 'verifier did not start'
                time.sleep(.02)
            install('concurrent-install', second, False, waiting_trust)
            unchanged()
        finally:
            process.kill()
            process.wait(timeout=5)
            if waiting.exists():
                os.killpg(int(waiting.read_text()), signal.SIGKILL)
    unchanged()
    checks.append({'name': 'interrupted-upgrade-preserves-current', 'exit_code': process.returncode, 'passed': True})
    verifier.write_bytes(saved)
    install('retry-after-interruption', second)
    assert os.readlink(root / 'current') == f'versions/{second_id}'
    assert automation([root / 'current' / PROGRAM, '--version']) == f'{PROGRAM} {upgrade_version}\n'
    run('final-rollback', ['rollback', root, first_id, trust, sha(trust)])
    unchanged()
    sizes = lambda path: sum(p.stat().st_size for p in path.rglob('*') if p.is_file() and not p.is_symlink())
    summary = {'schema': 'rust-stable-lifecycle-acceptance/v1', 'collector_sha256': sha(binary),
               'upgrade_collector_sha256': sha(upgrade_binary), 'upgrade_version': upgrade_version,
               'upgrade_scope': 'separately compiled test version of the same implementation; no production or historical release compatibility claim',
               'rsa': 'real RSA-2048/SHA-256 signatures; repository-generated test key',
               'sigstore': 'MOCK invocation/exit behavior only; no cryptographic or production acceptance',
               'production_signature': False, 'checks': checks,
               'first_install': installed, 'upgrade': upgraded, 'rollback': rolled_back,
               'package_bytes': sizes(first),
               'upgrade_package_bytes': sizes(second),
               'two_versions_bytes': sizes(root / 'versions' / first_id) + sizes(root / 'versions' / second_id),
               'interrupted_staging_bytes': sum(sizes(p) for p in (root / 'versions').glob('.staging-*')),
               'installation_bytes_including_interrupted_staging': sizes(root),
               'network_download_bytes': 0, 'network_note': 'local directory inputs; HTTPS download is exercised separately by validate_download.py',
               'stale_staging_note': 'killed processes may leave unselected staging directories; no automatic garbage collection yet'}
    write(output / 'automation.json', commands)
    write(output / 'summary.json', summary)
    print(json.dumps({'passed': len(checks), 'summary': str(output / 'summary.json')}))


if __name__ == '__main__':
    main()
