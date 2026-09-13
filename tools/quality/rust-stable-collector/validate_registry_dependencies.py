"""Repository-only real registry fixture checks using a workspace-local cache.

Cache layout knowledge is test setup only. The shipped Rust collector receives
explicit archive paths and never infers Cargo cache directories.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tomllib


def validate(output, run, digest, trace):
    crate = Path(__file__).resolve().parent
    fixture = crate.parent / 'fixtures/rust-stable/registry'
    project = output / 'project-registry'
    shutil.copytree(fixture, project)
    # The collector's locked graph already includes the exercised itoa crate.
    command = ['cargo', 'metadata', '--format-version', '1', '--locked', '--offline',
               '--manifest-path', str(project / 'Cargo.toml')]
    setup = subprocess.run(command, capture_output=True)
    (output / 'registry-setup.stdout').write_bytes(setup.stdout)
    (output / 'registry-setup.stderr').write_bytes(setup.stderr)
    setup.check_returncode()
    package = next(p for p in json.loads(setup.stdout)['packages'] if p['name'] == 'itoa' and p['version'] == '1.0.18')
    cached_source = Path(package['manifest_path']).parent
    namespace = cached_source.parent.name
    registry = cached_source.parents[2]
    home = output / 'registry-cargo-home'
    local_source = home / 'registry/src' / namespace / cached_source.name
    shutil.copytree(cached_source, local_source)
    # Copy only this dependency and the sparse index metadata Cargo needs.
    index = home / 'registry/index' / namespace
    (index / '.cache/it/oa').mkdir(parents=True)
    for name in ('config.json', '.cache/it/oa/itoa'):
        shutil.copyfile(registry / 'index' / namespace / name, index / name)
    archive = home / 'registry/cache' / namespace / (cached_source.name + '.crate')
    archive.parent.mkdir(parents=True)
    shutil.copyfile(registry / 'cache' / namespace / archive.name, archive)
    lock = tomllib.loads((project / 'Cargo.lock').read_text())
    checksum = next(p['checksum'] for p in lock['package'] if p['name'] == 'itoa')
    assert digest(archive) == checksum
    archives = output / 'registry-archives.json'
    archives.write_text(json.dumps({checksum: str(archive)}))
    env = {**os.environ, 'CARGO_HOME': str(home)}
    doctor = output / 'doctor/doctor.json'
    capture = output / 'capture-registry'
    request = run('prepare-registry', ['prepare', project, capture, doctor, '--registry-archives', archives], env=env)
    request_path = output / 'request-registry.json'
    request_path.write_text(json.dumps(request))
    anchor = run('registry', ['collect', request_path], env=env, trace=trace)
    args = ['verify', capture, anchor['manifest_sha256'], anchor['request_sha256']]
    run('verify-registry', args, env=env)
    description = run('registry-function-owner', ['describe', *args[1:]], env=env)
    assert len(description['owners']) == 1
    assert description['owners'][0]['coverage_owner']['coverage_function'] == {'type': 'ratio', 'covered': 1, 'total': 1}
    proof_path = capture / 'dependencies.json'
    proof = json.loads(proof_path.read_text())
    registry_proof = next(p for p in proof['packages'].values() if p['kind'] == 'locked-registry-archive')
    assert registry_proof['checksum'] == checksum and Path(registry_proof['source_root']) == local_source

    def collect_changed(label, changes, error):
        path = output / f'request-{label}.json'
        path.write_text(json.dumps({**request, 'output_root': str(output / f'capture-{label}'), **changes}))
        assert error in run(label, ['collect', path], success=False, env=env)
        assert not (output / f'capture-{label}/manifest.json').exists()

    collect_changed('missing-registry-archive', {'registry_archives': {}}, 'locked registry archive missing')
    archive_bytes = archive.read_bytes()
    try:
        archive.write_bytes(archive_bytes + b'tampered')
        assert 'registry archive checksum mismatch' in run('changed-registry-archive', args, success=False, env=env)
    finally:
        archive.write_bytes(archive_bytes)
    source = local_source / 'src/lib.rs'
    original_source = source.read_bytes()
    try:
        source.write_bytes(original_source + b'\n// poisoned cache\n')
        assert 'cached source differs from locked archive' in run('changed-registry-source', args, success=False, env=env)
        collect_changed('poisoned-registry-before-build', {}, 'cached source differs from locked archive')
    finally:
        source.write_bytes(original_source)
    extra = local_source / 'unlisted.rs'
    try:
        extra.write_text('// not in locked archive')
        assert 'undeclared cached crate files' in run('extra-registry-source', args, success=False, env=env)
    finally:
        extra.unlink()
    alias = local_source / 'alias.rs'
    try:
        alias.symlink_to('src/lib.rs')
        assert 'symlink or special file' in run('registry-source-symlink', args, success=False, env=env)
    finally:
        alias.unlink()

    manifest_path = capture / 'manifest.json'
    original_manifest = manifest_path.read_bytes()
    original_proof = proof_path.read_bytes()
    metadata_path = capture / 'cargo-metadata.json'
    original_metadata = metadata_path.read_bytes()
    try:
        mutations = [
            ('omitted-dependency-provenance', proof_path, {'schema': proof['schema'], 'packages': {}}, 'dependency provenance differs'),
            ('omitted-metadata-dependency', metadata_path, {**json.loads(original_metadata), 'packages': [p for p in json.loads(original_metadata)['packages'] if p['source'] is None]}, 'lock/metadata package inventory mismatch'),
        ]
        for label, path, value, error in mutations:
            path.write_text(json.dumps(value))
            manifest = json.loads(original_manifest)
            manifest['files'][path.name] = {'sha256': digest(path), 'bytes': path.stat().st_size}
            manifest_path.write_text(json.dumps(manifest))
            assert error in run(label, ['verify', capture, digest(manifest_path), anchor['request_sha256']], success=False, env=env)
            proof_path.write_bytes(original_proof)
            metadata_path.write_bytes(original_metadata)
    finally:
        proof_path.write_bytes(original_proof)
        metadata_path.write_bytes(original_metadata)
        manifest_path.write_bytes(original_manifest)
    run('restored-registry-integrity', args, env=env)
    return {'anchors': anchor, 'archive_sha256': checksum, 'source_files': len(registry_proof['files']),
            'setup_command': command, 'setup_is_repository_automation': True,
            'isolated_cargo_home': str(home), 'network_download_bytes': 0,
            'scope': 'single locked crates.io dependency; local cache copied by test setup'}
