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

    run('doctor', ['doctor', project, output / 'doctor'])
    certified_results = {}
    for configuration in ('default', 'branching', 'duplicate'):
        case = 'generated-' + configuration
        capture = output / ('capture-' + case)
        request = json.loads(run('prepare-' + case, ['prepare', project, capture,
                            output / 'doctor/doctor.json']))
        request['features'] = [] if configuration == 'default' else [configuration]
        request_file = output / (case + '.request.json')
        request_file.write_text(json.dumps(request))
        run(case, ['collect', request_file])
        certified = json.loads((capture / 'generated-owners.json').read_bytes())
        assert len(certified['owners']) == (2 if configuration == 'duplicate' else 1)
        result = {}
        for entry in certified['owners']:
            counts = {f['name']: f['execution_count'] for f in entry['owner']['functions']}
            duplicate = entry['filename'].endswith('duplicate_owners.rs')
            expected = ({'plain': 2, 'branch': 0, 'unexecuted': 0, 'configured': 0} if duplicate else
                        {'plain': 1, 'branch': 2, 'unexecuted': 0, 'configured': 1})
            assert counts == expected, counts
            assert entry['owner']['state'] == 'supported'
            assert entry['compilations'] and len(set(entry['compilations'])) == len(entry['compilations'])
            complexity = {f['name']: f['complexity'] for f in entry['analysis']['functions']}
            assert complexity['configured'] == (2 if configuration == 'branching' else 1)
            result[entry['logical_path']] = counts
        if configuration == 'duplicate':
            assert certified['owners'][0]['sha256'] == certified['owners'][1]['sha256']
        certified_results[configuration] = result
        manifest_path = capture / 'manifest.json'
        manifest = json.loads(manifest_path.read_bytes())
        anchor = identity(manifest_path)['sha256']
        run('verify-' + case, ['verify', capture, anchor, manifest['request_sha256']])
        target = capture / 'generated-owners.json'
        for name, mutation in [
            ('forged-count', lambda v: v['owners'][0]['owner']['functions'][0].update(execution_count=99)),
            ('forged-line-ratio', lambda v: v['owners'][0]['owner']['functions'][0]['coverage_line'].update(covered=99)),
            ('forged-line-counts', lambda v: v['owners'][0]['owner']['functions'][0].update(line_counts={'1': 99})),
            ('swapped-producer', lambda v: v['owners'][0].update(producer='stolen/other.d')),
            ('dropped-compilation', lambda v: v['owners'][0]['compilations'].pop()),
            ('dropped-owner', lambda v: v['owners'][0]['owner']['functions'].pop()),
            ('dropped-file', lambda v: v['owners'].pop()),
        ]:
            saved = target.read_bytes()
            saved_manifest = manifest_path.read_bytes()
            value = json.loads(saved)
            mutation(value)
            target.write_text(json.dumps(value))
            manifest['files']['generated-owners.json'] = identity(target)
            manifest_path.write_text(json.dumps(manifest))
            run(name + '-' + case, ['verify', capture, identity(manifest_path)['sha256'],
                manifest['request_sha256']], success=False,
                contains='generated owner facts differ from recomputed facts')
            target.write_bytes(saved)
            manifest_path.write_bytes(saved_manifest)
            manifest = json.loads(saved_manifest)
        workspace = output / ('source-' + case)
        args_export = ['export-core-source', capture, anchor, manifest['request_sha256']]
        run('export-' + case, [*args_export, workspace])
        run('export-existing-' + case, [*args_export, workspace], success=False,
            contains='source workspace must be a new directory')
        rejected = project / ('rejected-' + case)
        run('export-in-project-' + case, [*args_export, rejected], success=False)
        assert not rejected.exists()
        describe = ['describe', capture, anchor, manifest['request_sha256'], '--source-workspace', workspace]
        description = json.loads(run('describe-' + case, describe))
        generated = [o for o in description['owners'] if o.get('generated')]
        assert len(generated) == (8 if configuration == 'duplicate' else 4)
        assert len({(o['path'], o['discriminator']) for o in generated}) == len(generated)
        for owner in description['owners']:
            assert identity(workspace / owner['path'])['sha256'] == owner['source_sha256']
        # A modified snapshot cannot re-authorize itself, even if its own manifest
        # is rewritten. Both the collector and Core must still use captured bytes.
        source = workspace / generated[0]['path']
        saved = source.read_bytes()
        source.write_bytes(saved + b'// forged\n')
        run('tampered-source-' + case, describe, success=False)
        snapshot_manifest = workspace / 'source-workspace.json'
        snapshot_saved = snapshot_manifest.read_bytes()
        forged = json.loads(snapshot_saved)
        forged['files'][generated[0]['path']] = identity(source)
        snapshot_manifest.write_text(json.dumps(forged, indent=2))
        run('self-reanchored-snapshot-' + case, describe, success=False)
        snapshot_manifest.write_bytes(snapshot_saved)
        source.write_bytes(saved)
        source.unlink()
        run('missing-source-' + case, describe, success=False)
        source.symlink_to(capture / 'compiler-generated' / generated[0]['source_sha256'])
        run('symlink-source-' + case, describe, success=False)
        source.unlink()
        source.write_bytes(saved)
        extra = workspace / 'unexpected.rs'
        extra.write_text('fn omitted() {}')
        run('extra-source-' + case, describe, success=False)
        extra.unlink()
        if configuration != 'default':
            wrong_workspace = output / 'source-generated-default'
            run('mixed-configuration-' + case, [*describe[:-1], wrong_workspace], success=False)

    summary = {'schema': 'rust-generated-owner-regression/v1', 'binary': identity(binary),
               'fixture': identity(fixture / 'Cargo.lock'), 'checks': records, 'exports': exports,
               'certified_owners': certified_results,
               'proc_macro_token_stream_coverage': 'unsupported (unchanged)',
               'core_acceptance': 'separate authenticated Core fixture required', 'T4': 'incomplete'}
    (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps({'checks': len(records), 'state': 'generated-owner regression passed'}))


if __name__ == '__main__':
    main()
