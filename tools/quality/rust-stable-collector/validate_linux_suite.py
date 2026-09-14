#!/usr/bin/env python3
"""Run the complete candidate suite in one Linux userspace with one pinned binary.

Invoke identically on each system (or inside a container). External dependencies
must already exist. No plugin build, toolchain installation, signing or release.
Existing captures can be supplied only by their original paths and exact digests.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time

from validate_stable_candidate import check_trace, trace_command


def identity(path):
    data = path.read_bytes()
    return {'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('binary', 'core', 'core-example', 'package', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    for name in ('runtime', 'generated'):
        parser.add_argument('--' + name, nargs=2, metavar=('ORIGINAL_PATH', 'SUMMARY_SHA256'))
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    core = args.core.resolve(strict=True)
    example = args.core_example.resolve(strict=True)
    package = args.package.resolve(strict=True)
    output = args.output.absolute()
    output.mkdir(parents=True, exist_ok=False)
    assert platform.system() == 'Linux'
    assert identity(binary) == identity(package / binary.name), 'matrix package is another executable'
    before = identity(binary)
    scripts = Path(__file__).resolve().parent
    records = []

    def run(name, argv, env=None, trace=False):
        argv = list(map(str, argv))
        if trace:
            argv = trace_command(output / (name + '.execve'), argv)
        start = time.monotonic()
        with (output / (name + '.stdout')).open('xb') as stdout, (output / (name + '.stderr')).open('xb') as stderr:
            result = subprocess.run(argv, stdout=stdout, stderr=stderr, env=env, timeout=1800)
        records.append({'name': name, 'argv': argv, 'exit_code': result.returncode,
                        'seconds': time.monotonic() - start,
                        'stdout': identity(output / (name + '.stdout')),
                        'stderr': identity(output / (name + '.stderr'))})
        (output / 'commands.json').write_text(json.dumps(records, indent=2) + '\n')
        assert result.returncode == 0, (name, (output / (name + '.stderr')).read_text()[-3000:])
        if trace:
            check_trace(output / (name + '.execve'))

    inputs = {}
    for name, driver in [('runtime', 'validate_stable_candidate.py'), ('generated', 'validate_generated_owners.py')]:
        supplied = getattr(args, name)
        if supplied:
            path = Path(supplied[0]).resolve(strict=True)
            assert identity(path / 'summary.json')['sha256'] == supplied[1]
            summary = json.loads((path / 'summary.json').read_bytes())
            actual = summary.get('binary', summary.get('binary_sha256'))
            assert actual in (before, before['sha256']), 'capture belongs to another binary'
        else:
            path = output / name
            run(name, ['python3', scripts / driver, '--binary', binary, '--output', path, '--trace'])
        inputs[name] = {'path': str(path), 'summary': identity(path / 'summary.json')}
    run('macros', ['python3', scripts / 'validate_macro_templates.py', '--binary', binary, '--output', output / 'macros', '--trace'])
    for case in ('plain', 'partial', 'modules', 'boundaries', 'features', 'registry',
                 'generated-default', 'generated-branching', 'generated-duplicate'):
        root = Path(inputs['generated' if case.startswith('generated-') else 'runtime']['path'])
        env = dict(os.environ)
        if case == 'registry':
            env['CARGO_HOME'] = str(root / 'registry-cargo-home')
        run('core-' + case, [example, binary, core, root, output / ('core-' + case), case], env, trace=True)
    run('historical', ['python3', scripts / 'compare_historical_fixture.py', '--binary', binary, '--output', output / 'historical'])
    run('lifecycle', ['python3', scripts / 'validate_lifecycle.py', '--binary', binary, '--package', package, '--output', output / 'lifecycle', '--trace'])
    run('https', ['python3', scripts / 'validate_download.py', '--binary', binary, '--lifecycle', output / 'lifecycle', '--output', output / 'https', '--trace'])
    assert identity(binary) == before
    summary = {'schema': 'rust-stable-linux-suite/v1', 'state': 'candidate-only',
               'binary': before, 'core': identity(core), 'system': platform.platform(),
               'os_release': Path('/etc/os-release').read_text(), 'inputs': inputs,
               'commands': records, 'production_signature': False,
               'signature_scope': 'SHA-256 and explicitly mocked Sigstore',
               'measurement_migration_approved': False, 'release_ready': False}
    (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps({'summary': str(output / 'summary.json'), 'binary': before, 'stages': len(records)}))


if __name__ == '__main__':
    main()
