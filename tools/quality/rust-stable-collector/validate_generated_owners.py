#!/usr/bin/env python3
"""Repository-only real generated-owner regression; never a plugin runtime.

Proves that a shared generator writing a real source file, pulled in with
include!, yields distinct stable coverage owners including a real zero for an
unexecuted function. This does not certify the proc-macro token-stream path.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

from validate_stable_candidate import check_trace, trace_command


def identity(path):
    data = path.read_bytes()
    return {'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--trace', action='store_true')
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    args.output.mkdir(parents=True, exist_ok=False)
    output = args.output.resolve()
    fixture = Path(__file__).resolve().parent.parent / 'fixtures/rust-generated-owners'
    project = output / 'project'
    shutil.copytree(fixture, project)
    records = []

    def run(name, command, success=True, plugin=True, contains=None):
        argv = list(map(str, ([binary] if plugin else []) + list(command)))
        traced = argv
        if args.trace:
            traced = trace_command(output / (name + '.execve'), argv)
        env = dict(os.environ, CARGO_TARGET_DIR=str(output / 'build'))
        result = subprocess.run(traced, cwd=project, env=env, capture_output=True, timeout=300)
        (output / (name + '.stdout')).write_bytes(result.stdout)
        (output / (name + '.stderr')).write_bytes(result.stderr)
        assert (result.returncode == 0) == success, (name, result.returncode, result.stderr[-2000:])
        if contains:
            assert contains.encode() in result.stderr, (name, result.stderr[-2000:])
        if args.trace:
            check_trace(output / (name + '.execve'))
        records.append({'name': name, 'argv': argv, 'exit_code': result.returncode,
                        'stdout': identity(output / (name + '.stdout')),
                        'stderr': identity(output / (name + '.stderr'))})
        return result.stdout

    exports = {}
    for configuration in ('default', 'branching'):
        flags = ['--features', 'branching'] if configuration == 'branching' else []
        run('tests-' + configuration, ['cargo', 'test', '--manifest-path', project / 'Cargo.toml',
                                      '--locked', '--offline', *flags], plugin=False)
        coverage = output / (configuration + '-coverage.json')
        run('coverage-' + configuration, ['cargo', 'llvm-cov', '--manifest-path', project / 'Cargo.toml',
            '--locked', '--offline', '--workspace', '--json',
            '--target', 'x86_64-unknown-linux-gnu', '--output-path', coverage, *flags], plugin=False)
        raw = json.loads(coverage.read_bytes())
        assert raw['version'] == '3.1.0'
        generated = [f for unit in raw['data'] for f in unit['functions']
                     if any('generated_owners.rs' in name for name in f['filenames'])]
        # Match by the length-prefixed mangled segment, e.g. 6branch in ...6branch.
        expected = {'plain': 1, 'branch': 2, 'unexecuted': 0, 'configured': 1}
        counts = {}
        for function in generated:
            for name, count in expected.items():
                if function['name'].endswith(f'{len(name)}{name}'):
                    assert function['count'] == count, (function['name'], function['count'])
                    counts[name] = function['count']
        assert sorted(counts) == sorted(expected), counts
        assert counts == {'plain': 1, 'branch': 2, 'unexecuted': 0, 'configured': 1}, counts
        # A distinct owner per generated function, and a real zero not a missing entry.
        assert len(generated) >= 4, 'generated owners collapsed'
        exports[configuration] = {'coverage': identity(coverage), 'owners': counts,
                                  'zero_is_recorded_not_missing': True}

    # The collector must not reject the fixture: this is an in-workspace build
    # script, so no registry build script/proc-macro rule applies.
    run('doctor', ['doctor', project, output / 'doctor'])
    request = run('prepare', ['prepare', project, output / 'capture', output / 'doctor/doctor.json'])
    request_file = output / 'request.json'
    request_file.write_bytes(request)
    run('collect', ['collect', request_file])
    capture = output / 'capture'

    # The collector must certify the generated file itself, with each function
    # bound to a real owner and the unexecuted one to a real zero.
    certified = json.loads((capture / 'generated-owners.json').read_bytes())
    assert certified['schema'] == 'rust-stable-generated-owners/v1-candidate'
    assert len(certified['owners']) == 1, certified['owners']
    entry = certified['owners'][0]
    assert entry['filename'].endswith('generated_owners.rs')
    assert entry['producer'].endswith('.d')
    certified_counts = {f['name']: f['execution_count'] for f in entry['owner']['functions']}
    assert certified_counts == {'plain': 1, 'branch': 2, 'unexecuted': 0, 'configured': 1}, certified_counts
    assert entry['owner']['state'] == 'supported'
    zero = next(f for f in entry['owner']['functions'] if f['name'] == 'unexecuted')
    assert zero['coverage_region']['covered'] == 0 and zero['coverage_region']['total'] > 0

    # The recomputed verify path must accept the capture. The anchor is the
    # manifest's own digest, so a tampered capture fails.
    manifest = json.loads((capture / 'manifest.json').read_bytes())
    anchor = identity(capture / 'manifest.json')['sha256']
    run('verify', ['verify', capture, anchor, manifest['request_sha256']])

    # Negative cases: forged coverage numbers, a swapped producer and a dropped
    # owner must fail the recompute, not succeed silently. The manifest is also
    # updated so the tamper reaches the fact check rather than failing the hash.
    target = capture / 'generated-owners.json'
    for name, mutation in [
        ('forged-generated-count', lambda value: value['owners'][0]['owner']['functions'][2].update(execution_count=99)),
        ('swapped-generated-producer', lambda value: value['owners'][0].update(producer='stolen/other.d')),
        ('dropped-generated-owner', lambda value: value['owners'][0]['owner']['functions'].pop()),
    ]:
        saved_bytes = target.read_bytes()
        saved_manifest = (capture / 'manifest.json').read_bytes()
        value = json.loads(saved_bytes)
        mutation(value)
        target.write_text(json.dumps(value))
        manifest['files']['generated-owners.json'] = identity(target)
        updated_manifest = json.dumps(manifest).encode()
        (capture / 'manifest.json').write_bytes(updated_manifest)
        run(name, ['verify', capture, identity(capture / 'manifest.json')['sha256'],
                   manifest['request_sha256']], success=False,
            contains='generated owner facts differ from recomputed facts')
        target.write_bytes(saved_bytes)
        (capture / 'manifest.json').write_bytes(saved_manifest)
        manifest = json.loads(saved_manifest)

    summary = {'schema': 'rust-generated-owner-regression/v1', 'binary': identity(binary),
               'fixture': identity(fixture / 'Cargo.lock'), 'checks': records, 'exports': exports,
               'certified_owners': certified_counts,
               'proc_macro_token_stream_coverage': 'unsupported (unchanged)',
               'core_acceptance': 'unsupported', 'T4': 'incomplete'}
    (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps({'checks': len(records), 'state': 'generated-owner regression passed'}))


if __name__ == '__main__':
    main()
