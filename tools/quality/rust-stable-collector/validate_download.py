#!/usr/bin/env python3
"""Repository-only real loopback HTTPS transport acceptance.

Uses lifecycle test RSA signatures and explicitly MOCKED Sigstore. The TLS CA is
an explicit host pin; this is not public hosting, production signing or a second OS.
"""
import argparse
import copy
import hashlib
import http.server
import json
import os
from pathlib import Path
import signal
import ssl
import subprocess
import threading
import time

from validate_stable_candidate import check_trace, trace_command

PROGRAM = 'harness-gate-rust-stable-collector'
FILES = (PROGRAM, 'LICENSE', 'support.json', 'release-inventory.json', 'release-inventory.sig')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--lifecycle', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--trace', action='store_true', help='require real execve tracing of plugin commands')
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    lifecycle = args.lifecycle.resolve(strict=True)
    args.output.mkdir(parents=True, exist_ok=False)
    output = args.output.resolve()
    root = output / 'installation'
    root.mkdir(mode=0o755)
    trust = lifecycle / 'test-only-host-trust/trust.json'
    bundles = {name: lifecycle / path for name, path in (
        ('good', 'package-0.1.0-candidate.1'),
        ('upgrade', 'package-0.1.0-candidate.1.upgrade-test'),
        ('bad-rsa', 'bad-rsa'),
        ('missing-sigstore', 'missing-sigstore'),
    )}
    cert, key = output / 'test-ca.pem', output / 'test-key.pem'
    commands, checks, requests = [], [], []
    env = {k: v for k, v in os.environ.items() if k.lower() not in ('http_proxy', 'https_proxy', 'all_proxy', 'no_proxy')}
    def openssl(argv):
        result = subprocess.run(['openssl', *map(str, argv)], capture_output=True, timeout=30)
        commands.append({'argv': result.args, 'exit_code': result.returncode, 'stderr': result.stderr.decode()})
        assert result.returncode == 0, result.stderr.decode()

    ca_key = output / 'test-ca-key.pem'
    leaf = output / 'test-server.pem'
    csr = output / 'test-server.csr'
    extensions = output / 'test-server.ext'
    extensions.write_text('basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:localhost\n')
    openssl(['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', ca_key, '-out', cert,
        '-days', '1', '-subj', '/CN=GH259 test CA', '-addext', 'basicConstraints=critical,CA:TRUE'])
    openssl(['req', '-new', '-newkey', 'rsa:2048', '-nodes', '-keyout', key, '-out', csr, '-subj', '/CN=localhost'])
    openssl(['x509', '-req', '-in', csr, '-CA', cert, '-CAkey', ca_key, '-CAcreateserial', '-out', leaf,
        '-days', '1', '-extfile', extensions])
    paused = threading.Event()

    class Handler(http.server.BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def do_GET(self):
            mode, name = self.path.lstrip('/').split('/', 1)
            requests.append({'mode': mode, 'asset': name})
            if mode in ('redirect', 'redirect-http', 'redirect-loop', 'redirect-auth'):
                target = {'redirect': f'{base}/good/{name}', 'redirect-http': f'http://localhost:{server.server_port}/good/{name}',
                    'redirect-loop': f'{base}/redirect-loop/{name}', 'redirect-auth': f'https://user:secret@localhost:{server.server_port}/good/{name}'}[mode]
                self.send_response(302)
                self.send_header('Location', target)
                self.end_headers()
                return
            data = (bundles.get(mode, bundles['good']) / name).read_bytes()
            if name == 'LICENSE' and mode == 'request-during-transfer':
                with (output / 'request-during-transfer.request.json').open('ab') as changed:
                    changed.write(b'\n')
            if name == 'LICENSE' and mode == 'ca-during-transfer':
                with cert.open('ab') as changed:
                    changed.write(b'\n')
            if mode == 'status':
                self.send_error(503)
                return
            if mode == 'timeout':
                time.sleep(2)
            self.send_response(200)
            if mode != 'oversize':
                self.send_header('Content-Length', str(len(data) + (1 if mode == 'length' else 0)))
            if mode == 'encoding':
                self.send_header('Content-Encoding', 'gzip')
            self.end_headers()
            try:
                if mode == 'interrupted':
                    self.wfile.write(data[:1])
                    self.wfile.flush()
                    paused.set()
                    time.sleep(4)
                    return
                if mode == 'truncated':
                    data = data[:4]
                elif mode == 'corrupt':
                    data = b'!' + data[1:]
                elif mode == 'oversize':
                    data += b'!'
                self.wfile.write(data)
            except (BrokenPipeError, ConnectionResetError, ssl.SSLError):
                pass

    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    server.daemon_threads = True
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(leaf, key)
    server.socket = context.wrap_socket(server.socket, server_side=True)
    base = f'https://localhost:{server.server_port}'
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()

    def request(mode):
        bundle = bundles.get(mode, bundles['good'])
        return {'schema': 'rust-stable-download-request/v1', 'timeout_seconds': 1,
            'tls_root': {'path': str(cert), 'sha256': sha(cert)},
            'assets': {name: {'url': f'{base}/{mode}/{name}', 'sha256': sha(bundle / name), 'bytes': (bundle / name).stat().st_size} for name in FILES}}

    def command(name, value, digest=None):
        path = output / f'{name}.request.json'
        write(path, value)
        return [str(binary), 'download-install', str(path), digest or sha(path), str(trust), sha(trust), str(root), str(output / f'log-{name}')]

    def traced(argv, name):
        if args.trace:
            return trace_command(output / f'{name}.execve', argv)
        return argv

    def audit_trace(name):
        if args.trace:
            check_trace(output / f'{name}.execve')

    def unchanged():
        assert (root / 'current').readlink() == current
        assert sha(root / 'current' / PROGRAM) == current_program

    def run(name, value, success=False, digest=None, error=None):
        before = len(requests)
        argv = command(name, value, digest)
        result = subprocess.run(traced(argv, name), capture_output=True, timeout=15, env=env)
        audit_trace(name)
        (output / f'{name}.stdout').write_bytes(result.stdout)
        (output / f'{name}.stderr').write_bytes(result.stderr)
        commands.append({'argv': argv, 'exit_code': result.returncode})
        assert (result.returncode == 0) == success, (name, result.stderr.decode())
        if error:
            assert error in result.stderr.decode(), (name, result.stderr.decode())
        if not success:
            unchanged()
        checks.append({'name': name, 'passed': True, 'requests': len(requests) - before})
        write(output / 'commands.json', commands)
        return json.loads(result.stdout) if success else None

    try:
        first = run('install', request('good'), True)
        current = (root / 'current').readlink()
        current_program = sha(root / 'current' / PROGRAM)
        initial_bytes = sum(asset['bytes'] for asset in request('good')['assets'].values())
        assert first['download_bytes'] == initial_bytes == first['package_bytes']
        assert (root / 'current' / PROGRAM).is_file()
        observed = subprocess.run(traced([str(root / 'current' / PROGRAM), '--version'], 'installed-version'), capture_output=True, timeout=10)
        audit_trace('installed-version')
        assert observed.returncode == 0
        commands.append({'argv': observed.args, 'exit_code': 0, 'stdout': observed.stdout.decode()})
        for name, mode, error in (
            ('http-error', 'status', 'HTTP status 503'), ('wrong-length', 'length', 'Content-Length mismatch'),
            ('truncated-body', 'truncated', None), ('wrong-content', 'corrupt', 'hash mismatch'),
            ('oversized-body', 'oversize', 'exceeded declared size'), ('encoded-body', 'encoding', 'encoded release asset'),
            ('timeout', 'timeout', None), ('redirect-downgrade', 'redirect-http', 'requires HTTPS'),
            ('redirect-cycle', 'redirect-loop', 'too many download redirects'), ('redirect-credentials', 'redirect-auth', 'without credentials'),
            ('bad-rsa', 'bad-rsa', None), ('missing-sigstore', 'missing-sigstore', None),
        ):
            run(name, request(mode), error=error)
        for name in ('untrusted-tls', 'wrong-tls-name', 'wrong-ca-pin', 'http-input', 'credential-input', 'extra-asset', 'wrong-size', 'wrong-hash', 'wrong-request-pin', 'unknown-field'):
            value = request('good')
            if name == 'untrusted-tls': value.pop('tls_root')
            if name == 'wrong-tls-name':
                for asset in value['assets'].values(): asset['url'] = asset['url'].replace('localhost', '127.0.0.1')
            if name == 'wrong-ca-pin': value['tls_root']['sha256'] = '0' * 64
            if name == 'http-input': value['assets']['LICENSE']['url'] = f'http://localhost:{server.server_port}/good/LICENSE'
            if name == 'credential-input': value['assets']['LICENSE']['url'] = f'https://user:secret@localhost:{server.server_port}/good/LICENSE'
            if name == 'extra-asset': value['assets']['environment.tar'] = copy.deepcopy(value['assets']['LICENSE'])
            if name == 'wrong-size': value['assets']['LICENSE']['bytes'] = 0
            if name == 'wrong-hash': value['assets']['LICENSE']['sha256'] = '0' * 64
            if name == 'unknown-field': value['unsigned_fallback'] = True
            run(name, value, digest='0' * 64 if name == 'wrong-request-pin' else None)
            if name != 'wrong-hash': assert checks[-1]['requests'] == 0, checks[-1]
        original_cert = cert.read_bytes()
        try:
            for name, error in (('request-during-transfer', 'download request changed'), ('ca-during-transfer', 'TLS root changed')):
                run(name, request(name), error=error)
                audit = json.loads((output / f'log-{name}/download.json').read_text())
                assert not audit['transfer_complete'] and len(audit['transfers']) == len(FILES)
                assert all(asset['verified'] for asset in audit['transfers'])
                assert audit['received_body_bytes'] == initial_bytes
                assert not list((output / f'log-{name}/commands').glob('*.json'))
        finally:
            cert.write_bytes(original_cert)
        argv = command('interrupted', request('interrupted'))
        with (output / 'interrupted.stdout').open('wb') as stdout, (output / 'interrupted.stderr').open('wb') as stderr:
            process = subprocess.Popen(traced(argv, 'interrupted'), stdout=stdout, stderr=stderr, env=env, start_new_session=True)
            assert paused.wait(5), 'download never started'
            if args.trace:
                # Kill the downloading process, keeping the observer alive to
                # retain a complete record of the real SIGKILL termination.
                # The paused transfer has not launched signature subprocesses.
                start = json.loads((output / 'interrupted.execve').read_text().splitlines()[0])
                assert start['schema'] == 'harness-exec-events/v1'
                os.kill(start['root_pid'], signal.SIGKILL)
                assert process.wait(timeout=5) == 128 + signal.SIGKILL
            else:
                os.killpg(process.pid, signal.SIGKILL)
                assert process.wait(timeout=5) == -signal.SIGKILL
        audit_trace('interrupted')
        commands.append({'argv': argv, 'exit_code': -signal.SIGKILL})
        unchanged()
        abandoned = sum(p.stat().st_size for stage in root.glob('.download-*') for p in stage.iterdir() if p.is_file())
        checks.append({'name': 'interrupted-preserves-current', 'passed': True, 'abandoned_download_bytes': abandoned})
        retry = run('retry-redirect', request('redirect'), True)
        unchanged()
        assert retry['download_bytes'] == initial_bytes
        upgrade = run('upgrade', request('upgrade'), True)
        assert upgrade['previous'] == first['current'] and upgrade['current'] != first['current']
        current = (root / 'current').readlink()
        current_program = sha(root / 'current' / PROGRAM)
        assert current_program != sha(bundles['good'] / PROGRAM)
        argv = [str(binary), 'rollback', str(root), first['current'], str(trust), sha(trust), str(output / 'log-rollback')]
        rolled = subprocess.run(traced(argv, 'rollback'), capture_output=True, timeout=15, env=env)
        audit_trace('rollback')
        commands.append({'argv': argv, 'exit_code': rolled.returncode, 'stdout': rolled.stdout.decode(), 'stderr': rolled.stderr.decode()})
        assert rolled.returncode == 0 and json.loads(rolled.stdout)['current'] == first['current']
        assert sha(root / 'current' / PROGRAM) == sha(bundles['good'] / PROGRAM)
        checks.append({'name': 'rollback-after-downloaded-upgrade', 'passed': True})
        transfers = {p.parent.name: json.loads(p.read_text()) for p in output.glob('log-*/download.json')}
        assert transfers['log-oversized-body']['received_body_bytes'] == request('good')['assets']['LICENSE']['bytes'] + 1
        assert transfers['log-truncated-body']['received_body_bytes'] == 4
        write(output / 'summary.json', {'schema': 'rust-stable-download-acceptance/v1', 'binary_sha256': sha(binary),
            'checks': checks, 'requests': requests, 'transfers': transfers, 'commands': commands,
            'initial_download_bytes': initial_bytes, 'upgrade_download_bytes': upgrade['download_bytes'],
            'total_received_body_bytes': sum(v['received_body_bytes'] for v in transfers.values()),
            'interrupted_audit': 'SIGKILL prevents final audit; actual abandoned file bytes recorded separately',
            'abandoned_download_bytes': abandoned, 'cache_bytes': 0,
            'tls': 'real loopback HTTPS with explicitly pinned test CA; certificate and hostname failures tested',
            'process_trace': 'real execve' if args.trace else 'unavailable; command records only',
            'signature_scope': 'real test RSA; mock Sigstore invocation only; no production signature claim',
            'platform_scope': 'current host only; no cross-host or public-network acceptance'})
        print(json.dumps({'checks': len(checks), 'passed': True, 'initial_download_bytes': initial_bytes, 'upgrade_download_bytes': upgrade['download_bytes']}))
    finally:
        write(output / 'commands.json', commands)
        server.shutdown()
        server.server_close()


if __name__ == '__main__':
    main()
