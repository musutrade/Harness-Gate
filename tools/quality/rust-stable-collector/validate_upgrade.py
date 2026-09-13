"""Repository-only independently built lifecycle test programs; never ship these."""
import json
import os
from pathlib import Path
import shutil
import tomllib


def build_programs(output, automation, sha):
    crate = Path(__file__).resolve().parent
    manifest = tomllib.loads((crate / 'Cargo.toml').read_text())['package']
    version = manifest['version']
    upgraded_version = version + '.upgrade-test'
    toolchain = manifest['rust-version']
    source_root = output / 'upgrade-source'
    source = source_root / 'rust-stable-collector'
    source.mkdir(parents=True)
    shutil.copytree(crate.parent / 'fixtures/rust-macro-observation',
                    source_root / 'fixtures/rust-macro-observation')
    shutil.copytree(crate / 'src', source / 'src')
    before = {}
    for name in ('Cargo.toml', 'Cargo.lock'):
        shutil.copyfile(crate / name, source / name)
    for path in sorted(source_root.rglob('*')):
        if path.is_file():
            before[str(path.relative_to(source_root))] = sha(path)
    for name, prefix in (('Cargo.toml', ''), ('Cargo.lock', f'name = "{manifest["name"]}"\n')):
        path = source / name
        original = path.read_text()
        old = prefix + f'version = "{version}"'
        new = prefix + f'version = "{upgraded_version}"'
        assert original.count(old) == 1
        path.write_text(original.replace(old, new))
    environment = dict(os.environ)
    for key in ('RUSTC_BOOTSTRAP', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC',
                'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'RUSTDOCFLAGS',
                'CARGO_ENCODED_RUSTDOCFLAGS'):
        environment.pop(key, None)
    environment.update(CARGO_TARGET_DIR=str(output / 'upgrade-target'),
                       CARGO_NET_OFFLINE='true', RUSTUP_AUTO_INSTALL='0')
    automation(['cargo', f'+{toolchain}', 'build', '--manifest-path', source / 'Cargo.toml',
                '--locked', '--offline', '--release'], timeout=600, env=environment)
    upgraded = output / 'upgrade-target/release' / manifest['name']
    after = {str(p.relative_to(source_root)): sha(p) for p in sorted(source_root.rglob('*')) if p.is_file()}
    assert before.keys() == after.keys()
    assert {p for p in before if before[p] != after[p]} == {
        'rust-stable-collector/Cargo.toml', 'rust-stable-collector/Cargo.lock'}
    record = {'scope': 'separate stable release build of the same implementation with a test-only version change; not historical or production upgrade certification',
              'toolchain': toolchain, 'version': upgraded_version, 'binary_sha256': sha(upgraded),
              'source_before': before, 'source_after': after}
    (output / 'upgrade-build.json').write_text(json.dumps(record, indent=2) + '\n')
    fixtures = {}
    for name, body in {
        'program-nonzero': 'std::process::exit(23);',
        'program-timeout': 'std::thread::sleep(std::time::Duration::from_secs(75));',
        'program-mutates-stage': 'std::fs::write(std::env::current_exe().unwrap().parent().unwrap().join("support.json"), b"changed").unwrap();',
    }.items():
        path = output / f'{name}.rs'
        path.write_text('fn main() { ' + body + f' println!("{manifest["name"]} {version}");' + ' }\n')
        executable = output / f'{name}-fixture'
        automation(['rustc', f'+{toolchain}', '--edition=2021', '--crate-name', 'lifecycle_fixture', path, '-o', executable], env=environment)
        fixtures[name] = executable
    return version, upgraded_version, upgraded, fixtures
