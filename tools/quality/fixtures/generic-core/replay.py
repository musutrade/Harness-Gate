#!/usr/bin/env python3
"""Non-authoritative Python oracle replay of the GH-146 frozen corpus."""
import argparse
from datetime import datetime
import hashlib
import gzip
import json
from pathlib import Path
import sys
import tarfile
import tempfile

QUALITY = Path(__file__).resolve().parents[2]
REPO = QUALITY.parents[1]
sys.path.insert(0, str(QUALITY))
import harness_evidence as evidence
import policy_engine as engine
import project_model as model
import project_report

ROOT = Path(__file__).resolve().parent


def canonical(value):
    return (json.dumps(value, sort_keys=True, indent=2, ensure_ascii=False,
                       allow_nan=False) + '\n').encode('utf-8')


def digest(data):
    return hashlib.sha256(data).hexdigest()


def payload(path):
    """Compression is storage only; compare exact canonical uncompressed bytes."""
    return gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_bytes()


def portable_errors(value, work):
    """Remove only the replay-owned temporary prefix from OS error reasons."""
    if isinstance(value, dict):
        return {key: (item.replace(str(work), '$CASE_ROOT')
                      if key == 'reason' and isinstance(item, str)
                      else portable_errors(item, work)) for key, item in value.items()}
    if isinstance(value, list):
        return [portable_errors(item, work) for item in value]
    return value


def evaluate(case, blobs, work):
    def context(side):
        value = case[side]
        roots = {}
        for kind in ('source', 'artifact'):
            root = work / side / kind
            root.mkdir(parents=True)
            for name, sha in value['files'][kind].items():
                model.canonical_path(name)
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(blobs[sha])
            roots[kind + '_root'] = root
        return dict(project=value['project'], expected=value['expected'], **roots)

    head = context('head')
    result = {}
    try:
        evidence.validate_evidence(case['head']['records'], **head)
        result['validation'] = dict(accepted=True)
    except (evidence.MeasurementError, model.ModelError) as error:
        result['validation'] = dict(accepted=False, reason_class=type(error).__name__,
                                    reason=str(error))
    options = {key: case[key] for key in ('selection', 'mappings', 'exceptions', 'now')
               if key in case}
    if 'now' in options:
        options['now'] = datetime.fromisoformat(options['now'].replace('Z', '+00:00'))
    if 'base' in case:
        options.update(base_records=case['base']['records'], base_context=context('base'))
    try:
        policy_result = engine.evaluate(case['policy'], case['head']['records'], **head, **options)
        result['policy_result'] = policy_result
        result['project_report'] = project_report.report(policy_result, head['project'], case['policy'])
    except (evidence.MeasurementError, model.ModelError) as error:
        result['evaluation_error'] = dict(reason_class=type(error).__name__, reason=str(error))
    return portable_errors(result, work)


def replay(root=ROOT):
    manifest = evidence.load_json(root / 'manifest.json')
    if manifest['schema'] != 'generic-core-corpus/v1':
        raise ValueError('unknown corpus schema')
    for name, sha in manifest['files'].items():
        if digest((root / name).read_bytes()) != sha:
            raise ValueError('changed corpus file: ' + name)
    for name, sha in manifest['schemas'].items():
        if digest((REPO / name).read_bytes()) != sha:
            raise ValueError('changed external schema: ' + name)
    with tarfile.open(root / 'artifacts.tar.gz') as archive:
        blobs = {}
        for member in archive.getmembers():
            if not member.isfile() or member.name in blobs:
                raise ValueError('invalid corpus blob member')
            data = archive.extractfile(member).read()
            if digest(data) != member.name:
                raise ValueError('changed corpus blob: ' + member.name)
            blobs[member.name] = data
    results = []
    with tempfile.TemporaryDirectory() as directory:
        for item in manifest['cases']:
            case = json.loads(payload(root / item['input']))
            actual = evaluate(case, blobs, Path(directory) / item['id'])
            expected = payload(root / item['expected'])
            if canonical(actual) != expected:
                raise ValueError('oracle mismatch: ' + item['id'])
            results.append(dict(id=item['id'], state=actual.get('policy_result', {}).get(
                'aggregate', {}).get('state', 'rejected')))
    return dict(schema='generic-core-corpus-replay/v1', authoritative=False,
                cases=results, passed=len(results))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = replay()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(canonical(result))
    print(f"Passed {result['passed']} frozen Python oracle cases (non-authoritative)")
