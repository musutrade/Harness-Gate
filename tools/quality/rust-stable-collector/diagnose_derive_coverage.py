#!/usr/bin/env python3
"""Repository-only real coverage regression for built-in Clone's missing owners.

This diagnoses a compiler capability boundary; it is not a collector backend or
certified evidence converter. No dependencies are installed by this driver.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess


def identity(path):
    data = path.read_bytes()
    return {'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--toolchain', default='1.98.1')
    parser.add_argument('--llvm-cov', required=True, type=Path)
    parser.add_argument('--llvm-profdata', required=True, type=Path)
    args = parser.parse_args()
    llvm_cov = args.llvm_cov.resolve(strict=True)
    llvm_profdata = args.llvm_profdata.resolve(strict=True)
    args.output.mkdir(parents=True, exist_ok=False)
    output = args.output.resolve()
    fixture = Path(__file__).resolve().parent.parent / 'fixtures/rust-derive-coverage'
    project = output / 'project'
    shutil.copytree(fixture, project)
    source = {str(p.relative_to(project)): identity(p)
              for p in sorted(project.rglob('*')) if p.is_file()}
    env = dict(os.environ)
    cleared = ('RUSTUP_TOOLCHAIN', 'RUSTC_BOOTSTRAP', 'RUSTFLAGS', 'RUSTDOCFLAGS',
               'CARGO_ENCODED_RUSTFLAGS', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER')
    for key in cleared:
        env.pop(key, None)
    overrides = {'CARGO_TARGET_DIR': str(output / 'build'),
                 'LLVM_COV': str(llvm_cov), 'LLVM_PROFDATA': str(llvm_profdata)}
    env.update(overrides)
    commands = []

    def run(name, argv):
        argv = list(map(str, argv))
        record = {'name': name, 'argv': argv, 'cwd': str(project)}
        commands.append(record)
        try:
            result = subprocess.run(argv, cwd=project, env=env, stdin=subprocess.DEVNULL,
                                    capture_output=True, timeout=180)
            record['exit_code'] = result.returncode
            for stream in ('stdout', 'stderr'):
                path = output / (name + '.' + stream)
                path.write_bytes(getattr(result, stream))
                record[stream] = {'path': str(path), **identity(path)}
            assert result.returncode == 0, (name, result.returncode, result.stderr[-2000:])
            return result.stdout.decode()
        except (OSError, subprocess.TimeoutExpired) as error:
            record['error'] = str(error)
            raise
        finally:
            (output / 'commands.json').write_text(json.dumps({
                'commands': commands, 'environment_overrides': overrides,
                'cleared_environment': cleared}, indent=2) + '\n')

    rustc = run('rustc', ['rustc', '+' + args.toolchain, '-vV'])
    assert 'release: ' + args.toolchain + '\n' in rustc and 'nightly' not in rustc
    llvm_version = re.search(r'LLVM version: (\d+\.\d+\.\d+)', rustc).group(1)
    for name, path in (('llvm-cov', llvm_cov), ('llvm-profdata', llvm_profdata)):
        version = run(name, [path, '--version'])
        assert re.search(r'LLVM version (\d+\.\d+\.\d+)', version).group(1) == llvm_version
    cargo_cov = run('cargo-llvm-cov', ['cargo', '+' + args.toolchain, 'llvm-cov', '--version'])
    run('fmt', ['cargo', '+' + args.toolchain, 'fmt', '--', '--check'])
    lib = project / 'src/lib.rs'
    lines = lib.read_text().splitlines()
    expected = {}
    for name in ('clone_one', 'clone_two', 'clone_unused', 'clone_configured',
                 'annotation_control_executes', 'distinct_derive_inputs_execute',
                 'configured_derive_executes'):
        anchors = [i + 1 for i, line in enumerate(lines) if 'fn ' + name + '(' in line]
        assert len(anchors) == 1
        expected[anchors[0]] = (name, 0 if name == 'clone_unused' else 1)
    manual_impl = lines.index('impl Clone for Manual {')
    assert lines[manual_impl + 1].strip() == 'fn clone(&self) -> Self {'
    expected[manual_impl + 2] = ('Manual::clone', 1)
    exports = {}
    for configuration in ('default', 'extra'):
        coverage = output / (configuration + '.json')
        flags = ['--features', 'extra'] if configuration == 'extra' else []
        result = run(configuration, ['cargo', '+' + args.toolchain, 'llvm-cov', '--locked',
                     '--offline', '--json', '--target', 'x86_64-unknown-linux-gnu',
                     '--output-path', coverage, *flags])
        assert '3 passed; 0 failed' in result
        raw = json.loads(coverage.read_bytes())
        assert raw['type'] == 'llvm.coverage.json.export' and raw['version'] == '3.1.0'
        entries = [f for unit in raw['data'] for f in unit['functions']]
        assert len(entries) == len(expected), 'owner inventory changed; review compiler capability'
        owners = {}
        for function in entries:
            assert function['filenames'] == [str(lib)]
            regions = function['regions']
            assert regions and regions[0][5] == 0 and regions[0][7] == 0
            # A fixed-fixture assertion only, not a general source ownership rule.
            anchor = regions[0][0]
            assert anchor in expected and anchor not in owners, 'unexpected or duplicate owner'
            name, count = expected[anchor]
            assert function['count'] == count
            owners[anchor] = {'fixture_owner': name, 'count': count, 'symbol': function['name']}
        assert set(owners) == set(expected)
        exports[configuration] = {'path': str(coverage), **identity(coverage),
                                  'owners': owners, 'functions': entries,
                                  'configured_test_result': 9 if flags else 7,
                                  'missing_derived_owners': ['One::clone', 'Two::clone',
                                                            'Unused::clone', 'Configured::clone'],
                                  'missing_control_owner': 'Annotated::clone'}
    after = {str(p.relative_to(project)): identity(p)
             for p in sorted(project.rglob('*')) if p.is_file()}
    assert source == after, 'fixture changed during execution'
    summary = {'schema': 'rust-derive-coverage-diagnostic/v1', 'rustc': rustc,
               'cargo_llvm_cov': cargo_cov, 'target': 'x86_64-unknown-linux-gnu',
               'source': source, 'external_tools': {str(p): identity(p)
                                                  for p in (llvm_cov, llvm_profdata)},
               'commands': commands, 'cleared_environment': cleared,
               'environment_overrides': overrides, 'exports': exports,
               'scope': 'built-in Clone and explicit annotation control only',
               'derived_coverage_and_crap': 'unsupported; absent function records are not zero',
               'process_trace': 'not run', 'T4': 'incomplete'}
    (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps({'commands': len(commands), 'configurations': 2,
                      'tests_per_configuration': 3, 'T4': 'incomplete'}))


if __name__ == '__main__':
    main()
