"""Repository-only compiler file-input acceptance, using real stable builds."""
import json
from pathlib import Path
import shutil


def validate(output, run, digest, trace):
    fixture = Path(__file__).resolve().parent.parent / 'fixtures/rust-stable/plain'
    project = output / 'input-project'
    shutil.copytree(fixture, project)
    source = project / 'src/lib.rs'
    original = source.read_text()
    doctor = output / 'doctor/doctor.json'

    def collect(label, prefix, success):
        source.write_text(prefix + original)
        capture = output / f'capture-{label}'
        request = run(f'prepare-{label}', ['prepare', project, capture, doctor])
        request_path = output / f'request-{label}.json'
        request_path.write_text(json.dumps(request))
        result = run(label, ['collect', request_path], success=success, trace=trace)
        if not success:
            assert 'compiler input outside authenticated inventory' in result, result
            assert not (capture / 'manifest.json').exists()
        return capture, result

    outside = output / 'outside.txt'
    outside.write_text('external input')
    collect('external-include-bytes', 'pub const EXTERNAL: &[u8] = include_bytes!("../../outside.txt");\n', False)
    outside_rs = output / 'outside.rs'
    outside_rs.write_text('pub const EXTERNAL: u8 = 1;\n')
    collect('external-include-rust', 'include!("../../outside.rs");\n', False)
    collect('external-path-module', '#[path = "../../outside.rs"] mod outside;\n', False)

    (project / 'data.txt').write_text('authenticated data')
    capture, anchor = collect('workspace-include-bytes', 'pub const DATA: &[u8] = include_bytes!("../data.txt");\npub const PACKAGE: &str = env!("CARGO_PKG_NAME");\n', True)
    args = ['verify', capture, anchor['manifest_sha256'], anchor['request_sha256']]
    run('verify-workspace-include', args)
    proof_path = capture / 'compiler-inputs.json'
    manifest_path = capture / 'manifest.json'
    original_proof = proof_path.read_bytes()
    original_manifest = manifest_path.read_bytes()
    proof = json.loads(original_proof)
    assert any(str(project / 'data.txt') == item['path'] for record in proof['records'].values() for item in record['inputs'].values())
    env_record = next(name for name, record in proof['records'].items() if '# env-dep:CARGO_PKG_NAME=' in record['raw'])
    env_line = next(line for line in proof['records'][env_record]['raw'].splitlines(keepends=True) if line.startswith('# env-dep:CARGO_PKG_NAME='))

    def mutated(label, change, error):
        value = json.loads(original_proof)
        change(value)
        proof_path.write_text(json.dumps(value))
        manifest = json.loads(original_manifest)
        manifest['files']['compiler-inputs.json'] = {'sha256': digest(proof_path), 'bytes': proof_path.stat().st_size}
        manifest_path.write_text(json.dumps(manifest))
        assert error in run(label, ['verify', capture, digest(manifest_path), anchor['request_sha256']], success=False)

    try:
        mutated('empty-compiler-proof', lambda value: value.update(records={}), 'invalid compiler input proof')
        mutated('omitted-compiler-input', lambda value: next(iter(value['records'].values()))['inputs'].popitem(), 'dep-info input set differs')
        mutated('malformed-dep-info', lambda value: next(iter(value['records'].values())).update(raw='a: $(untrusted)'), 'unsupported dep-info syntax')
        mutated('wrong-compiler-cwd', lambda value: next(iter(value['records'].values())).update(cwd=str(output)), 'compiler cwd identity differs')
        mutated('omitted-dep-info-producer', lambda value: value.update(records={'unobserved.d': next(iter(value['records'].values()))}), 'compiler dep-info producer set differs')
        mutated('unobserved-env-dep', lambda value: value['records'][env_record].update(raw=value['records'][env_record]['raw'] + '\n# env-dep:GH259_UNOBSERVED=changed\n'), 'compiler dep-info bytes differ from producer')
        mutated('changed-env-dep', lambda value: value['records'][env_record].update(raw=value['records'][env_record]['raw'].replace(env_line, '# env-dep:CARGO_PKG_NAME=changed\n')), 'compiler dep-info bytes differ from producer')
        mutated('omitted-env-dep', lambda value: value['records'][env_record].update(raw=value['records'][env_record]['raw'].replace(env_line, '')), 'compiler dep-info bytes differ from producer')
    finally:
        proof_path.write_bytes(original_proof)
        manifest_path.write_bytes(original_manifest)
    producer_path = next(path for path in (capture / 'compiler-invocations').iterdir() if json.loads(path.read_text()).get('dep_info_identity'))
    original_producer = producer_path.read_bytes()
    try:
        for label, identity, error in [
            ('missing-dep-info-identity', None, 'compiler dep-info identity missing'),
            ('changed-dep-info-identity', {'bytes': 1, 'sha256': '0' * 64}, 'compiler dep-info bytes differ from producer'),
        ]:
            invocation = json.loads(original_producer)
            invocation['dep_info_identity'] = identity
            producer_path.write_text(json.dumps(invocation))
            manifest = json.loads(original_manifest)
            manifest['files'][str(producer_path.relative_to(capture))] = {'sha256': digest(producer_path), 'bytes': producer_path.stat().st_size}
            manifest_path.write_text(json.dumps(manifest))
            assert error in run(label, ['verify', capture, digest(manifest_path), anchor['request_sha256']], success=False)
    finally:
        producer_path.write_bytes(original_producer)
        manifest_path.write_bytes(original_manifest)
    run('restored-compiler-input-proof', args)

    # Generated source is retained as observed bytes, with no certification of
    # the build script's arbitrary reads or the generated function's metrics.
    generated_capture = output / 'capture-boundaries'
    generated_proof = json.loads((generated_capture / 'compiler-inputs.json').read_text())
    generated = [item for record in generated_proof['records'].values() for item in record['inputs'].values() if item['generated']]
    assert generated
    for item in generated:
        assert digest(generated_capture / 'compiler-generated' / item['identity']['sha256']) == item['identity']['sha256']
    return {'external_inputs': 'three real captures blocked before manifest',
            'workspace_data': 'identity verified', 'generated_inputs': len(generated),
            'dep_info_bytes': 'compiler-output digest binds complete dep-info including three env-dep mutations; absent and inconsistent producer identities rejected',
            'scope': 'observed rustc dep-info bytes and file inputs only; environment semantics, arbitrary build-script reads and environment closure are not certified'}
