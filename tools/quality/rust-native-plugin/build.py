#!/usr/bin/env python3
"""Build the standalone code-only native plugin; never downloads dependencies."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tomllib

HERE = Path(__file__).resolve().parent
QUALITY = HERE.parent
MODULES = ('rust_native_driver.py', 'rust_native_classify.py', 'rust_native_policy.py',
           'rust_collector_project.py', 'harness_evidence.py', 'project_model.py', 'quality_evidence.py')


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


def licenses(output, rustc):
    # Reuse the repository's locked-archive/source verification; this is build
    # preparation only and is not part of either measurement engine.
    verifier = module('locked_notices', HERE / 'locked_notices.py')
    version = subprocess.check_output([str(rustc), '-vV'], text=True)
    if 'commit-hash: 8bab26f4f68e0e26f0bb7960be334d5b520ea452\n' not in version:
        raise ValueError('build launcher with the same pinned Rust 1.97.1 compiler as the driver')
    sysroot = Path(subprocess.check_output([str(rustc), '--print', 'sysroot'], text=True).strip())
    manifest = QUALITY / 'rust-native-driver/Cargo.toml'
    metadata = json.loads(subprocess.check_output([str(sysroot / 'bin/cargo'), 'metadata',
        '--manifest-path', str(manifest), '--locked', '--offline', '--format-version', '1',
        '--filter-platform', 'x86_64-unknown-linux-gnu']))
    lock = {(p['name'], p['version'], p.get('source')): p
            for p in tomllib.loads(manifest.with_name('Cargo.lock').read_text())['package']}
    sections = [(QUALITY.parents[1] / 'LICENSE').read_text()]
    for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
        if package['source']:
            notices, _ = verifier.registry_notices(package, lock)
            sections.extend(f'{package["name"]} {package["version"]} / {name}\n{text}'
                            for name, text in sorted(notices.items()))
    sections.append('Rust standard library 1.97.1\n' +
                    (sysroot / 'share/doc/rust/COPYRIGHT-library.html').read_text())
    path = output / 'LICENSE'
    path.write_text(('\n\n' + '=' * 72 + '\n\n').join(sections))
    metadata_path = output / 'cargo-metadata.json'
    metadata_path.write_text(json.dumps(metadata))
    return path, metadata_path


def build(driver, output, rustc):
    driver = driver.resolve(strict=True)
    if driver.read_bytes()[:4] != b'\x7fELF':
        raise ValueError('a precompiled Linux native driver is required')
    version = tomllib.loads((HERE / 'Cargo.toml').read_text())['package']['version']
    output.mkdir(parents=True, exist_ok=False)
    notice, metadata = licenses(output, rustc)
    original_driver_sha256 = hashlib.sha256(driver.read_bytes()).hexdigest()
    staged_driver = output / 'native-driver'
    shutil.copyfile(driver, staged_driver)
    subprocess.run(['strip', '--strip-unneeded', str(staged_driver)], check=True)
    driver = staged_driver.resolve()
    payload = {'bin/harness-gate-rust-native-driver': (driver, 0o755),
               'app/native_external.py': (HERE / 'native_external.py', 0o644)}
    payload.update({'app/' + name: (QUALITY / name, 0o644) for name in MODULES})
    payload.update({'app/schema/' + p.name: (p, 0o644) for p in sorted((QUALITY / 'schema').glob('*.json'))})
    payload['LICENSE'] = (notice, 0o644)
    inventory = {name: {'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'bytes': path.stat().st_size, 'mode': mode}
                 for name, (path, mode) in payload.items()}
    identity = hashlib.sha256(json.dumps(inventory, sort_keys=True).encode()).hexdigest()
    embedded = output / 'payload.rs'
    rows = ',\n'.join(f'({json.dumps(name)}, include_bytes!({json.dumps(str(path))}), {mode})'
                     for name, (path, mode) in sorted(payload.items()))
    embedded.write_text(f'const VERSION: &str = {json.dumps(version)};\nconst PAYLOAD_ID: &str = {json.dumps(identity)};\n'
                        f'const PAYLOAD: &[(&str, &[u8], u32)] = &[\n{rows}\n];\n')
    binary = output / 'harness-gate-rust-collector-linux-amd64'
    subprocess.run([str(rustc), '--edition=2021', '-C', 'opt-level=s', '-C', 'strip=symbols',
                    str(HERE / 'launcher.rs'), '-o', str(binary)], check=True,
                   env=os.environ | {'HARNESS_GATE_NATIVE_PAYLOAD': str(embedded.resolve())})
    if inventory != {name: {'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'bytes': path.stat().st_size, 'mode': mode}
                     for name, (path, mode) in payload.items()}:
        raise ValueError('plugin inputs changed during build')
    record = {'schema': 'native-external-plugin-build/v1', 'version': version, 'payload': inventory,
              'original_driver_sha256': original_driver_sha256,
              'payload_id': identity, 'binary': str(binary), 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
              'external_dependencies': ['Rust 1.97.1 8bab26f4f68e0e26f0bb7960be334d5b520ea452',
                  'matching rustc_driver/LLVM libraries and sysroot', 'LLVM coverage tools 22.1.6', 'Python >=3.12', 'system linker'],
              'bundled_toolchains': False, 'measurement_engine': 'rust-native-production-mir-block/1'}
    (output / 'build.json').write_text(json.dumps(record, indent=2) + '\n')
    sbom_path = output / 'harness-gate-rust-collector.sbom.cdx.json'
    sbom_tool = module('sbom', QUALITY.parent / 'release/generate-sbom.py')
    sbom_tool.run(metadata, QUALITY / 'rust-native-driver/Cargo.lock', sbom_path)
    sbom = json.loads(sbom_path.read_text())
    sbom['metadata']['component'] = sbom_tool.component({'name': 'harness-gate-rust-collector', 'version': version})
    sbom['metadata']['properties'].extend([
        {'name': 'native.external-build', 'value': json.dumps(record, sort_keys=True)},
        {'name': 'license.location', 'value': 'embedded LICENSE; run the executable with --licenses'}])
    sbom_path.write_text(json.dumps(sbom, indent=2, sort_keys=True) + '\n')
    print(json.dumps({'binary': str(binary), 'bytes': binary.stat().st_size, 'sha256': record['binary_sha256']}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--driver', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--rustc', type=Path, default=Path(shutil.which('rustc') or 'rustc'))
    args = parser.parse_args()
    build(args.driver, args.output.resolve(), args.rustc)
