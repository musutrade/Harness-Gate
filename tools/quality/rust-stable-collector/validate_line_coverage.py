"""Real-capture line normalization and corruption checks; no historical re-seal."""
import json


def validate(output, run, digest):
    observed = {}
    for case, covered in [('plain', 5), ('partial', 4)]:
        capture = output / f'capture-{case}'
        manifest = capture / 'manifest.json'
        saved_manifest = manifest.read_bytes()
        request = json.loads(saved_manifest)['request_sha256']
        description = run(f'certified-line-{case}', ['describe', capture, digest(manifest), request])
        owners = [o['coverage_owner'] for o in description['owners']]
        assert [o['coverage_line'] for o in owners] == [
            {'type': 'ratio', 'covered': covered, 'total': 5},
            {'type': 'ratio', 'covered': 0, 'total': 3}]
        observed[case] = owners
        coverage = capture / 'coverage.json'
        saved = coverage.read_bytes()
        try:
            for label, mutate, expected in [
                ('segment-count', lambda v: v['files'][0]['segments'][0].__setitem__(2, 99), 'owner lines disagree'),
                ('segment-entry', lambda v: v['files'][0]['segments'][0].__setitem__(4, False), 'owner lines disagree'),
                ('segment-position', lambda v: v['files'][0]['segments'][-1].__setitem__(0, 2**32-1), 'outside authenticated source'),
                ('summary-lines', change_summary, 'owner lines disagree with file summary'),
            ]:
                raw = json.loads(saved)
                mutate(raw['data'][0])
                coverage.write_text(json.dumps(raw))
                updated = json.loads(saved_manifest)
                updated['files']['coverage.json'] = {'sha256': digest(coverage), 'bytes': coverage.stat().st_size}
                manifest.write_text(json.dumps(updated))
                result = run(f'line-{label}-{case}', ['verify', capture, digest(manifest), request], success=False)
                assert expected in result, result
        finally:
            coverage.write_bytes(saved)
            manifest.write_bytes(saved_manifest)
    return observed


def change_summary(raw):
    # Internally consistent file and aggregate rows still contradict real owners.
    for summary in (raw['files'][0]['summary'], raw['totals']):
        lines = summary['lines']
        lines['count'] += 1
        lines['percent'] = 100 * lines['covered'] / lines['count']
        if 'notcovered' in lines:
            lines['notcovered'] = lines['count'] - lines['covered']
