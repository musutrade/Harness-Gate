#!/usr/bin/env python3
"""Repository-only real shared-generator regression; never a plugin runtime.

Compiler exports are retained as diagnostics, not signed collector evidence.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tomllib

from validate_stable_candidate import check_trace


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
    fixture = Path(__file__).resolve().parent.parent / 'fixtures/rust-macro-observation'
    project = output / 'project'
    shutil.copytree(fixture, project)
    records = []

    def run(name, command, success=True, plugin=True, contains=None):
        argv = list(map(str, ([binary] if plugin else []) + list(command)))
        traced = argv
        if args.trace:
            traced = ['strace', '-f', '-q', '-s', '16384', '-e', 'trace=execve',
                      '-o', str(output / (name + '.execve')), *argv]
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

    observations = {}
    exports = {}
    for configuration in ('default', 'branching'):
        path = output / (configuration + '.json')
        run('observe-' + configuration, ['observe-macro-template', project, configuration, path])
        observation = json.loads(path.read_bytes())
        observations[configuration] = observation
        functions = observation['analysis']['functions']
        assert observation['analysis']['state'] == 'supported'
        assert {f['invocation']['owner']: f['complexity']['value'] for f in functions} == {
            'plain': 1, 'branch': 2, 'unexecuted': 2, 'configured': 1 if configuration == 'default' else 2}
        assert all(f['coverage']['state'] == 'unsupported' and 'value' not in f['coverage']
                   and f['crap']['state'] == 'unsupported' for f in functions)
        run('verify-' + configuration, ['verify-macro-template', project, path, identity(path)['sha256']])
        flags = ['--features', 'branching'] if configuration == 'branching' else []
        run('tests-' + configuration, ['cargo', 'test', '--manifest-path', project / 'Cargo.toml',
                                      '--locked', '--offline', *flags], plugin=False)
        coverage = output / (configuration + '-coverage.json')
        run('coverage-' + configuration, ['cargo', 'llvm-cov', '--manifest-path', project / 'Cargo.toml',
            '--locked', '--offline', '--package', 'gate-observation-consumer', '--json',
            '--target', 'x86_64-unknown-linux-gnu', '--output-path', coverage, *flags], plugin=False)
        raw = json.loads(coverage.read_bytes())
        assert raw['version'] == '3.1.0'
        consumer = str(project / 'consumer/src/lib.rs')
        entries = [f for unit in raw['data'] for f in unit['functions'] if consumer in f['filenames']]
        assert len(entries) == 2, 'generated export changed: review owner mapping before claiming coverage'
        for function in entries:
            assert function['count'] == 1
            file_id = function['filenames'].index(consumer)
            regions = [r for r in function['regions'] if r[5] == file_id and r[7] == 0]
            assert regions and all(r[0] >= 12 for r in regions), 'non-test owner appeared: review mapping'
        exports[configuration] = {'coverage': identity(coverage), 'consumer_functions': entries,
                                  'generated_business_functions': 'absent; never infer zero',
                                  'certification': 'unsupported diagnostic export, not Core evidence'}

    plain, branch = observations['default'], observations['branching']
    assert plain['source_files'] == branch['source_files']
    assert plain['analysis']['functions'][-1]['generated_tokens_sha256'] != branch['analysis']['functions'][-1]['generated_tokens_sha256']
    original = output / 'default.json'
    run('wrong-anchor', ['verify-macro-template', project, original, '0' * 64],
        success=False, contains='anchor mismatch')
    for name, change in [
        ('forged-coverage', lambda v: v['analysis']['functions'][2].update(coverage={'state': 'supported', 'value': 0})),
        ('mixed-config-owner', lambda v: v['analysis']['functions'].__setitem__(-1, branch['analysis']['functions'][-1])),
        ('wrong-target', lambda v: v['requested_configuration'].update(target='aarch64-unknown-linux-gnu')),
        ('changed-feature', lambda v: v['requested_configuration'].update(name='branching')),
        ('duplicate-owner-claim', lambda v: v['analysis']['functions'].append(v['analysis']['functions'][0])),
    ]:
        value = json.loads(original.read_bytes())
        change(value)
        mutated = output / (name + '.json')
        mutated.write_text(json.dumps(value))
        run(name, ['verify-macro-template', project, mutated, identity(mutated)['sha256']],
            success=False, contains='differs from recomputed source facts')

    for relative in ('generator/src/lib.rs', 'macros/src/lib.rs', 'Cargo.lock', 'consumer/Cargo.toml'):
        path = project / relative
        saved = path.read_bytes()
        path.write_bytes(saved + b'\n')
        name = 'changed-' + relative.replace('/', '-')
        run(name, ['observe-macro-template', project, 'default', output / (name + '.json')],
            success=False, contains='source/version identity mismatch')
        path.write_bytes(saved)

    source = project / 'consumer/src/lib.rs'
    saved = source.read_text()
    source.write_text(saved + '\n')
    run('source-drift', ['verify-macro-template', project, original, identity(original)['sha256']],
        success=False, contains='differs from recomputed source facts')
    for name, extra in [('duplicate-invocation', 'observed_function!(plain,true);'),
                        ('nested', 'mod nested { observed_function!(inside,true); }'),
                        ('derive', '#[derive(Unknown)] struct Derived;'),
                        ('cfg', '#[cfg(unix)] observed_function!(platform,true);')]:
        source.write_text(saved + '\n' + extra)
        path = output / (name + '.json')
        run(name, ['observe-macro-template', project, 'default', path],
            success=name != 'duplicate-invocation',
            contains='duplicate active generated owner' if name == 'duplicate-invocation' else None)
        if name != 'duplicate-invocation':
            result = json.loads(path.read_bytes())['analysis']
            assert result['state'] == 'unsupported' and result['functions'] == []
    source.write_text(saved)

    # Retain the separate authenticated-capture blocker. Do not bypass the
    # collector's build-input contract to pass this shared-generator example.
    archives = {}
    cargo_home = Path(os.environ.get('CARGO_HOME', str(Path.home() / '.cargo')))
    for package in tomllib.loads((project / 'Cargo.lock').read_text())['package']:
        if 'checksum' in package:
            found = list((cargo_home / 'registry/cache').glob('*/' + package['name'] + '-' + package['version'] + '.crate'))
            assert len(found) == 1, package['name']
            archives[package['checksum']] = str(found[0])
    archive_file = output / 'archives.json'
    archive_file.write_text(json.dumps(archives))
    run('doctor', ['doctor', project, output / 'doctor'])
    request = run('prepare', ['prepare', project, output / 'capture', output / 'doctor/doctor.json',
                              '--registry-archives', archive_file])
    request_file = output / 'request.json'
    request_file.write_bytes(request)
    run('capture-blocked', ['collect', request_file], success=False,
        contains='registry build scripts/proc macros unsupported')
    summary = {'schema': 'rust-shared-macro-regression/v1', 'binary': identity(binary),
               'source': plain['source_files'], 'observations': {c: identity(output / (c + '.json')) for c in observations},
               'tools': json.loads((output / 'doctor/doctor.json').read_bytes()),
               'checks': records, 'exports': exports, 'trace': 'passed' if args.trace else 'not run',
               'core_macro_coverage_and_crap': 'unsupported', 'T4': 'incomplete'}
    (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps({'checks': len(records), 'state': 'bounded macro regression passed; T4 incomplete'}))


if __name__ == '__main__':
    main()
