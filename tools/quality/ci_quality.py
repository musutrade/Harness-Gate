#!/usr/bin/env python3
"""Required CI policy and fresh, review-only quality candidates (schema 1)."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import time
import uuid

from critical_paths import require_committed_sources
from production_coverage import require
from quality_common import ROOT, git_sha, metadata, sha256, write_json
from source_measure import HOTSPOTS

COMMON = ('test', 'security-audit', 'fmt', 'clippy', 'build', 'quality-coverage',
          'quality-contracts', 'docs-consistency', 'release-contracts', 'quality-scripts')
PUSH_ONLY = ('test-cross-platform', 'build-cross-platform', 'coverage',
             'quality-contracts-cross-platform', 'quality-baseline')
STAGES = ('legacy', 'production', 'risk', 'matrix')
REQUIRED_ARTIFACTS = {'coverage.json', 'coverage.raw.json', 'coverage.lcov',
                      'coverage.cobertura.xml', 'production.json', 'risk.json',
                      'critical-paths.json', 'critical-path-runs/bundle.json'} | {
    f'{label}-{suffix}' for label in ('base', 'head') for suffix in (
        'source.tar', 'manifest.json', 'coverage.json', 'coverage.lcov',
        'coverage.cobertura.xml', 'risk.json')}


def aggregate(event: str, needs: dict) -> list[str]:
    require(event in ('push', 'pull_request'), 'unsupported CI event')
    required = COMMON + (PUSH_ONLY if event == 'push' else ())
    return [name for name in required if needs.get(name, {}).get('result') != 'success']


def verify(path: Path, head: str, base: str, run_id: str) -> dict:
    report = json.loads(path.read_text())
    require(report['schema_version'] == 1 and report['candidate'] is True, 'invalid candidate schema')
    require((report['commit'], report['base_sha'], report['run_id']) == (head, base, run_id),
            'stale or mixed candidate identity')
    require(set(report['stages']) == set(STAGES), 'missing required evidence stage')
    require(all(s['status'] == 'success' for s in report['stages'].values()),
            'failed, cancelled, skipped or incomplete collection')
    require(REQUIRED_ARTIFACTS <= report['artifacts'].keys(), 'missing required raw evidence')
    for relative, digest in report['artifacts'].items():
        artifact = (path.parent / relative).resolve()
        require(artifact.is_relative_to(path.parent.resolve()), 'artifact escapes candidate')
        require(sha256(artifact) == digest, f'stale or modified artifact: {relative}')
    return report


class Collector:
    def __init__(self, directory: Path, base: str, head: str, run_id: str):
        self.directory = directory.resolve()
        require(not self.directory.exists(), 'candidate directory already exists; choose a fresh path')
        self.directory.mkdir(parents=True)
        self.environment = dict(os.environ)
        for key in ('LLVM_PROFILE_FILE', 'CARGO_BUILD_TARGET', 'CARGO_LLVM_COV_TARGET_DIR',
                    'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS'):
            self.environment.pop(key, None)
        self.environment['CARGO_TARGET_DIR'] = str(self.directory / 'build')
        self.report = {**metadata(), 'schema_version': 1, 'candidate': True,
                       'base_sha': base, 'commit': head, 'run_id': run_id,
                       'stages': {}, 'commands': [], 'artifacts': {}}
        self.save()

    def save(self):
        write_json(self.directory / 'candidate.json', self.report)

    def command(self, name: str, command: list[str], allowed=(0,)):
        started = time.monotonic()
        with (self.directory / f'{name}.log').open('w') as log:
            code = subprocess.run(command, cwd=ROOT, env=self.environment,
                                  stdout=log, stderr=subprocess.STDOUT).returncode
        self.report['commands'].append({'name': name, 'argv': command, 'exit_code': code,
                                        'seconds': time.monotonic() - started})
        self.save()
        require(code in allowed, f'{name} exited {code}; see {name}.log')

    def python(self, name: str, script: str, *args: str, allowed=(0,)):
        self.command(name, [sys.executable, str(ROOT / 'tools/quality' / script), *map(str, args)], allowed)

    def legacy(self):
        self.python('legacy', 'coverage.py', '--output', self.directory / 'coverage.json')

    def production(self):
        self.python('production', 'coverage.py', '--production', '--raw', self.directory / 'coverage.raw.json',
                    '--lcov', self.directory / 'coverage.lcov', '--output', self.directory / 'production.json')

    def risk(self):
        base, head = self.report['base_sha'], self.report['commit']
        changed = subprocess.check_output(['git', 'diff', '--name-only', base, head, '--',
                                            'tools/harness-gate/src'], cwd=ROOT, text=True).splitlines()
        unsupported = [p for p in changed if p.endswith('.rs') and
                       p.removeprefix('tools/harness-gate/src/') not in HOTSPOTS]
        require(not unsupported, 'production changes outside supported risk series; measurement review required: '
                + ', '.join(unsupported))
        self.command('analyzer-build', ['cargo', 'build', '--locked', '--manifest-path',
                                      str(ROOT / 'tools/quality/rust-measure/Cargo.toml')])
        binary = Path(self.environment['CARGO_TARGET_DIR']) / 'debug/harness-gate-rust-measure'
        for label, commit in (('base', base), ('head', head)):
            snapshot = self.directory / 'snapshots' / label
            snapshot.mkdir(parents=True)
            archive = self.directory / f'{label}-source.tar'
            self.command(f'{label}-archive', ['git', 'archive', '--format=tar', f'--output={archive}',
                                              commit, 'tools/harness-gate', 'tools/quality/fixtures'])
            with tarfile.open(archive) as source:
                source.extractall(snapshot, filter='data')
            crate = snapshot / 'tools/harness-gate'
            manifest = self.directory / f'{label}-manifest.json'
            self.python(f'{label}-prepare', 'source_measure.py', 'prepare', '--crate', crate,
                        '--binary', binary, '--manifest', manifest)
            cargo_manifest = str(crate / 'Cargo.toml')
            self.command(f'{label}-coverage', ['cargo', 'llvm-cov', 'nextest', '--locked',
                         '--manifest-path', cargo_manifest, '--json', '--output-path',
                         str(self.directory / f'{label}-coverage.json')])
            for flag, suffix in (('--lcov', 'lcov'), ('--cobertura', 'cobertura.xml')):
                self.command(f'{label}-{suffix}', ['cargo', 'llvm-cov', 'report', '--manifest-path',
                             cargo_manifest, flag, '--output-path', str(self.directory / f'{label}-coverage.{suffix}')])
            # A complete base report may contain historical selected failures.
            # The compare command enforces every selected head threshold.
            self.python(f'{label}-measure', 'source_measure.py', 'measure', '--crate', crate,
                        '--binary', binary, '--manifest', manifest, '--llvm',
                        self.directory / f'{label}-coverage.json', '--output',
                        self.directory / f'{label}-risk.json', allowed=(0, 1))
            report = json.loads((self.directory / f'{label}-risk.json').read_text())
            require(report['manifest_sha256'] == sha256(manifest), 'stale risk manifest')
            require(report['llvm_sha256'] == sha256(self.directory / f'{label}-coverage.json'), 'stale risk profiles')
            original = json.loads(manifest.read_text())
            for path, item in original['files'].items():
                committed = subprocess.check_output(['git', 'show', f'{commit}:tools/harness-gate/src/{path}'], cwd=ROOT)
                require(item['original'].encode() == committed, 'risk source/commit mismatch')
        self.python('risk-compare', 'source_measure.py', 'compare', '--base', self.directory / 'base-risk.json',
                    '--head', self.directory / 'head-risk.json', '--output', self.directory / 'risk.json')
        comparison = json.loads((self.directory / 'risk.json').read_text())
        (self.directory / 'risk.md').write_text('# Candidate function risk\n\n'
            f'Base: `{base}`\n\nHead: `{head}`\n\n'
            f"Identities: {len(comparison['identities'])}; failures: {len(comparison['failures'])}.\n\n"
            'Scope: six GH-94 files. Raw counters, exact rational CRAP and historical debt are retained in head-risk.json. '
            'Branch coverage is unsupported. This candidate is not an accepted baseline.\n')

    def matrix(self):
        self.python('matrix', 'critical_paths.py', '--collect', '--evidence',
                    self.directory / 'critical-path-runs/bundle.json', '--output',
                    self.directory / 'critical-paths.json')

    def collect(self):
        started = time.monotonic()
        for stage in STAGES:
            tick = time.monotonic()
            self.report['stages'][stage] = {'status': 'running'}
            self.save()
            try:
                getattr(self, stage)()
                result = {'status': 'success'}
            except (ValueError, OSError, KeyError, subprocess.SubprocessError) as error:
                result = {'status': 'failure', 'error': str(error)}
            result['seconds'] = time.monotonic() - tick
            self.report['stages'][stage] = result
            self.save()
            print(f'{stage}: {result}', flush=True)
        require(git_sha() == self.report['commit'], 'commit changed during collection')
        require_committed_sources(self.report['commit'])
        self.report['collection_seconds'] = time.monotonic() - started
        for artifact in self.directory.rglob('*'):
            relative = artifact.relative_to(self.directory)
            if relative.parts[0] not in ('build', 'snapshots', 'candidate.json') and artifact.is_file():
                self.report['artifacts'][str(relative)] = sha256(artifact)
        self.save()
        verify(self.directory / 'candidate.json', self.report['commit'], self.report['base_sha'], self.report['run_id'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=('aggregate', 'collect', 'verify'))
    parser.add_argument('--event', default=os.environ.get('EVENT_NAME'))
    parser.add_argument('--needs', default=os.environ.get('NEEDS_JSON'))
    parser.add_argument('--base-sha')
    parser.add_argument('--head-sha')
    parser.add_argument('--run-id', default=str(uuid.uuid4()))
    parser.add_argument('--output', type=Path, default=ROOT / 'target/quality/candidate')
    args = parser.parse_args()
    try:
        if args.operation == 'aggregate':
            failures = aggregate(args.event, json.loads(args.needs or '{}'))
            require(not failures, f'required jobs did not succeed: {failures}')
        else:
            for commit in (args.base_sha, args.head_sha):
                require(bool(re.fullmatch('[0-9a-f]{40}', commit or '')), 'full base/head SHAs required')
                subprocess.run(['git', 'cat-file', '-e', f'{commit}^{{commit}}'], cwd=ROOT, check=True)
            if args.operation == 'collect':
                require(git_sha() == args.head_sha, 'head must equal checked-out commit')
                require_committed_sources(args.head_sha)
                Collector(args.output, args.base_sha, args.head_sha, args.run_id).collect()
            else:
                verify(args.output / 'candidate.json', args.head_sha, args.base_sha, args.run_id)
        return 0
    except (ValueError, KeyError, TypeError, OSError, subprocess.SubprocessError) as error:
        print(f'quality CI failed: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
