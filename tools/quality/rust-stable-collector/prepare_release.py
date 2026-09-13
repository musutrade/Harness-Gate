#!/usr/bin/env python3
"""Repository-only unsigned candidate preparation. Never ship or invoke this script.

The output is four payload/inventory files, deliberately missing the mandatory
signature envelope. Review artifacts stay outside it. No signing or publication.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import tarfile
import tempfile
import tomllib

PROGRAM = 'harness-gate-rust-stable-collector'
TARGET = 'x86_64-unknown-linux-gnu'
ROOT = Path(__file__).resolve().parents[3]
CRATE = ROOT / 'tools/quality/rust-stable-collector'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read(path, limit=64 * 1024 * 1024):
    require(not path.is_symlink() and path.is_file(), f'not a regular file: {path}')
    require(path.stat().st_size <= limit, f'file exceeds bound: {path}')
    return path.read_bytes()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def identity(path):
    data = read(path)
    return {'sha256': sha(data), 'bytes': len(data)}


def strict_json(data):
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result, f'duplicate JSON key: {key}')
            result[key] = value
        return result
    def invalid(value):
        raise ValueError(f'non-finite JSON value: {value}')
    return json.loads(data, object_pairs_hook=pairs, parse_constant=invalid)


def write(path, value):
    with path.open('x') as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write('\n')


def source_identity():
    paths = [CRATE / 'Cargo.toml', CRATE / 'Cargo.lock', ROOT / 'LICENSE',
             CRATE / 'prepare_release.py', *sorted((CRATE / 'src').glob('*.rs'))]
    return {str(p.relative_to(ROOT)): identity(p) for p in paths}


def registry_notices(package, lock):
    """Authenticate archived licenses and the actual unpacked build input.

    This never extracts or republishes a .crate archive. Registry metadata alone
    is not proof of the cached source that Cargo will compile.
    """
    require(package['source'] == 'registry+https://github.com/rust-lang/crates.io-index',
            f'unsupported release dependency source: {package["id"]}')
    source = Path(package['manifest_path']).parent
    key = (package['name'], package['version'], package['source'])
    expected = lock[key]['checksum']
    archive = source.parents[2] / 'cache' / source.parent.name / (source.name + '.crate')
    require(identity(archive)['sha256'] == expected, f'crate checksum mismatch: {archive}')
    notices = {}
    seen = set()
    total = 0
    with tarfile.open(archive, 'r:gz') as tar:
        for member in tar:
            relative = PurePosixPath(member.name)
            require(relative.parts and relative.parts[0] == source.name
                    and '..' not in relative.parts and not relative.is_absolute(),
                    f'unsafe crate member: {member.name}')
            if member.isdir():
                continue
            require(member.isfile(), f'nonregular crate member: {member.name}')
            name = relative.relative_to(source.name).as_posix()
            require(name not in seen, f'duplicate crate member: {name}')
            seen.add(name)
            total += member.size
            require(member.size <= 16 * 1024 * 1024 and total <= 128 * 1024 * 1024,
                    'crate expands beyond release preparation bound')
            contents = tar.extractfile(member).read()
            require(contents == read(source / name), f'cached source differs from locked archive: {source / name}')
            leaf = relative.name.lower()
            if leaf.startswith(('license', 'copying', 'copyright', 'notice', 'unlicense')):
                notices[name] = contents.decode('utf-8')
    # Cargo cache markers are generated outside the archive; no additional source
    # or license file can silently affect a build or its notices.
    actual = set()
    for path in source.rglob('*'):
        require(not path.is_symlink(), f'cached crate symlink: {path}')
        if path.is_file():
            actual.add(path.relative_to(source).as_posix())
        else:
            require(path.is_dir(), f'cached crate special file: {path}')
    require(actual - {'.cargo-ok', '.cargo-checksum.json'} == seen,
            f'undeclared cached crate files: {source}')
    require(package.get('license') and notices, f'missing crate license notices: {package["id"]}')
    return notices, {'package': package['name'], 'version': package['version'],
                     'license_expression': package['license'], 'crate_sha256': expected,
                     'notices': {name: sha(text.encode()) for name, text in sorted(notices.items())}}


def acceptance(path, pin, binary):
    raw = read(path, 8 * 1024 * 1024)
    require(sha(raw) == pin, 'acceptance anchor mismatch')
    value = strict_json(raw)
    require(value['schema'] == 'rust-stable-candidate-acceptance/v1'
            and value['state'] == 'candidate-only' and value['binary'] == binary,
            'acceptance does not identify this candidate binary')
    checks = value['checks']
    names = {check['name'] for check in checks}
    require(len(names) == len(checks) and len(checks) >= 74
            and {'doctor', 'plain', 'boundaries', 'features', 'verify', 'failed-test',
                 'corrupt-coverage', 'mixed-artifact', 'changed-source',
                 'negative-function-count', 'overflow-function-count',
                 'fractional-function-count', 'missing-function-regions',
                 'invalid-region-file', 'reversed-region', 'unknown-region-kind',
                 'covered-exceeds-total', 'wrong-summary-percent', 'wrong-notcovered',
                 'invalid-segment-boolean', 'mcdc-capability', 'duplicate-coverage-key',
                 'certified-function-owners', 'missing-llvm-owner', 'duplicate-llvm-symbol',
                 'multiple-llvm-owners', 'cross-file-owner', 'unknown-source-owner',
                 'inherited-parent-count', 'unexecuted-owner-regions',
                 'omitted-source-owners', 'changed-test-exclusions',
                 'prepare-registry', 'registry', 'verify-registry',
                 'registry-function-owner', 'missing-registry-archive',
                 'changed-registry-archive', 'changed-registry-source',
                 'poisoned-registry-before-build', 'extra-registry-source',
                 'registry-source-symlink', 'omitted-dependency-provenance',
                 'omitted-metadata-dependency', 'restored-registry-integrity',
                 'external-include-bytes', 'external-include-rust', 'external-path-module',
                 'workspace-include-bytes', 'verify-workspace-include', 'empty-compiler-proof',
                 'omitted-compiler-input', 'malformed-dep-info', 'wrong-compiler-cwd',
                 'omitted-dep-info-producer', 'restored-compiler-input-proof'} <= names
            and all(check['passed'] is True for check in checks),
            'candidate acceptance is incomplete or failed')
    tools = value['tools']
    require(tools['release'] in ('1.97.1', '1.98.1') and tools['host'] == TARGET
            and tools['cargo_llvm_cov']['version'] == 'cargo-llvm-cov 0.9.0'
            and re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+', tools['llvm']),
            'unsupported acceptance tools')
    return {'rust': tools['release'], 'llvm': tools['llvm'], 'cargo_llvm_cov': '0.9.0',
            'target': TARGET, 'acceptance_sha256': pin}


def prepare(output, target_dir, toolchain, observations):
    require(toolchain in ('1.97.1', '1.98.1'), 'explicit candidate stable toolchain required')
    require(not output.exists() and not output.is_symlink(), 'output must be fresh')
    require(target_dir.is_absolute() and target_dir.is_relative_to(ROOT / 'target'),
            'build target must stay in this workspace target directory')
    output.parent.mkdir(parents=True, exist_ok=True)
    initial = source_identity()
    # Build tools do not inherit compiler wrappers or unstable flags. User project
    # builds are separate and continue to honor project-selected toolchains.
    env = {key: os.environ[key] for key in ('HOME', 'PATH', 'CARGO_HOME', 'RUSTUP_HOME') if key in os.environ}
    env.update(CARGO_TARGET_DIR=str(target_dir), RUSTFLAGS='', RUSTDOCFLAGS='',
               CARGO_ENCODED_RUSTFLAGS='', RUSTC_WRAPPER='', RUSTC_WORKSPACE_WRAPPER='')
    commands = []
    with tempfile.TemporaryDirectory(prefix='.prepare-', dir=output.parent) as temporary:
        stage = Path(temporary)
        def run(argv):
            result = subprocess.run(argv, cwd=ROOT, env=env, capture_output=True, timeout=600)
            record = {'argv': argv, 'exit_code': result.returncode,
                      'stdout_sha256': sha(result.stdout), 'stderr_sha256': sha(result.stderr)}
            commands.append(record)
            index = len(commands)
            (stage / f'command-{index}.stdout').write_bytes(result.stdout)
            (stage / f'command-{index}.stderr').write_bytes(result.stderr)
            require(result.returncode == 0, f'preparation command failed: {argv}: {result.stderr.decode()[-2000:]}')
            return result.stdout

        rust = run(['rustc', f'+{toolchain}', '-vV']).decode()
        require(f'release: {toolchain}\n' in rust and f'host: {TARGET}\n' in rust,
                'build compiler must be the selected stable Linux x86_64 GNU toolchain')
        sysroot = Path(run(['rustc', f'+{toolchain}', '--print', 'sysroot']).decode().strip())
        compiler = identity(sysroot / 'bin/rustc')
        std_notice = sysroot / 'share/doc/rust/COPYRIGHT-library.html'
        std_text = read(std_notice, 4 * 1024 * 1024).decode()
        metadata = strict_json(run(['cargo', f'+{toolchain}', 'metadata', '--manifest-path',
            str(CRATE / 'Cargo.toml'), '--locked', '--offline', '--format-version', '1',
            '--filter-platform', TARGET]))
        lock = {(p['name'], p['version'], p.get('source')): p
                for p in tomllib.loads(read(CRATE / 'Cargo.lock').decode())['package']}
        sections = ['Harness-Gate\n' + read(ROOT / 'LICENSE').decode()]
        dependencies = []
        for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
            if package['source'] is None:
                require(Path(package['manifest_path']) == CRATE / 'Cargo.toml',
                        'unexpected local release dependency')
                continue
            notices, provenance = registry_notices(package, lock)
            dependencies.append(provenance)
            for name, text in sorted(notices.items()):
                sections.append(f'{package["name"]} {package["version"]} / {name}\n{text}')
        sections.append(f'Rust standard library {toolchain} / COPYRIGHT-library.html\n{std_text}')
        messages = run(['cargo', f'+{toolchain}', 'build', '--manifest-path',
            str(CRATE / 'Cargo.toml'), '--release', '--locked', '--offline', '--message-format=json'])
        artifacts = [strict_json(line) for line in messages.splitlines()]
        executable = [item['executable'] for item in artifacts
                      if item.get('reason') == 'compiler-artifact' and item.get('executable')
                      and item['target']['name'] == PROGRAM]
        require(len(executable) == 1, 'release build did not produce one collector executable')
        binary = Path(executable[0])
        require(binary.is_relative_to(target_dir), 'unexpected build output location')
        binary_id = identity(binary)
        observed = [acceptance(path, pin, binary_id) for path, pin in observations]
        require(observed and len({o['acceptance_sha256'] for o in observed}) == len(observed),
                'unique pinned acceptance records required')
        # Check post-build inputs as well; cached build input is not assumed clean.
        for package in metadata['packages']:
            if package['source']:
                registry_notices(package, lock)
        require(source_identity() == initial and identity(sysroot / 'bin/rustc') == compiler
                and read(std_notice).decode() == std_text, 'release inputs changed during preparation')
        package = stage / 'unsigned-package'
        package.mkdir()
        shutil.copyfile(binary, package / PROGRAM)
        (package / PROGRAM).chmod(0o755)
        (package / 'LICENSE').write_text('\n\n' + ('\n\n' + '=' * 72 + '\n\n').join(sections))
        version = tomllib.loads(read(CRATE / 'Cargo.toml').decode())['package']['version']
        support = {'schema': 'rust-stable-support/v1', 'release_version': version,
            'target': TARGET, 'program': binary_id, 'license': identity(package / 'LICENSE'),
            'release_status': 'candidate-review-required',
            'complexity_series': 'rust-source-decisions/v1-candidate',
            'coverage_series': 'rust-llvm-source-coverage/v1-candidate',
            'function_coverage': 'rust-llvm-exact-root-owner/v1-candidate', 'function_crap': 'unsupported',
            'build_rust': toolchain, 'observations': observed}
        write(package / 'support.json', support)
        files = {name: identity(package / name) for name in (PROGRAM, 'LICENSE', 'support.json')}
        write(package / 'release-inventory.json', {'schema': 'rust-stable-release-inventory/v1',
            'version': version, 'target': TARGET, 'files': files})
        write(stage / 'preparation.json', {'schema': 'rust-stable-preparation/v1',
            'state': 'unsigned-candidate-review-required', 'production_ready': False,
            'source': initial, 'compiler': compiler, 'commands': commands,
            'dependencies': dependencies, 'license_scope': 'conservative filtered Cargo graph including build dependencies plus Rust library notices',
            'rust_library_notices': identity(std_notice), 'payload': files,
            'inventory': identity(package / 'release-inventory.json'),
            'unsigned_package_bytes': sum(p.stat().st_size for p in package.iterdir()),
            'network_download_bytes': 0, 'signed_package_bytes': None,
            'publication_blockers': ['reviewed measurement migration', 'two toolchains and two systems',
                'production license review', 'real dual-signature acceptance', 'protected release approval'],
            'acceptance_records': [o['acceptance_sha256'] for o in observed]})
        require(identity(binary) == binary_id and identity(package / PROGRAM) == binary_id,
                'program changed while packaging')
        # Atomic publication is only into a fresh workspace review directory.
        # The unsigned package still fails the installed verifier's exact-file rule.
        os.rename(stage, output)
    return output / 'preparation.json'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--target-dir', type=Path, required=True)
    parser.add_argument('--toolchain', choices=('1.97.1', '1.98.1'), required=True)
    parser.add_argument('--acceptance', nargs=2, action='append', required=True, metavar=('SUMMARY', 'SHA256'))
    args = parser.parse_args()
    result = prepare(args.output.absolute(), args.target_dir.resolve(), args.toolchain,
                     [(Path(path), pin) for path, pin in args.acceptance])
    print(json.dumps({'preparation': str(result), 'state': 'unsigned-candidate-review-required'}))


if __name__ == '__main__':
    main()
