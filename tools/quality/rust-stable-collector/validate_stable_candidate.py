#!/usr/bin/env python3
"""Repository-only real acceptance driver; never distributed with the plugin.

Requires an already built candidate, installed target toolchain and LLVM tools.
No compiler bootstrap, install, baseline update or production publication.
"""
import argparse
import ast
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

from validate_registry_dependencies import validate as validate_registry
from validate_compiler_inputs import validate as validate_compiler_inputs
from validate_module_owners import validate as validate_module_owners


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def trace_command(path, command):
    """Repository observer; no fallback to lossy syscall text or untraced runs."""
    helper = os.environ.get('HARNESS_EXEC_AUDIT')
    assert helper and Path(helper).is_file(), 'set HARNESS_EXEC_AUDIT to the built repository exec observer'
    return [helper, str(path), '--', *map(str, command)]


def check_exec_events(path):
    def unique_object(pairs):
        result = {}
        for key, value in pairs:
            assert key not in result, 'duplicate execution event field'
            result[key] = value
        return result
    records = [json.loads(line, object_pairs_hook=unique_object) for line in path.read_text().splitlines()]
    assert len(records) >= 3, 'incomplete execution event stream'
    start, finish = records[0], records[-1]
    assert set(start) == {'event', 'schema', 'root_pid', 'scope'}
    assert start['event'] == 'start' and start['schema'] == 'harness-exec-events/v1'
    assert start['scope'] == 'successful-exec' and type(start['root_pid']) is int and start['root_pid'] > 0
    assert set(finish) == {'event', 'exec_count', 'root_exit_code'}
    assert finish['event'] == 'complete' and type(finish['exec_count']) is int
    assert finish['exec_count'] == len(records) - 2
    assert type(finish['root_exit_code']) is int and 0 <= finish['root_exit_code'] <= 255
    assert records[1].get('pid') == start['root_pid'], 'missing root execution'
    for record in records[1:-1]:
        assert set(record) == {'event', 'pid', 'executable', 'argv'}
        assert record['event'] == 'exec' and type(record['pid']) is int and record['pid'] > 0
        assert isinstance(record['executable'], str) and record['executable'].startswith('/')
        assert isinstance(record['argv'], list) and record['argv'] and all(isinstance(x, str) for x in record['argv'])
        for argument in [record['executable'], *record['argv']]:
            assert '\0' not in argument
            assert not re.search(r'(?:^|/)(?:python|pypy)[^/ ]*$', argument), record
            assert not argument.startswith(('-Z', '+nightly')), record
            assert not re.search(r'/nightly[-/]|rustc-dev|rustc_driver|RUSTC_BOOTSTRAP=', argument), record


def check_trace(path):
    """Audit complete execve records from our untimestamped strace invocation.

    Fail closed on abbreviated or incomplete arguments. Interleaved unfinished
    syscalls must be joined by PID before inspecting their arguments. This does
    not audit the environment behind strace's default pointer/count rendering.
    """
    if path.read_text().startswith('{'):
        return check_exec_events(path)
    quoted = r'"(?:[^"\\\n]|\\(?:[0-7]{1,3}|x[0-9a-fA-F]{2}|[abfnrtv\\"]))*"'
    strings = rf'{quoted}(?:,\s*{quoted})*'
    array = rf'\[(?:{strings})?\]'
    environment = rf'(?:0x[0-9a-fA-F]+(?: /\* \d+ vars \*/)?|NULL|{array})'
    complete = re.compile(rf'execve\({quoted},\s*{array},\s*{environment}\)\s*=\s*(?:0|-1 [A-Z0-9_]+ \([^\n]*\))')
    pending = {}
    count = 0
    for line in path.read_text().splitlines():
        match = re.fullmatch(r'(?:(?:\[pid\s+(\d+)\]|(\d+))\s+)?(.*)', line)
        pid = match[1] or match[2] or 'main'
        record = match[3]
        if record.startswith('<... execve resumed>'):
            assert pid in pending, f'unmatched execve resume: {line}'
            record = pending.pop(pid) + record.removeprefix('<... execve resumed>')
        elif record.startswith('execve('):
            assert pid not in pending, f'duplicate unfinished execve: {line}'
        elif not record or record.startswith(('--- ', '+++ ')):
            continue
        else:
            raise AssertionError(f'unrecognized execution trace record: {line}')
        if record.endswith(' <unfinished ...>'):
            pending[pid] = record.removesuffix(' <unfinished ...>')
            continue
        assert complete.fullmatch(record), f'incomplete or abbreviated execve: {record}'
        count += 1
        for literal in re.findall(quoted, record):
            argument = ast.literal_eval(literal)
            assert not re.search(r'(?:^|/)(?:python|pypy)[^/ ]*$', argument), record
            assert not argument.startswith(('-Z', '+nightly')), record
            assert not re.search(r'/nightly[-/]|rustc-dev|rustc_driver|RUSTC_BOOTSTRAP=', argument), record
    assert not pending, f'unfinished execution trace: {pending}'
    assert count, 'empty execution trace'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--trace', action='store_true', help='require HARNESS_EXEC_AUDIT observer; fail if unavailable')
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    args.output.mkdir(parents=True, exist_ok=False)
    output = args.output.resolve()
    project = output / 'plain'
    boundaries = output / 'boundaries'
    fixtures = Path(__file__).resolve().parent.parent / 'fixtures/rust-stable'
    shutil.copytree(fixtures / 'plain', project)
    shutil.copytree(fixtures / 'boundaries', boundaries)
    shutil.copytree(fixtures / 'partial', output / 'partial')
    records = []
    observations = {}

    def run(name, command, *, success=True, env=None, trace=False):
        invocation = [str(binary), *map(str, command)]
        trace_path = output / f'{name}.execve'
        if args.trace and trace:
            invocation = trace_command(trace_path, invocation)
        result = subprocess.run(invocation, capture_output=True, env=env, timeout=360)
        (output / f'{name}.stdout').write_bytes(result.stdout)
        (output / f'{name}.stderr').write_bytes(result.stderr)
        assert (result.returncode == 0) == success, (name, result.returncode, result.stderr.decode())
        if args.trace and trace:
            check_trace(trace_path)
        records.append({'name': name, 'exit_code': result.returncode, 'passed': True})
        return json.loads(result.stdout) if success else result.stderr.decode()

    def write_request(name, root, features=None):
        request = run(f'prepare-{name}', ['prepare', root, output / f'capture-{name}', output / 'doctor/doctor.json'])
        request['features'] = features or []
        path = output / f'request-{name}.json'
        path.write_text(json.dumps(request))
        return path

    def verify(name, anchor, *, success=True, request_digest=None):
        return run(name, ['verify', output / 'capture-plain', anchor['manifest_sha256'], request_digest or anchor['request_sha256']], success=success)

    try:
        tools = run('doctor', ['doctor', project, output / 'doctor'], trace=True)
        request_path = write_request('plain', project)
        # Include directory entries: file hashes alone miss an empty directory
        # left behind by a rejected request. These are workspace-owned fixtures.
        def project_snapshot():
            return {str(path.relative_to(project)): 'directory' if path.is_dir() else digest(path)
                    for path in project.rglob('*')}

        before = project_snapshot()
        alias = output / 'project-alias'
        alias.symlink_to(project, target_is_directory=True)
        isolation = []
        for entry in ('doctor', 'collect'):
            for kind, parent in (('direct', project), ('alias', alias),
                                 ('parent', output / 'doctor' / '..' / project.name)):
                destination = parent / f'rejected-{entry}-{kind}'
                name = f'output-isolation-{entry}-{kind}'
                if entry == 'doctor':
                    command = ['doctor', project, destination]
                else:
                    request = json.loads(request_path.read_text())
                    request['output_root'] = str(destination)
                    mutated = output / f'{name}.json'
                    mutated.write_text(json.dumps(request))
                    command = ['collect', mutated]
                error = json.loads(run(name, command, success=False))
                assert error['state'] == 'measurement_error' and 'outside' in error['message']
                assert not destination.exists() and project_snapshot() == before
                isolation.append({'name': name, 'destination': str(destination),
                                  'output_exists': False, 'project_unchanged': True})
        alias.unlink()
        relative = output / 'plain-doctor-relative'
        run('doctor-relative-output', ['doctor', project, os.path.relpath(relative)])
        assert (relative / 'doctor.json').is_file() and project_snapshot() == before
        observations['output_isolation'] = {'before': before, 'after': project_snapshot(),
                                            'cases': isolation, 'relative_output_created': True}
        anchor = run('plain', ['collect', request_path], trace=True)
        assert anchor['state'] == 'unsupported' and anchor['capture'] == 'complete'
        assert verify('verify', anchor)['core_acceptance'] == 'pending'
        analysis = json.loads((output / 'capture-plain/source-analysis.json').read_text())
        functions = analysis['files']['src/lib.rs']['functions']
        assert [(f['name'], f['complexity']) for f in functions] == [('classify', 3), ('never_called', 1)]
        assert analysis['files']['src/lib.rs']['exclusions'] == ['test module: tests']
        report = json.loads((output / 'capture-plain/candidate.json').read_text())
        assert report['function_crap']['state'] == 'unsupported'
        raw = json.loads((output / 'capture-plain/coverage.json').read_text())
        never_called = [f for f in raw['data'][0]['functions'] if f['name'].endswith('12never_called')]
        assert len(never_called) == 1 and never_called[0]['count'] == 0
        description = run('certified-function-owners', ['describe', output / 'capture-plain', anchor['manifest_sha256'], anchor['request_sha256']])
        mapped = [o['coverage_owner'] for o in description['owners']]
        assert [o['coverage_function'] for o in mapped] == [{'type': 'ratio', 'covered': 1, 'total': 1}, {'type': 'ratio', 'covered': 0, 'total': 1}]
        assert [o['coverage_region'] for o in mapped] == [{'type': 'ratio', 'covered': 6, 'total': 6}, {'type': 'ratio', 'covered': 0, 'total': 3}]
        observations['function_owners'] = mapped
        observations['plain'] = {'anchors': anchor, 'llvm_totals_test_inclusive': raw['data'][0]['totals'], 'lexical_functions': functions}
        partial_request = write_request('partial', output / 'partial')
        partial_anchor = run('partial', ['collect', partial_request], trace=True)
        partial_description = run('certified-region-owners', ['describe', output / 'capture-partial', partial_anchor['manifest_sha256'], partial_anchor['request_sha256']])
        partial_owners = [o['coverage_owner'] for o in partial_description['owners']]
        assert [o['coverage_function'] for o in partial_owners] == [{'type': 'ratio', 'covered': 1, 'total': 1}, {'type': 'ratio', 'covered': 0, 'total': 1}]
        assert [o['coverage_region'] for o in partial_owners] == [{'type': 'ratio', 'covered': 5, 'total': 6}, {'type': 'ratio', 'covered': 0, 'total': 3}]
        partial_raw = json.loads((output / 'capture-partial/coverage.json').read_text())
        assert partial_raw['data'][0]['totals']['regions']['count'] > 9, 'raw tests must be excluded from normalized owners'
        observations['partial'] = {'anchors': partial_anchor, 'function_owners': partial_owners, 'llvm_totals_test_inclusive': partial_raw['data'][0]['totals']}
        observations['modules'] = validate_module_owners(output, run, write_request, digest)

        assert not list((output / 'capture-plain').glob('build-*'))
        assert 'output must be a new directory' in run('stale-output', ['collect', request_path], success=False)
        verify('wrong-request-anchor', anchor, success=False, request_digest='0' * 64)
        bad_anchor = {**anchor, 'manifest_sha256': '0' * 64}
        verify('wrong-manifest-anchor', bad_anchor, success=False)

        coverage = output / 'capture-plain/coverage.json'
        original_coverage = coverage.read_bytes()
        coverage.write_bytes(b'{}')
        assert 'artifact identity/set mismatch' in verify('corrupt-coverage', anchor, success=False)
        coverage.write_bytes(original_coverage)
        # Re-anchor only the test capture to exercise semantic validation beyond
        # SHA checks. These synthetic mutations are not producer or signing proof.
        manifest_path = output / 'capture-plain/manifest.json'
        original_manifest = manifest_path.read_bytes()
        # Even newly pinned bytes must have exactly one interpretation. These
        # test-only re-anchors exercise decoding, not producer authentication.
        def duplicate_member(value, section, key, wrong, wrong_first):
            encoded = json.dumps(value)
            original = json.dumps(value[section])
            pairs = []
            for name, item in value[section].items():
                items = ([wrong, item] if wrong_first else [item, wrong]) if name == key else [item]
                pairs.extend(f'{json.dumps(name)}: {json.dumps(v)}' for v in items)
            needle = f'{json.dumps(section)}: {original}'
            assert encoded.count(needle) == 1 and key in value[section]
            return encoded.replace(needle, f'{json.dumps(section)}: ' + '{' + ', '.join(pairs) + '}', 1).encode()

        captured_request = output / 'capture-plain/request.json'
        original_request = captured_request.read_bytes()
        wrong_identity = {'sha256': '0' * 64, 'bytes': 0}
        try:
            for wrong_first in (True, False):
                order = 'first' if wrong_first else 'last'
                manifest_path.write_bytes(duplicate_member(json.loads(original_manifest), 'files', 'coverage.json', wrong_identity, wrong_first))
                assert 'duplicate key' in verify(f'duplicate-manifest-identity-{order}', {**anchor, 'manifest_sha256': digest(manifest_path)}, success=False)

                request = json.loads(original_request)
                captured_request.write_bytes(duplicate_member(request, 'source_files', 'src/lib.rs', wrong_identity, wrong_first))
                manifest = json.loads(original_manifest)
                manifest['request_sha256'] = digest(captured_request)
                manifest['files']['request.json'] = {'sha256': digest(captured_request), 'bytes': captured_request.stat().st_size}
                manifest_path.write_text(json.dumps(manifest))
                assert 'duplicate key' in verify(f'duplicate-request-identity-{order}', {**anchor, 'manifest_sha256': digest(manifest_path)}, request_digest=digest(captured_request), success=False)

                request['output_root'] = str(output / f'capture-duplicate-{order}')
                pending = output / f'request-duplicate-{order}.json'
                pending.write_bytes(duplicate_member(request, 'source_files', 'src/lib.rs', wrong_identity, wrong_first))
                assert 'duplicate key' in run(f'duplicate-collect-identity-{order}', ['collect', pending], success=False)
                assert not Path(request['output_root']).exists(), 'ambiguous input must fail before creating capture or launching tools'
        finally:
            captured_request.write_bytes(original_request)
            manifest_path.write_bytes(original_manifest)

        archives_path = output / 'duplicate-archives.json'
        for order, values in [('first', ['/missing/a', '/missing/b']), ('last', ['/missing/b', '/missing/a'])]:
            archives_path.write_text('{' + ','.join(f'{json.dumps("0" * 64)}:{json.dumps(v)}' for v in values) + '}')
            assert 'duplicate key' in run(f'duplicate-archive-identity-{order}', ['prepare', project, output / 'unused-duplicate-archives', output / 'doctor/doctor.json', '--registry-archives', archives_path], success=False)
        mutations = [
            ('negative-function-count', ['data', 0, 'functions', 0, 'count'], -1, 'nonnegative integer'),
            ('overflow-function-count', ['data', 0, 'functions', 0, 'count'], 2**63, 'exporter range'),
            ('fractional-function-count', ['data', 0, 'functions', 0, 'count'], 1.5, 'nonnegative integer'),
            ('missing-function-regions', ['data', 0, 'functions', 0, 'regions'], [], 'empty LLVM function regions'),
            ('invalid-region-file', ['data', 0, 'functions', 0, 'regions', 0, 5], 999, 'file ID out of range'),
            ('reversed-region', ['data', 0, 'functions', 0, 'regions', 0, 0], 999, 'reversed LLVM region'),
            ('unknown-region-kind', ['data', 0, 'functions', 0, 'regions', 0, 7], 99, 'unsupported LLVM region kind'),
            ('covered-exceeds-total', ['data', 0, 'totals', 'lines', 'covered'], 999, 'covered exceeds count'),
            ('wrong-summary-percent', ['data', 0, 'files', 0, 'summary', 'lines', 'percent'], 1.5, 'percentage mismatch'),
            ('wrong-notcovered', ['data', 0, 'totals', 'regions', 'notcovered'], 999, 'notcovered mismatch'),
            ('invalid-segment-boolean', ['data', 0, 'files', 0, 'segments', 0, 3], 1, 'segment fields'),
            ('mcdc-capability', ['data', 0, 'functions', 0, 'mcdc_records'], [[1]], 'unsupported LLVM MC/DC'),
        ]
        # Keep each changed row internally consistent; only cross-file totals
        # reconciliation can detect these test-only reanchored corruptions.
        for metric in ('lines', 'functions', 'instantiations', 'regions', 'branches'):
            for field in ('count', 'covered'):
                row = json.loads(original_coverage)['data'][0]['totals'][metric]
                if field == 'count':
                    row['count'] += 1
                elif row['covered']:
                    row['covered'] -= 1
                else:
                    row['count'] = max(row['count'], 1)
                    row['covered'] = 1
                row['percent'] = row['covered'] / row['count'] * 100
                if 'notcovered' in row:
                    row['notcovered'] = row['count'] - row['covered']
                mutations.append((f'total-{metric}-{field}', ['data', 0, 'totals', metric],
                                  row, f'LLVM {metric} total'))
        mutations.append(('file-summary-disagreement', ['data', 0, 'files', 0, 'summary', 'lines'],
                          {'count': 0, 'covered': 0, 'percent': 0}, 'LLVM lines total'))
        llvm_functions = json.loads(original_coverage)['data'][0]['functions']
        unused_index = next(i for i, f in enumerate(llvm_functions) if f['name'].endswith('12never_called'))
        second_owner = {**llvm_functions[0], 'name': 'different-symbol-same-owner'}
        mutations.extend([
            ('duplicate-owner-region', ['data', 0, 'functions', 0, 'regions'], llvm_functions[0]['regions'] + [llvm_functions[0]['regions'][0]], 'duplicate LLVM owner region span'),
            ('owner-region-counter-disagreement', ['data', 0, 'functions', 0, 'regions', 1, 4], 0, 'LLVM owner regions disagree'),
            ('missing-llvm-owner', ['data', 0, 'functions'], llvm_functions[1:], 'source owner missing'),
            ('duplicate-llvm-symbol', ['data', 0, 'functions'], llvm_functions + [llvm_functions[0]], 'duplicate LLVM owner symbol'),
            ('multiple-llvm-owners', ['data', 0, 'functions'], llvm_functions + [second_owner], 'multiple LLVM records'),
            ('cross-file-owner', ['data', 0, 'functions', 0, 'filenames'], llvm_functions[0]['filenames'] + ['/untrusted.rs'], 'ambiguous cross-file'),
            ('unknown-source-owner', ['data', 0, 'functions', 0, 'regions', 0, 1], 2, 'missing or ambiguous LLVM source owner'),
            ('inherited-parent-count', ['data', 0, 'functions', unused_index, 'count'], 2, 'entry count mismatch'),
            ('unexecuted-owner-regions', ['data', 0, 'functions', unused_index, 'regions', 1, 4], 1, 'unexecuted LLVM owner'),
        ])
        for field in ('count', 'covered'):
            entry = json.loads(original_coverage)['data'][0]
            for row in (entry['files'][0]['summary']['regions'], entry['totals']['regions']):
                row[field] += 1 if field == 'count' else -1
                row['percent'] = row['covered'] / row['count'] * 100
                row['notcovered'] = row['count'] - row['covered']
            mutations.append((f'owner-region-summary-{field}', ['data', 0], entry, 'LLVM owner regions disagree'))
        try:
            for label, path, replacement, error in mutations:
                changed = json.loads(original_coverage)
                parent = changed
                for component in path[:-1]:
                    parent = parent[component]
                parent[path[-1]] = replacement
                coverage.write_text(json.dumps(changed))
                manifest = json.loads(original_manifest)
                manifest['files']['coverage.json'] = {'sha256': digest(coverage), 'bytes': coverage.stat().st_size}
                manifest_path.write_text(json.dumps(manifest))
                assert error in verify(label, {**anchor, 'manifest_sha256': digest(manifest_path)}, success=False)
            coverage.write_bytes(original_coverage.replace(b'"count":', b'"count":0,"count":', 1))
            manifest = json.loads(original_manifest)
            manifest['files']['coverage.json'] = {'sha256': digest(coverage), 'bytes': coverage.stat().st_size}
            manifest_path.write_text(json.dumps(manifest))
            assert 'duplicate key' in verify('duplicate-coverage-key', {**anchor, 'manifest_sha256': digest(manifest_path)}, success=False)
        finally:
            coverage.write_bytes(original_coverage)
            manifest_path.write_bytes(original_manifest)
        analysis_path = output / 'capture-plain/source-analysis.json'
        original_analysis = analysis_path.read_bytes()
        try:
            for label, field in [('omitted-source-owners', 'functions'), ('changed-test-exclusions', 'excluded_spans')]:
                changed = json.loads(original_analysis)
                changed['files']['src/lib.rs'][field] = []
                analysis_path.write_text(json.dumps(changed))
                manifest = json.loads(original_manifest)
                manifest['files']['source-analysis.json'] = {'sha256': digest(analysis_path), 'bytes': analysis_path.stat().st_size}
                manifest_path.write_text(json.dumps(manifest))
                assert 'source analysis differs from recomputed facts' in verify(label, {**anchor, 'manifest_sha256': digest(manifest_path)}, success=False)
        finally:
            analysis_path.write_bytes(original_analysis)
            manifest_path.write_bytes(original_manifest)
        unknown = output / 'capture-plain/unlisted.json'
        unknown.write_text('{}')
        verify('mixed-artifact', anchor, success=False)
        unknown.unlink()
        source = project / 'src/lib.rs'
        original_source = source.read_bytes()
        source.write_bytes(original_source + b'\n// changed\n')
        assert 'source identity changed' in verify('changed-source', anchor, success=False)
        source.write_bytes(original_source)
        assert verify('restored-integrity', anchor)['integrity'] == 'verified'

        for label, features in [('boundaries', []), ('features', ['extra'])]:
            path = write_request(label, boundaries, features)
            result = run(label, ['collect', path], trace=True)
            assert result['state'] == 'unsupported'
            data = json.loads((output / f'capture-{label}/source-analysis.json').read_text())
            inventory = data['files']['src/lib.rs']
            rows = {f['name']: f for f in inventory['functions']}
            for name in ['deferred', 'generic', 'closure', 'feature_only']:
                assert rows[name]['state'] == 'unsupported' and rows[name]['complexity'] is None, (label, name)
            raw = json.loads((output / f'capture-{label}/coverage.json').read_text())
            owners = raw['data'][0]['functions']
            # These suffixes identify this controlled fixture's emitted symbols;
            # they are assertions, never the candidate's owner-mapping algorithm.
            constructor = [f for f in owners if f['name'].endswith('8deferred')]
            future_body = [f for f in owners if '8deferred' in f['name'] and not f['name'].endswith('8deferred')]
            assert len(constructor) == len(future_body) == 1
            assert constructor[0]['count'] == 1 and future_body[0]['count'] == 0
            assert any(f['name'].endswith('9generated') and f['count'] == 1 for f in owners)
            assert any(f['name'].endswith('8expanded') and f['count'] == 1 for f in owners)
            assert any(f['name'].endswith('12feature_only') for f in owners) == bool(features)
            observations[label] = {'anchors': result, 'llvm_totals_test_inclusive': raw['data'][0]['totals'], 'async_constructor_count': 1, 'async_body_count': 0, 'lexical_functions': inventory['functions'], 'unsupported': inventory['unsupported']}
            assert inventory['unsupported'] and 'build.rs' in data['excluded_paths']

        observations["registry"] = validate_registry(output, run, digest, True)
        observations["compiler_inputs"] = validate_compiler_inputs(output, run, digest, True)

        changed_tools = json.loads(request_path.read_text())
        changed_tools['output_root'] = str(output / 'capture-wrong-tools')
        changed_tools['tools']['llvm_cov']['identity']['sha256'] = '0' * 64
        path = output / 'request-wrong-tools.json'
        path.write_text(json.dumps(changed_tools))
        assert 'tool selection or identity changed' in run('wrong-tool-identity', ['collect', path], success=False)

        config_dir = project / '.cargo'
        config_dir.mkdir()
        config = config_dir / 'config.toml'
        config.write_text('[build]\nrustc-wrapper="python3"\n')
        assert 'unsupported Cargo configuration' in run('config-injection', ['prepare', project, output / 'capture-injection', output / 'doctor/doctor.json'], success=False)
        config.unlink()
        config_dir.rmdir()
        env = {**os.environ, 'PATH': '/nonexistent'}
        assert 'Rust required' in run('missing-rust', ['doctor', project, output / 'doctor-missing'], success=False, env=env)
        env = {**os.environ, 'RUSTFLAGS': '-Zunstable-options'}
        assert 'unsupported environment RUSTFLAGS' in run('injected-flags', ['doctor', project, output / 'doctor-flags'], success=False, env=env)
        pin = project / 'rust-toolchain.toml'
        pin.write_text('[toolchain]\nchannel="stable-candidate-intentionally-uninstalled"\n')
        env = {k: v for k, v in os.environ.items() if k != 'RUSTUP_TOOLCHAIN'}
        assert 'Rust required' in run('missing-project-toolchain', ['doctor', project, output / 'doctor-pin'], success=False, env=env)
        pin.unlink()
        (project / 'alias.rs').symlink_to('src/lib.rs')
        run('symlink-source', ['prepare', project, output / 'capture-symlink', output / 'doctor/doctor.json'], success=False)
        (project / 'alias.rs').unlink()

        source.write_text(original_source.decode() + '\n#[test] fn must_fail() { panic!("fixture intentional failure"); }\n')
        failing = write_request('failed-test', project)
        assert 'tool exited' in run('failed-test', ['collect', failing], success=False)
        assert not (output / 'capture-failed-test/manifest.json').exists()
        source.write_bytes(original_source)
        summary = {
            'schema': 'rust-stable-candidate-acceptance/v1', 'state': 'candidate-only',
            'checks': records, 'observations': observations, 'tools': tools, 'binary': {'sha256': digest(binary), 'bytes': binary.stat().st_size},
            'capture_bytes': sum(p.stat().st_size for p in (output / 'capture-plain').rglob('*') if p.is_file()),
            'persistent_candidate_cache_bytes': 0,
            'execve_trace': 'verified' if args.trace else 'not performed',
            'core_protocol_authentication': 'not exercised by this capture suite; see separate Core acceptance',
            'release_signature_lifecycle': 'not exercised by this capture suite; see separate lifecycle acceptance',
            'second_toolchain_and_system': 'not established by this invocation',
        }
        (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
        print(json.dumps({'checks_passed': len(records), 'summary': str(output / 'summary.json')}))
    except BaseException:
        (output / 'partial-checks.json').write_text(json.dumps(records, indent=2) + '\n')
        raise


if __name__ == '__main__':
    main()
