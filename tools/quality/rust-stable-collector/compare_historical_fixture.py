#!/usr/bin/env python3
"""Repository-only migration probe; never executes or re-seals legacy capture.

Read just three ordinary archive members, retain their original identifiers, and
run the current stable collector on a byte-identical copy of historical source.
The Cargo harness is a new build boundary and is explicitly recorded as such.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    output = args.output.absolute()
    output.mkdir(parents=True, exist_ok=False)
    repository = Path(__file__).resolve().parents[3]
    archive_path = Path('docs/quality/gh-220/native-fixtures.tar.gz')
    archive = repository / archive_path
    index_path = Path('docs/quality/gh-220/native-fixtures.json')
    index = json.loads((repository / index_path).read_bytes())
    original = next(row for row in index if row['name'] == 'complete')
    members = {}
    with tarfile.open(archive) as stream:
        for name in ('complete/evidence.json', 'complete/report.json', 'complete/raw/source.rs'):
            member = stream.getmember(name)
            assert member.isfile() and member.size < 1024 * 1024
            members[name] = stream.extractfile(member).read()
    manifest = json.loads(members['complete/evidence.json'])
    report = json.loads(members['complete/report.json'])
    source = members['complete/raw/source.rs']
    assert sha(members['complete/evidence.json']) == original['evidence_sha256'] == report['evidence_sha256']
    assert sha(source) == manifest['artifacts']['source.rs'] == report['source_sha256']
    assert report['coverage'] == original['coverage']
    project = output / 'source'
    project.mkdir()
    (project / 'source.rs').write_bytes(source)
    (project / 'Cargo.toml').write_text('''[package]
name = "historical-stable-probe"
version = "0.1.0"
edition = "2021"
autobins = false

[workspace]

[features]
extra = []

[[test]]
name = "historical_probe"
path = "source.rs"
harness = false
''')
    (project / 'Cargo.lock').write_text('''version = 4

[[package]]
name = "historical-stable-probe"
version = "0.1.0"
''')

    def run(label, *arguments):
        command = [str(binary), *map(str, arguments)]
        (output / f'{label}.command.json').write_text(json.dumps(command, indent=2) + '\n')
        result = subprocess.run(command, capture_output=True, timeout=360)
        (output / f'{label}.stdout').write_bytes(result.stdout)
        (output / f'{label}.stderr').write_bytes(result.stderr)
        assert result.returncode == 0, (label, result.stderr.decode())
        return json.loads(result.stdout)

    run('doctor', 'doctor', project, output / 'doctor')
    request = run('prepare', 'prepare', project, output / 'capture-historical', output / 'doctor/doctor.json')
    request_path = output / 'request.json'
    request_path.write_text(json.dumps(request, indent=2) + '\n')
    anchor = run('historical', 'collect', request_path)
    run('verify', 'verify', output / 'capture-historical', anchor['manifest_sha256'], anchor['request_sha256'])
    description = run('description', 'describe', output / 'capture-historical', anchor['manifest_sha256'], anchor['request_sha256'])
    coverage = json.loads((output / 'capture-historical/coverage.json').read_bytes())
    assert (project / 'source.rs').read_bytes() == source
    assert all(not row['file_supported'] for row in description['owners'])
    summary = {
        'schema': 'rust-stable-historical-comparison/v1',
        'state': 'migration-review-required',
        'collector_sha256': sha(binary.read_bytes()),
        'historical': {
            'archive': str(archive_path), 'archive_sha256': sha(archive.read_bytes()),
            'index': str(index_path), 'original_workspace_path': original['workspace_path'],
            'members': {name: sha(data) for name, data in members.items()},
            'source_sha256': sha(source), 'series': report['series'], 'scope': report['scope'],
            'coverage': report['coverage'], 'backend_complete': report['backend_complete'],
            'source_provenance_complete': report['source_provenance_complete'],
            'functions': [{k: row[k] for k in ('name', 'kind', 'cc', 'count', 'crap_exact')} for row in report['functions']],
            'threshold_failures': report['threshold_failures'],
            'authentication': 'original manifest/source anchors checked; report is an archived summary, not re-certified',
        },
        'stable': {'anchor': anchor, 'series': description['series'],
                   'llvm_totals': coverage['data'][0]['totals'], 'owners': description['owners'],
                   'build_boundary': 'Cargo harness=false test target runs unchanged source main; no claim of identical native build flags'},
        'migration': {'comparable_series': False, 'baseline_adopted': False,
                      'required_binding_changed': False, 'function_coverage_and_crap': 'unsupported',
                      'raw_coverage': 'standard LLVM totals are diagnostic; test/production boundary is not certified'},
    }
    (output / 'comparison.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps({'comparison': str(output / 'comparison.json'), 'state': summary['state']}))


if __name__ == '__main__':
    main()
