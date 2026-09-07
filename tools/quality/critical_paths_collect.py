#!/usr/bin/env python3
"""Collect fresh LLVM profiles from one exact nextest identity per path."""
from __future__ import annotations

import hashlib
import json
import os
import subprocess
import tomllib
import uuid
from pathlib import Path

from critical_paths import (INVENTORY, POLICY, RULE, platform_for, require_committed_sources,
                            source_identity, validate_inventory)
from production_coverage import require
from quality_common import CRATE, ROOT, git_sha, metadata, sha256, write_json


def collect(evidence: Path) -> None:
    lock = ROOT / 'target/critical-path-collection.lock'
    lock.parent.mkdir(parents=True, exist_ok=True)
    try:
        lock.mkdir()
    except FileExistsError as error:
        raise ValueError(f'collection already active; inspect {lock}') from error
    try:
        collect_locked(evidence)
    finally:
        lock.rmdir()


def collect_locked(evidence: Path) -> None:
    inventory = tomllib.loads(INVENTORY.read_text())
    policy = json.loads(POLICY.read_text())
    validate_inventory(inventory, policy)
    meta = metadata()
    for tool in ('nextest', 'llvm-cov'):
        meta[tool] = subprocess.check_output(['cargo', tool, '--version'], text=True).strip()
    identity = {'commit': git_sha(), 'target': meta['target'], 'rule': RULE}
    require_committed_sources(identity['commit'])
    before = source_identity(CRATE)
    evidence = evidence.resolve()
    evidence.parent.mkdir(parents=True, exist_ok=True)
    # Remove only the old declaration; retain all raw evidence, including failures.
    evidence.unlink(missing_ok=True)
    bundle = {'identity': identity, 'source_root': str(CRATE), 'sources': before,
              'inventory_sha256': hashlib.sha256(json.dumps(inventory, sort_keys=True).encode()).hexdigest(),
              'tools': meta, 'runs': {}}
    environment = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / 'target/build'),
                       NEXTEST_EXPERIMENTAL_LIBTEST_JSON='1')
    # Ambient flags/foreign profiles must never redirect this collector.
    for key in ('LLVM_PROFILE_FILE', 'CARGO_BUILD_TARGET', 'CARGO_LLVM_COV_TARGET_DIR',
                'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS'):
        environment.pop(key, None)
    environment['CARGO_BUILD_TARGET'] = identity['target']
    base = ['cargo', 'llvm-cov']
    manifest = ['--manifest-path', str(CRATE / 'Cargo.toml')]
    for row in inventory['paths']:
        if platform_for(identity['target']) not in row['platforms']:
            continue
        run_id = str(uuid.uuid4())
        directory = evidence.parent / run_id
        directory.mkdir()
        test_filter = f"binary(={row['binary']}) & test(={row['test']})"
        commands = [base + ['clean', '--workspace'] + manifest,
                    base + ['nextest'] + manifest + ['--locked', '--no-report', '-E', test_filter,
                        '--message-format', 'libtest-json-plus', '--message-format-version', '0.1'],
                    base + ['report'] + manifest + ['--json', '--output-path', str(directory / 'coverage.json')]]
        run = {'identity': identity, 'run_id': run_id, 'binary': row['binary'], 'test': row['test'],
               'commands': commands, 'observable': row['observable'], 'assertions': row['assertions']}
        for index, command in enumerate(commands):
            with (directory / ('nextest.jsonl' if index == 1 else f'command-{index}.stdout')).open('w') as stdout, (directory / f'command-{index}.log').open('w') as stderr:
                code = subprocess.run(command, cwd=ROOT, env=environment, stdout=stdout, stderr=stderr).returncode
            run[['clean_exit', 'test_exit', 'coverage_exit'][index]] = code
            write_json(directory / 'run.json', run)
            require(code == 0, f"{row['id']}: command failed ({code}); see {directory}")
        for kind, name in [('nextest', 'nextest.jsonl'), ('coverage', 'coverage.json')]:
            run[kind] = {'path': f'{run_id}/{name}', 'sha256': sha256(directory / name),
                         'identity': identity, 'run_id': run_id}
        bundle['runs'][row['id']] = run
        write_json(directory / 'run.json', run)
        print(f"collected {row['id']}", flush=True)
    require(before == source_identity(CRATE) and identity['commit'] == git_sha(), 'source/commit changed during collection')
    require_committed_sources(identity['commit'])
    write_json(evidence, bundle)
