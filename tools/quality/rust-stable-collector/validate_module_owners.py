"""Repository-only real stable coverage checks for inline module owners."""
import json
from pathlib import Path
import shutil


def validate(output, run, write_request, digest):
    fixture = Path(__file__).resolve().parent.parent / 'fixtures/rust-stable/modules'
    project = output / 'modules'
    shutil.copytree(fixture, project)
    request = write_request('modules', project)
    anchor = run('modules', ['collect', request], trace=True)
    capture = output / 'capture-modules'
    args = ['describe', capture, anchor['manifest_sha256'], anchor['request_sha256']]
    description = run('certified-module-owners', args)
    owners = description['owners']
    assert [o['function']['name'] for o in owners] == [
        'left::classify', 'left::nested::never_called', 'right::classify']
    assert [o['function']['complexity'] for o in owners] == [2, 1, 2]
    assert all(o['coverage_state'] == 'supported' for o in owners)
    mapped = [o['coverage_owner'] for o in owners]
    assert [o['execution_count'] for o in mapped] == [1, 0, 2]
    assert [o['coverage_function'] for o in mapped] == [
        {'type': 'ratio', 'covered': 1, 'total': 1},
        {'type': 'ratio', 'covered': 0, 'total': 1},
        {'type': 'ratio', 'covered': 1, 'total': 1}]
    assert [o['coverage_region'] for o in mapped] == [
        {'type': 'ratio', 'covered': 4, 'total': 5},
        {'type': 'ratio', 'covered': 0, 'total': 3},
        {'type': 'ratio', 'covered': 5, 'total': 5}]
    assert len({o['symbol'] for o in mapped}) == 3
    analysis = json.loads((capture / 'source-analysis.json').read_text())
    assert len(analysis['files']['src/lib.rs']['excluded_spans']) == 2
    coverage = capture / 'coverage.json'
    manifest = capture / 'manifest.json'
    original_coverage, original_manifest = coverage.read_bytes(), manifest.read_bytes()
    raw = json.loads(original_coverage)
    assert raw['data'][0]['totals']['regions']['count'] > 13

    def mutated(label, change, error):
        altered = json.loads(original_coverage)
        change(altered['data'][0]['functions'])
        coverage.write_text(json.dumps(altered))
        updated = json.loads(original_manifest)
        updated['files']['coverage.json'] = {'sha256': digest(coverage), 'bytes': coverage.stat().st_size}
        manifest.write_text(json.dumps(updated))
        result = run(label, ['verify', capture, digest(manifest), anchor['request_sha256']], success=False)
        assert error in result, result

    try:
        unused = mapped[1]['llvm_function_index']
        executed = mapped[0]['llvm_function_index']
        mutated('missing-module-owner', lambda records: records.pop(unused), 'source owner missing')
        mutated('duplicate-module-owner', lambda records: records.append({**records[executed], 'name': 'duplicate-module-symbol'}), 'multiple LLVM records')
        mutated('module-parent-count-inheritance', lambda records: records[unused]['regions'][1].__setitem__(4, 1), 'unexecuted LLVM owner')
    finally:
        coverage.write_bytes(original_coverage)
        manifest.write_bytes(original_manifest)
    run('restored-module-owners', ['verify', capture, anchor['manifest_sha256'], anchor['request_sha256']])

    # Real compilation still succeeds, but source activation/annotated module
    # ownership has not been certified. No counts are promoted through it.
    for label, prefix in [('module-attribute', '#[allow(dead_code)]\n'),
                          ('module-cfg', '#[cfg(any())] mod inactive { pub fn dormant() {} }\n')]:
        variant = output / label
        shutil.copytree(fixture, variant)
        source = variant / 'src/lib.rs'
        source.write_text(prefix + source.read_text())
        request = write_request(label, variant)
        blocked = run(label, ['collect', request], trace=True)
        result = run(f'describe-{label}', ['describe', output / f'capture-{label}', blocked['manifest_sha256'], blocked['request_sha256']])
        assert result['owners'] and all(o['coverage_state'] == 'unsupported' and o['coverage_owner'] is None for o in result['owners'])
    return {'anchors': anchor, 'function_owners': mapped,
            'scope': 'unannotated free functions in unannotated inline modules; same-name and unexecuted owners independently matched; nested tests excluded; annotated/cfg module coverage remains unsupported'}
