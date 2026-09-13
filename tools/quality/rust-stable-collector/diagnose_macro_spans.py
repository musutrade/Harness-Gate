#!/usr/bin/env python3
"""Repository-only span diagnostic; not a collector or certified coverage input.

Copies the pinned fixture and adds logging around its unchanged generated tokens.
Uses only stable proc_macro span methods; never builds compiler-private code.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess


WRAPPER = '''use proc_macro::{TokenStream, TokenTree};

fn dump(owner: &str, path: &str, stream: TokenStream) {
    for (index, token) in stream.into_iter().enumerate() {
        let path = format!("{path}.{index}");
        let span = token.span();
        let end = span.end();
        let kind = match &token {
            TokenTree::Group(group) => format!("{:?}", group.delimiter()),
            TokenTree::Ident(_) => "ident".into(),
            TokenTree::Punct(_) => "punct".into(),
            TokenTree::Literal(_) => "literal".into(),
        };
        eprintln!("GH259-SPAN\\t{owner}\\t{path}\\t{kind}\\t{}\\t{}\\t{}\\t{}",
            span.line(), span.column(), end.line(), end.column());
        if let TokenTree::Group(group) = token {
            dump(owner, &path, group.stream());
        }
    }
}

#[proc_macro]
pub fn observed_function(input: TokenStream) -> TokenStream {
    match gate_observation_generator::expand(input.into()) {
        Ok(observation) => {
            let generated: TokenStream = observation.generated.into();
            let before = generated.to_string();
            dump(&observation.owner, "output", generated.clone());
            assert_eq!(before, generated.to_string());
            eprintln!("GH259-TOKENS\\t{}\\t{}", observation.owner, before);
            generated
        }
        Err(error) => error.into_compile_error().into(),
    }
}
'''


def identity(path):
    data = path.read_bytes()
    return {'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--toolchain', default='1.97.1')
    parser.add_argument('--llvm-cov', required=True, type=Path)
    parser.add_argument('--llvm-profdata', required=True, type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    output = args.output.resolve()
    fixture = Path(__file__).resolve().parent.parent / 'fixtures/rust-macro-observation'
    project = output / 'project'
    shutil.copytree(fixture, project)
    source = {str(p.relative_to(project)): identity(p) for p in sorted(project.rglob('*')) if p.is_file()}
    env = dict(os.environ, CARGO_TARGET_DIR=str(output / 'build'))
    cleared = ('RUSTUP_TOOLCHAIN', 'RUSTC_BOOTSTRAP', 'RUSTFLAGS', 'RUSTDOCFLAGS',
               'CARGO_ENCODED_RUSTFLAGS', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER')
    for key in cleared:
        env.pop(key, None)
    # Explicit external tools avoid cargo-llvm-cov offering rustup installation.
    llvm_cov = args.llvm_cov.resolve(strict=True)
    llvm_profdata = args.llvm_profdata.resolve(strict=True)
    env.update(LLVM_COV=str(llvm_cov), LLVM_PROFDATA=str(llvm_profdata))
    commands = []

    def run(name, argv):
        result = subprocess.run(list(map(str, argv)), cwd=project, env=env,
                                stdin=subprocess.DEVNULL, capture_output=True, timeout=300)
        stdout, stderr = output / (name + '.stdout'), output / (name + '.stderr')
        stdout.write_bytes(result.stdout)
        stderr.write_bytes(result.stderr)
        commands.append({'name': name, 'argv': list(map(str, argv)), 'exit_code': result.returncode,
                         'stdout': identity(stdout), 'stderr': identity(stderr)})
        assert result.returncode == 0, (name, result.returncode, result.stderr[-2000:])
        return result

    rustc = run('rustc', ['rustc', '+' + args.toolchain, '-vV']).stdout.decode()
    assert 'release: ' + args.toolchain + '\n' in rustc and 'nightly' not in rustc
    llvm_version = re.search(r'LLVM version: (\d+\.\d+\.\d+)', rustc).group(1)
    for name, path in (('llvm-cov', llvm_cov), ('llvm-profdata', llvm_profdata)):
        version = run(name, [path, '--version']).stdout.decode()
        assert re.search(r'LLVM version (\d+\.\d+\.\d+)', version).group(1) == llvm_version
    cargo_cov = run('cargo-llvm-cov', ['cargo', '+' + args.toolchain, 'llvm-cov', '--version']).stdout.decode()
    exports, observations = {}, {}
    wrapper = project / 'macros/src/lib.rs'
    for mode in ('original', 'diagnostic'):
        if mode == 'diagnostic':
            wrapper.write_text(WRAPPER)
        for configuration in ('default', 'branching'):
            name = mode + '-' + configuration
            coverage = output / (name + '.json')
            flags = ['--features', 'branching'] if configuration == 'branching' else []
            result = run(name, ['cargo', '+' + args.toolchain, 'llvm-cov', '--manifest-path',
                project / 'Cargo.toml', '--locked', '--offline', '--package', 'gate-observation-consumer',
                '--json', '--target', 'x86_64-unknown-linux-gnu', '--output-path', coverage, *flags])
            raw = json.loads(coverage.read_bytes())
            assert raw['version'] == '3.1.0'
            consumer = str(project / 'consumer/src/lib.rs')
            entries = [f for unit in raw['data'] for f in unit['functions'] if consumer in f['filenames']]
            assert len(entries) == 2, 'export changed; review the generated owner gap'
            for function in entries:
                assert function['count'] == 1
                file_id = function['filenames'].index(consumer)
                regions = [r for r in function['regions'] if r[5] == file_id and r[7] == 0]
                assert regions and all(r[0] >= 12 for r in regions), 'generated business owner appeared'
            exports[name] = {'identity': identity(coverage), 'consumer_functions': entries}
            if mode != 'diagnostic':
                continue
            owners, tokens = {}, {}
            for line in result.stderr.decode().splitlines():
                fields = line.split('\t')
                if fields[0] == 'GH259-SPAN':
                    _, owner, path, kind, *span = fields
                    item = {'path': path, 'kind': kind, 'span': list(map(int, span))}
                    old = owners.setdefault(owner, {}).setdefault(path, item)
                    assert old == item, 'repeated expansion changed spans'
                elif fields[0] == 'GH259-TOKENS':
                    _, owner, text = fields
                    assert tokens.setdefault(owner, text) == text
            assert set(owners) == set(tokens) == {'plain', 'branch', 'unexecuted', 'configured'}
            body_ranges = {}
            for owner, spans in owners.items():
                body = spans['output.7']
                assert body['kind'] == 'Brace'
                children = [v for k, v in spans.items() if k.startswith('output.7.')]
                assert children and all(c['span'] == body['span'] for c in children)
                body_ranges[owner] = {'body': body['span'], 'interior_tokens': len(children),
                                      'all_interior_ranges_equal_body': True}
            observations[configuration] = {'owners': owners, 'generated_tokens': tokens,
                                           'body_ranges': body_ranges}
    for configuration in ('default', 'branching'):
        assert exports['original-' + configuration]['consumer_functions'] == exports[
            'diagnostic-' + configuration]['consumer_functions'], 'logging changed coverage export'
    for owner in ('plain', 'branch', 'unexecuted'):
        assert observations['default']['generated_tokens'][owner] == observations[
            'branching']['generated_tokens'][owner]
    assert observations['default']['generated_tokens']['configured'] != observations[
        'branching']['generated_tokens']['configured']
    # Only the diagnostic wrapper changed; the shared generator and consumer did not.
    after = {str(p.relative_to(project)): identity(p) for p in sorted(project.rglob('*')) if p.is_file()}
    assert set(after) == set(source)
    assert [p for p in source if source[p] != after[p]] == ['macros/src/lib.rs']
    summary = {'schema': 'rust-macro-span-diagnostic/v1', 'rustc': rustc, 'cargo_llvm_cov': cargo_cov,
               'external_tools': {str(p): identity(p) for p in (llvm_cov, llvm_profdata)},
               'source': source, 'diagnostic_wrapper': identity(wrapper), 'commands': commands,
               'cleared_environment': cleared, 'exports': exports, 'observations': observations,
               'compiler_internal_span_context': 'not observable via this stable diagnostic',
               'coverage_and_crap': 'unsupported; missing business function records',
               'process_trace': 'not run', 'T4': 'incomplete'}
    (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps({'commands': len(commands), 'configurations': 2, 'T4': 'incomplete'}))


if __name__ == '__main__':
    main()
