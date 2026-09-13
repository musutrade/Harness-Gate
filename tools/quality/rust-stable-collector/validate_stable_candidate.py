#!/usr/bin/env python3
"""Repository-only real acceptance driver; never distributed with the plugin.

Requires an already built candidate, installed target toolchain and LLVM tools.
No compiler bootstrap, install, baseline update or production publication.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check_trace(path):
    """Check actual execve arguments, excluding intentionally rejected probes."""
    text = path.read_text()
    assert 'execve(' in text, 'empty execution trace'
    for line in text.splitlines():
        if 'execve(' not in line:
            continue
        assert not re.search(r'(?:/|")(python[^/" ]*|pypy[^/" ]*)"', line), line
        assert not re.search(r'"-Z[^" ]*"|/nightly[-/]|"\+nightly|rustc-dev|rustc_driver|RUSTC_BOOTSTRAP=', line), line


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--trace', action='store_true', help='require strace; fail if unavailable')
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    args.output.mkdir(parents=True, exist_ok=False)
    output = args.output.resolve()
    project = output / 'plain'
    boundaries = output / 'boundaries'
    fixtures = Path(__file__).resolve().parent.parent / 'fixtures/rust-stable'
    shutil.copytree(fixtures / 'plain', project)
    shutil.copytree(fixtures / 'boundaries', boundaries)
    records = []
    observations = {}

    def run(name, command, *, success=True, env=None, trace=False):
        invocation = [str(binary), *map(str, command)]
        trace_path = output / f'{name}.execve'
        if args.trace and trace:
            invocation = ['strace', '-f', '-qq', '-s', '16384', '-e', 'trace=execve', '-o', str(trace_path), *invocation]
        result = subprocess.run(invocation, capture_output=True, env=env, timeout=360)
        (output / f'{name}.stdout').write_bytes(result.stdout)
        (output / f'{name}.stderr').write_bytes(result.stderr)
        assert (result.returncode == 0) == success, (name, result.returncode, result.stderr.decode())
        if args.trace and trace:
            check_trace(trace_path)
        records.append({'name': name, 'exit_code': result.returncode, 'passed': True})
        return json.loads(result.stdout) if success else result.stderr.decode()

    def write_request(name, root, features=None):
        request = run(f'prepare-{name}', ['prepare', root, output / f'capture-{name}', output / 'doctor/doctor.json'])
        request['features'] = features or []
        path = output / f'request-{name}.json'
        path.write_text(json.dumps(request))
        return path

    def verify(name, anchor, *, success=True, request_digest=None):
        return run(name, ['verify', output / 'capture-plain', anchor['manifest_sha256'], request_digest or anchor['request_sha256']], success=success)

    try:
        tools = run('doctor', ['doctor', project, output / 'doctor'], trace=True)
        request_path = write_request('plain', project)
        anchor = run('plain', ['collect', request_path], trace=True)
        assert anchor['state'] == 'unsupported' and anchor['capture'] == 'complete'
        assert verify('verify', anchor)['core_acceptance'] == 'pending'
        analysis = json.loads((output / 'capture-plain/source-analysis.json').read_text())
        functions = analysis['files']['src/lib.rs']['functions']
        assert [(f['name'], f['complexity']) for f in functions] == [('classify', 3), ('never_called', 1)]
        assert analysis['files']['src/lib.rs']['exclusions'] == ['test module: tests']
        report = json.loads((output / 'capture-plain/candidate.json').read_text())
        assert report['function_crap']['state'] == 'unsupported'
        raw = json.loads((output / 'capture-plain/coverage.json').read_text())
        never_called = [f for f in raw['data'][0]['functions'] if f['name'].endswith('12never_called')]
        assert len(never_called) == 1 and never_called[0]['count'] == 0
        observations['plain'] = {'anchors': anchor, 'llvm_totals_test_inclusive': raw['data'][0]['totals'], 'lexical_functions': functions}
        assert not list((output / 'capture-plain').glob('build-*'))
        assert 'output must be a new directory' in run('stale-output', ['collect', request_path], success=False)
        verify('wrong-request-anchor', anchor, success=False, request_digest='0' * 64)
        bad_anchor = {**anchor, 'manifest_sha256': '0' * 64}
        verify('wrong-manifest-anchor', bad_anchor, success=False)

        coverage = output / 'capture-plain/coverage.json'
        original_coverage = coverage.read_bytes()
        coverage.write_bytes(b'{}')
        assert 'artifact identity/set mismatch' in verify('corrupt-coverage', anchor, success=False)
        coverage.write_bytes(original_coverage)
        unknown = output / 'capture-plain/unlisted.json'
        unknown.write_text('{}')
        verify('mixed-artifact', anchor, success=False)
        unknown.unlink()
        source = project / 'src/lib.rs'
        original_source = source.read_bytes()
        source.write_bytes(original_source + b'\n// changed\n')
        assert 'source identity changed' in verify('changed-source', anchor, success=False)
        source.write_bytes(original_source)
        assert verify('restored-integrity', anchor)['integrity'] == 'verified'

        for label, features in [('boundaries', []), ('features', ['extra'])]:
            path = write_request(label, boundaries, features)
            result = run(label, ['collect', path], trace=True)
            assert result['state'] == 'unsupported'
            data = json.loads((output / f'capture-{label}/source-analysis.json').read_text())
            inventory = data['files']['src/lib.rs']
            rows = {f['name']: f for f in inventory['functions']}
            for name in ['deferred', 'generic', 'closure', 'feature_only']:
                assert rows[name]['state'] == 'unsupported' and rows[name]['complexity'] is None, (label, name)
            raw = json.loads((output / f'capture-{label}/coverage.json').read_text())
            owners = raw['data'][0]['functions']
            # These suffixes identify this controlled fixture's emitted symbols;
            # they are assertions, never the candidate's owner-mapping algorithm.
            constructor = [f for f in owners if f['name'].endswith('8deferred')]
            future_body = [f for f in owners if '8deferred' in f['name'] and not f['name'].endswith('8deferred')]
            assert len(constructor) == len(future_body) == 1
            assert constructor[0]['count'] == 1 and future_body[0]['count'] == 0
            assert any(f['name'].endswith('9generated') and f['count'] == 1 for f in owners)
            assert any(f['name'].endswith('8expanded') and f['count'] == 1 for f in owners)
            assert any(f['name'].endswith('12feature_only') for f in owners) == bool(features)
            observations[label] = {'anchors': result, 'llvm_totals_test_inclusive': raw['data'][0]['totals'], 'async_constructor_count': 1, 'async_body_count': 0, 'lexical_functions': inventory['functions'], 'unsupported': inventory['unsupported']}
            assert inventory['unsupported'] and 'build.rs' in data['excluded_paths']

        changed_tools = json.loads(request_path.read_text())
        changed_tools['output_root'] = str(output / 'capture-wrong-tools')
        changed_tools['tools']['llvm_cov']['identity']['sha256'] = '0' * 64
        path = output / 'request-wrong-tools.json'
        path.write_text(json.dumps(changed_tools))
        assert 'tool selection or identity changed' in run('wrong-tool-identity', ['collect', path], success=False)

        config_dir = project / '.cargo'
        config_dir.mkdir()
        config = config_dir / 'config.toml'
        config.write_text('[build]\nrustc-wrapper="python3"\n')
        assert 'unsupported Cargo configuration' in run('config-injection', ['prepare', project, output / 'capture-injection', output / 'doctor/doctor.json'], success=False)
        config.unlink()
        config_dir.rmdir()
        env = {**os.environ, 'PATH': '/nonexistent'}
        assert 'Rust required' in run('missing-rust', ['doctor', project, output / 'doctor-missing'], success=False, env=env)
        env = {**os.environ, 'RUSTFLAGS': '-Zunstable-options'}
        assert 'unsupported environment RUSTFLAGS' in run('injected-flags', ['doctor', project, output / 'doctor-flags'], success=False, env=env)
        pin = project / 'rust-toolchain.toml'
        pin.write_text('[toolchain]\nchannel="stable-candidate-intentionally-uninstalled"\n')
        env = {k: v for k, v in os.environ.items() if k != 'RUSTUP_TOOLCHAIN'}
        assert 'Rust required' in run('missing-project-toolchain', ['doctor', project, output / 'doctor-pin'], success=False, env=env)
        pin.unlink()
        (project / 'alias.rs').symlink_to('src/lib.rs')
        run('symlink-source', ['prepare', project, output / 'capture-symlink', output / 'doctor/doctor.json'], success=False)
        (project / 'alias.rs').unlink()

        source.write_text(original_source.decode() + '\n#[test] fn must_fail() { panic!("fixture intentional failure"); }\n')
        failing = write_request('failed-test', project)
        assert 'tool exited' in run('failed-test', ['collect', failing], success=False)
        assert not (output / 'capture-failed-test/manifest.json').exists()
        source.write_bytes(original_source)
        summary = {
            'schema': 'rust-stable-candidate-acceptance/v1', 'state': 'candidate-only',
            'checks': records, 'observations': observations, 'tools': tools, 'binary': {'sha256': digest(binary), 'bytes': binary.stat().st_size},
            'capture_bytes': sum(p.stat().st_size for p in (output / 'capture-plain').rglob('*') if p.is_file()),
            'persistent_candidate_cache_bytes': 0,
            'execve_trace': 'verified' if args.trace else 'not performed',
            'core_protocol_authentication': 'not exercised by this capture suite; see separate Core acceptance',
            'release_signature_lifecycle': 'not exercised by this capture suite; see separate lifecycle acceptance',
            'second_toolchain_and_system': 'not established by this invocation',
        }
        (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
        print(json.dumps({'checks_passed': len(records), 'summary': str(output / 'summary.json')}))
    except BaseException:
        (output / 'partial-checks.json').write_text(json.dumps(records, indent=2) + '\n')
        raise


if __name__ == '__main__':
    main()
