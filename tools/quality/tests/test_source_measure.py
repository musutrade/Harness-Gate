from __future__ import annotations

import copy
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tools/quality"))
from source_measure import HOTSPOTS, SERIES, SOURCE_FILES, prepare, ast, byte_span, closure_name, compare, compiler_configuration, complexity, digest, instrument, measure, original_point, provenance


class SourceMeasureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        target = ROOT / "target/gh-94-measure"
        subprocess.run(["cargo", "build", "--locked", "--manifest-path",
                        str(ROOT / "tools/quality/rust-measure/Cargo.toml")],
                       env={**os.environ, "CARGO_TARGET_DIR": str(target)}, check=True, capture_output=True)
        cls.binary = target / "debug" / ("harness-gate-rust-measure" + (".exe" if os.name == "nt" else ""))

    def inventory(self, source, target=None):
        with tempfile.TemporaryDirectory() as temp:
            file = Path(temp) / "input.rs"
            file.write_bytes(source.encode())
            return ast(file, self.binary, compiler_configuration(target))

    def test_compiler_target_filters_modules_methods_fields_and_body_blocks(self):
        source = '''
struct State { #[cfg(windows)] handle: usize, #[cfg(unix)] fd: usize }
#[cfg(unix)] mod platform { fn open() { if true {} } }
#[cfg(windows)] mod platform { fn open() { if true {} if true {} } }
impl State { #[cfg(unix)] fn unix() {} #[cfg(windows)] fn windows() {} }
trait Run { #[cfg(unix)] fn unix() {} #[cfg(windows)] fn windows() {} }
#[cfg(not(test))]
fn both() {
    #[cfg(unix)] { if true {} }
    #[cfg(windows)] { if true {} if true {} let hidden = || if true {}; }
    #[cfg(test)] { unsupported!(a => b); }
}
#[cfg(test)] mod tests { #[test] fn ignored() { unsupported!(a => b); } }
#[cfg(all(test, unix))] mod external_tests;
#[test] fn ignored_function() { unsupported!(a => b); }
'''
        unix = self.inventory(source, 'x86_64-unknown-linux-gnu')
        windows = self.inventory(source, 'x86_64-pc-windows-msvc')
        self.assertIn('unix', unix['configuration']['cfg'])
        self.assertIn('windows', windows['configuration']['cfg'])
        self.assertEqual([s['name'] for s in unix['symbols']], ['platform::open', 'State::unix', 'unix', 'both'])
        self.assertEqual([complexity(s['raw']) for s in unix['symbols']], [2, 1, 1, 2])
        self.assertEqual([s['name'] for s in windows['symbols']], ['platform::open', 'State::windows', 'windows', 'both', 'both::closure_10_58'])
        self.assertEqual([complexity(s['raw']) for s in windows['symbols']], [3, 1, 1, 3, 2])
        for result in (unix, windows):
            self.assertTrue(result['excluded'])
            self.assertFalse(any(s['test'] for s in result['symbols']))

    def test_unsupported_configuration_fails_without_partial_inventory(self):
        inactive = "unix" if "windows" in compiler_configuration()["cfg"] else "windows"
        for source in (
            '#[cfg(feature="extra")] fn f() {}',
            '#[cfg(any(unix, windows))] fn f() {}',
            '#[cfg(target_os="linux")] fn f() {}',
            '#[cfg(not(not(test)))] fn f() {}',
            '#[cfg(all(test, feature="extra"))] mod unsupported;',
            '#[cfg_attr(unix, inline)] fn f() {}',
            '#[cfg(windows)] #[cfg(feature="extra")] fn f() {}',
            'fn f() { let x = #[cfg(' + inactive + ')] { 1 }; }',
            'fn f() { println!("{}", #[cfg(unix)] { 1 }); }',
        ):
            with self.subTest(source=source), tempfile.TemporaryDirectory() as temp:
                file = Path(temp) / 'input.rs'; file.write_bytes(source.encode())
                result = subprocess.run([str(self.binary), str(file), '--target-cfg'],
                                        input=json.dumps(compiler_configuration()), text=True, capture_output=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('unsupported', result.stderr)
                self.assertEqual(result.stdout, '')

    def test_replay_and_project_sources_are_measured_for_the_host_target(self):
        for path in ('process/replay.rs', 'project/discovery.rs', 'project/mod.rs'):
            with self.subTest(path=path):
                self.assertIn(path, SOURCE_FILES)
                source = ROOT / 'tools/harness-gate/src' / path
                result = ast(source, self.binary)
                self.assertTrue(result['symbols'])
                self.assertFalse(any(s['test'] for s in result['symbols']))
                if path == 'process/replay.rs':
                    names = {s['name'] for s in result['symbols']}
                    host_is_windows = 'windows' in compiler_configuration()['cfg']
                    self.assertEqual('platform::open_at' in names, not host_is_windows)
                    self.assertEqual('platform::open_directory' in names, host_is_windows)
                    windows = ast(source, self.binary, compiler_configuration('x86_64-pc-windows-msvc'))
                    windows_names = {s['name'] for s in windows['symbols']}
                    self.assertIn('platform::open_directory', windows_names)
                    self.assertNotIn('platform::open_at', windows_names)

    def test_compare_rejects_mixed_target_configurations(self):
        base = {'series': SERIES, 'tools': {}, 'configuration': compiler_configuration(), 'functions': []}
        other = 'x86_64-unknown-linux-gnu' if 'windows' in base['configuration']['cfg'] else 'x86_64-pc-windows-msvc'
        head = dict(base, configuration=compiler_configuration(other))
        with self.assertRaisesRegex(ValueError, 'incompatible base/head target configurations'):
            compare(base, head)
        del base['configuration']; del head['configuration']
        with self.assertRaisesRegex(ValueError, 'incompatible base/head target configurations'):
            compare(base, head)

    def test_native_inventory_has_separate_identity_and_contracts(self):
        subprocess.run(["cargo", "test", "--locked", "--manifest-path",
                        str(ROOT / "tools/quality/rust-measure/Cargo.toml")],
                       env={**os.environ, "CARGO_TARGET_DIR": str(ROOT / "target/gh-94-measure")},
                       check=True, capture_output=True)
        with tempfile.TemporaryDirectory() as temp:
            file = Path(temp) / "input.rs"
            file.write_text('fn f() { tracing::error!(value = %value); }')
            run = subprocess.run([str(self.binary), "--native-inventory", str(file)],
                                 text=True, capture_output=True, check=True)
            result = json.loads(run.stdout)
            self.assertEqual(result["analyzer"], "harness-gate-rust-native-inventory")
            self.assertFalse(result["certified_llvm_mapping"])
            with self.assertRaises(subprocess.CalledProcessError):
                ast(file, self.binary)

    def test_quality_configuration_source_certification(self):
        crate = ROOT / 'tools/harness-gate'
        for name in ('mod', 'model', 'policy', 'validation', 'compiler', 'collectors', 'baseline', 'baseline/git'):
            path = f'config/quality/{name}.rs'
            with self.subTest(path=path):
                self.assertIn(path, SOURCE_FILES)
                symbols = ast(crate / 'src' / path, self.binary)['symbols']
                production = [s for s in symbols if not s['test']]
                if name == 'model':
                    self.assertEqual(production, [])
                else:
                    self.assertTrue(production)
                    source = (crate / 'src' / path).read_text(encoding="utf-8")
                    transformed, _ = instrument(source, {'symbols': symbols})
                    self.assertEqual(len(self.inventory(transformed)['symbols']), len(symbols))
        module = (crate / 'src/config/quality/mod.rs').read_text(encoding="utf-8")
        self.assertIn('#[cfg(test)]\nmod tests;', module)

    def test_preset_source_certification(self):
        crate = ROOT / 'tools/harness-gate'
        inventory = json.loads((ROOT / 'tools/quality/production-source.json').read_text(encoding="utf-8"))
        declared = {path.removeprefix('src/') for path in inventory['boundaries']['preset']['files']}
        self.assertEqual(declared, {path for path in SOURCE_FILES if path.startswith('preset/')})
        for name in ('catalog', 'composition', 'filesystem', 'import', 'initialize', 'migration', 'mod'):
            path = f'preset/{name}.rs'
            with self.subTest(path=path):
                self.assertIn(path, SOURCE_FILES)
                source = crate / 'src' / path
                symbols = ast(source, self.binary)['symbols']
                if name != 'mod':
                    self.assertTrue(any(not symbol['test'] for symbol in symbols))
                transformed, _ = instrument(source.read_text(encoding="utf-8"), {'symbols': symbols})
                self.assertEqual(len(self.inventory(transformed)['symbols']), len(symbols))
        self.assertIn('#[cfg(test)]\nmod tests;', (crate / 'src/preset/mod.rs').read_text(encoding="utf-8"))

    def test_resource_validation_source_certification(self):
        path = 'config/validation/mod.rs'
        self.assertIn(path, SOURCE_FILES)
        symbols = ast(ROOT / 'tools/harness-gate/src' / path, self.binary)['symbols']
        names = {symbol['name'] for symbol in symbols if not symbol['test']}
        self.assertTrue({'validate_resource_conflicts', 'validate_shared_services',
                         'validate_log_conflicts', 'validate_service_injections'} <= names)
        for module in ('config', 'verify'):
            source = (ROOT / f'tools/harness-gate/src/{module}/mod.rs').read_text(encoding="utf-8")
            self.assertIn('#[cfg(test)]\nmod tests;', source)

    def test_verify_reporting_source_certification(self):
        crate = ROOT / 'tools/harness-gate'
        for path in ('verify/mod.rs', 'verify/quality.rs', 'verify/report.rs', 'failure.rs', 'config/import.rs'):
            with self.subTest(path=path):
                self.assertIn(path, SOURCE_FILES)
                source = crate / 'src' / path
                symbols = ast(source, self.binary)['symbols']
                self.assertTrue(any(not symbol['test'] for symbol in symbols))
                transformed, _ = instrument(source.read_text(encoding="utf-8"), {'symbols': symbols})
                self.assertEqual(len(self.inventory(transformed)['symbols']), len(symbols))

    def test_actual_redaction_source_inventory_and_instrumentation(self):
        self.assertEqual(SERIES, {
            'analyzer': 'harness-gate-rust-measure/0.3.1', 'rule': 'mccabe-rust-3/1',
            'instrumentation': 'closure-black-box/1', 'mapping': 'insertions-utf8/1',
            'selection': 'gh287-text-redaction/1', 'configuration': 'compiler-target-production/1',
        })
        path = 'utils/redaction.rs'
        self.assertIn(path, SOURCE_FILES)
        source_file = ROOT / 'tools/harness-gate/src' / path
        source = source_file.read_bytes().decode('utf-8')
        inventory = ast(source_file, self.binary)
        self.assertEqual(inventory, self.inventory(source))
        symbols = inventory['symbols']
        self.assertEqual([s['kind'] for s in symbols], ['function', 'closure', 'closure'])
        self.assertEqual(symbols[0]['name'], 'redact_text')
        self.assertTrue(all(s['name'].startswith('redact_text::closure_') for s in symbols[1:]))
        self.assertFalse(any(s['test'] for s in symbols))
        self.assertEqual([complexity(s['raw']) for s in symbols], [1, 1, 1])
        self.assertTrue(inventory['excluded'], 'test module must leave production ranges')
        transformed, edits = instrument(source, inventory)
        self.assertEqual(len(edits), 4)
        reparsed = self.inventory(transformed)
        self.assertEqual([s['kind'] for s in reparsed['symbols']], [s['kind'] for s in symbols])
        for original, inserted in zip(symbols, reparsed['symbols']):
            self.assertEqual(complexity(original['raw']), complexity(inserted['raw']))
            for field in ('span', 'body'):
                mapped = byte_span(transformed, inserted[field])
                self.assertEqual((*original_point(mapped[:2], edits),
                                  *original_point(mapped[2:], edits)),
                                 byte_span(source, original[field]))

    def test_once_lock_regex_tuple_fold_has_independent_native_owners(self):
        source = '''use regex::Regex;
use std::sync::OnceLock;
pub fn redact(input: &str, patterns: &OnceLock<Vec<(Regex, &'static str)>>, count: usize) -> String {
    let patterns = patterns.get_or_init(|| {
        vec![
            (Regex::new("SYNTHETIC_A").expect("first regex"), "public-a"),
            (Regex::new("SYNTHETIC_B").expect("second regex"), "public-b"),
        ].into_iter().take(count).collect()
    });
    patterns.iter().fold(input.to_string(), |text, (pattern, replacement)| {
        pattern.replace_all(&text, *replacement).into_owned()
    })
}
#[cfg(test)] mod tests { #[test] fn excluded() { unsupported!(a => b); } }
'''
        main = '''mod redaction;
use regex::Regex;
use std::sync::OnceLock;
fn main() {
    let mode = std::env::args().nth(1).expect("mode");
    let count = if mode.ends_with("zero") { 0 } else { 2 };
    let patterns = OnceLock::new();
    if mode.starts_with("preset") {
        patterns.set(vec![
            (Regex::new("SYNTHETIC_A").unwrap(), "public-a"),
            (Regex::new("SYNTHETIC_B").unwrap(), "public-b"),
        ].into_iter().take(count).collect()).unwrap();
    }
    let result = redaction::redact("public-context SYNTHETIC_A SYNTHETIC_B", &patterns, count);
    assert_eq!(result, if count == 0 { "public-context SYNTHETIC_A SYNTHETIC_B" }
                       else { "public-context public-a public-b" });
}
'''
        # This fixture builds its own pinned regex dependency graph. Never join
        # against an unbound rlib left by another build in a shared target dir.
        repo_lock = (ROOT / 'tools/harness-gate/Cargo.lock').read_bytes()
        packages = tomllib.loads(repo_lock.decode())['package']
        by_name = {}
        for package in packages:
            by_name.setdefault(package['name'], []).append(package)
        selected = {}
        pending = ['regex']
        while pending:
            name = pending.pop()
            if name in selected:
                continue
            self.assertEqual(len(by_name[name]), 1, f'ambiguous fixture dependency: {name}')
            selected[name] = by_name[name][0]
            pending.extend(dep.split()[0] for dep in selected[name].get('dependencies', []))
        regex_version = selected['regex']['version']
        cargo_manifest = ('[package]\nname = "redaction-native-fixture"\nversion = "0.0.0"\n'
                          'edition = "2021"\n[workspace]\n[dependencies]\n'
                          f'regex = "={regex_version}"\n')
        lock = ('version = 4\n\n[[package]]\nname = "redaction-native-fixture"\n'
                'version = "0.0.0"\ndependencies = ["regex"]\n\n')
        for block in repo_lock.decode().split('[[package]]\n')[1:]:
            package = tomllib.loads('[[package]]\n' + block)['package'][0]
            if package['name'] in selected:
                self.assertEqual(package, selected[package['name']])
                lock += '[[package]]\n' + block
        lock_bytes = lock.encode()
        retained_root = Path(os.environ.get('RUST_MEASURE_NATIVE_EVIDENCE',
                                            ROOT / 'target/gh287-native-evidence'))
        retained_root.mkdir(parents=True, exist_ok=True)
        evidence = Path(tempfile.mkdtemp(prefix='once-lock-fold-', dir=retained_root))
        commands = []

        def run(command, **kwargs):
            record = {'argv': [str(arg) for arg in command], 'cwd': str(kwargs.get('cwd', ROOT)),
                      'environment': kwargs.pop('record_environment', {})}
            if 'input' in kwargs:
                record['stdin_sha256'] = digest(kwargs['input'])
            commands.append(record)
            number = len(commands)
            try:
                result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, **kwargs)
            except OSError as error:
                record['error'] = str(error)
                (evidence / 'commands.json').write_text(json.dumps(commands, indent=2) + '\n')
                raise
            record['returncode'] = result.returncode
            (evidence / f'command-{number}.stdout').write_bytes(result.stdout)
            (evidence / f'command-{number}.stderr').write_bytes(result.stderr)
            (evidence / 'commands.json').write_text(json.dumps(commands, indent=2) + '\n')
            self.assertEqual(result.returncode, 0, f'{command}: {result.stderr.decode(errors="replace")}')
            return result.stdout

        with tempfile.TemporaryDirectory(dir=ROOT / 'target') as temp:
            crate = Path(temp)
            (crate / 'src').mkdir()
            file = crate / 'src/redaction.rs'
            file.write_bytes(source.encode())
            configuration = compiler_configuration()
            inventory = json.loads(run([str(self.binary), str(file), '--target-cfg'],
                                       input=json.dumps(configuration).encode()))
            self.assertEqual(inventory['configuration'], configuration)
            self.assertEqual([s['kind'] for s in inventory['symbols']], ['function', 'closure', 'closure'])
            self.assertEqual([complexity(s['raw']) for s in inventory['symbols']], [1, 1, 1])
            transformed, edits = instrument(source, inventory)
            file.write_bytes(transformed.encode())
            (crate / 'src/main.rs').write_bytes(main.encode())
            (crate / 'Cargo.toml').write_bytes(cargo_manifest.encode())
            (crate / 'Cargo.lock').write_bytes(lock_bytes)
            manifest = {'series': SERIES, 'configuration': configuration, 'files': {'redaction.rs': {
                'original': source, 'original_sha256': digest(source.encode()),
                'instrumented_sha256': digest(transformed.encode()), 'inventory': inventory, 'edits': edits}}}
            for name, data in [('fixture.original.rs', source.encode()), ('fixture.instrumented.rs', transformed.encode()),
                               ('main.rs', main.encode()), ('Cargo.toml', cargo_manifest.encode()), ('Cargo.lock', lock_bytes),
                               ('repository.Cargo.lock', repo_lock)]:
                (evidence / name).write_bytes(data)
            (evidence / 'instrumentation-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
            environment = {**os.environ, 'CARGO_TARGET_DIR': str(crate / 'build'),
                           'RUSTFLAGS': '-C instrument-coverage -C opt-level=0'}
            environment.pop('CARGO_ENCODED_RUSTFLAGS', None)
            environment.pop('LLVM_PROFILE_FILE', None)
            run(['cargo', 'fetch', '--locked', '--manifest-path', str(crate / 'Cargo.toml'),
                 '--target', manifest['configuration']['target']], cwd=crate, env=environment)
            self.assertEqual((crate / 'Cargo.lock').read_bytes(), lock_bytes)
            run(['cargo', 'build', '--locked', '--offline', '--manifest-path', str(crate / 'Cargo.toml'),
                 '--target', manifest['configuration']['target']], cwd=crate, env=environment,
                record_environment={key: environment[key] for key in ('CARGO_TARGET_DIR', 'RUSTFLAGS')})
            self.assertEqual((crate / 'Cargo.lock').read_bytes(), lock_bytes)
            suffix = '.exe' if os.name == 'nt' else ''
            executable = crate / 'build' / manifest['configuration']['target'] / 'debug' / ('redaction-native-fixture' + suffix)
            shutil.copyfile(executable, evidence / executable.name)
            sysroot = run(['rustc', '--print', 'sysroot']).decode().strip()
            version = run(['rustc', '-vV']).decode().strip()
            host = next(line.removeprefix('host: ') for line in version.splitlines() if line.startswith('host: '))
            llvm_bin = Path(sysroot) / 'lib/rustlib' / host / 'bin'
            llvm_tools = {name: llvm_bin / (name + suffix) for name in ('llvm-profdata', 'llvm-cov')}
            tool_versions = {name: run([str(path), '--version']).decode().strip() for name, path in llvm_tools.items()}
            results, rejected = {}, []
            for mode in ('fresh-zero', 'fresh-multi', 'preset-zero', 'preset-multi'):
                raw, profile = evidence / f'{mode}.profraw', evidence / f'{mode}.profdata'
                run([str(executable), mode], env={**environment, 'LLVM_PROFILE_FILE': str(raw)},
                    record_environment={'LLVM_PROFILE_FILE': str(raw)})
                run([str(llvm_tools['llvm-profdata']), 'merge', '-sparse', str(raw), '-o', str(profile)])
                exported = run([str(llvm_tools['llvm-cov']), 'export', str(executable), f'-instr-profile={profile}'])
                (evidence / f'{mode}.llvm.json').write_bytes(exported)
                llvm = json.loads(exported)
                rows = measure(manifest, llvm, crate, self.binary)
                self.assertEqual(len(rows), 3)
                parent, initializer, fold = rows
                self.assertEqual([r['kind'] for r in rows], ['function', 'closure', 'closure'])
                expected = [1, int(mode.startswith('fresh')), 0 if mode.endswith('zero') else 2]
                for row, count in zip(rows, expected):
                    self.assertEqual(len(row['instances']), 1)
                    self.assertEqual(row['instances'][0]['count'], count, (mode, row['name']))
                    self.assertGreater(row['lines']['count'], 0)
                    self.assertGreater(row['regions']['count'], 0)
                    self.assertEqual(row['cc'], 1)
                    if count == 0:
                        self.assertEqual(row['lines']['covered'], 0)
                        self.assertEqual(row['regions']['covered'], 0)
                    else:
                        self.assertEqual(row['lines']['covered'], row['lines']['count'])
                        self.assertEqual(row['regions']['covered'], row['regions']['count'])
                self.assertEqual(len({r['instances'][0]['index'] for r in rows}), 3)
                results[mode] = rows
                for owner, row in zip(('parent', 'initializer', 'fold'), rows):
                    missing = copy.deepcopy(llvm)
                    missing['data'][0]['functions'].pop(row['instances'][0]['index'])
                    with self.assertRaisesRegex(ValueError, re.escape('missing LLVM function: redaction.rs:' + row['name'] + ':')):
                        measure(manifest, missing, crate, self.binary)
                    rejected.append(f'{mode}: missing {owner} (count={row["instances"][0]["count"]})')
                for owner, row in (('initializer', initializer), ('fold', fold)):
                    borrowed = copy.deepcopy(llvm)
                    function = borrowed['data'][0]['functions'][row['instances'][0]['index']]
                    native_parent = llvm['data'][0]['functions'][parent['instances'][0]['index']]
                    function['regions'] = copy.deepcopy(native_parent['regions'])
                    function['filenames'] = copy.deepcopy(native_parent['filenames'])
                    with self.assertRaisesRegex(ValueError, 'function kind mismatch: redaction.rs:redact'):
                        measure(manifest, borrowed, crate, self.binary)
                    rejected.append(f'{mode}: {owner} cannot borrow parent mapping')
                (evidence / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
                (evidence / 'negative-checks.json').write_text(json.dumps(rejected, indent=2) + '\n')
            report = {'series': SERIES, 'configuration': manifest['configuration'], 'tools': provenance(self.binary),
                      'checkout_sha': run(['git', 'rev-parse', 'HEAD'], cwd=ROOT).decode().strip(),
                      'fixture_binary_sha256': digest(executable.read_bytes()),
                      'test_source_sha256': digest(Path(__file__).read_bytes()),
                      'repository_lock_sha256': digest(repo_lock), 'fixture_lock_sha256': digest(lock_bytes),
                      'locked_packages': selected,
                      'llvm_tools': {name: {'sha256': digest(path.read_bytes()), 'version': tool_versions[name]}
                                     for name, path in llvm_tools.items()},
                      'results': results, 'validated': rejected}
            (evidence / 'native-result.json').write_text(json.dumps(report, indent=2) + '\n')
            hashes = {path.name: digest(path.read_bytes()) for path in sorted(evidence.iterdir())}
            (evidence / 'sha256.json').write_text(json.dumps(hashes, indent=2) + '\n')

    def test_ast_controls_and_nested_ownership(self):
        source = '''const N: usize = 2;
fn outer(xs: &[bool]) -> bool {
    fn nested(x: bool) -> bool { if x { true } else { false } }
    let Some(x) = xs.first() else { return false };
    println!("{}", xs.iter().all(|x| *x && nested(*x)));
    match x { true if xs.len() > N => true, _ => false }
}
#[cfg(test)] mod tests { #[test] fn only_test() {} }
'''
        symbols = self.inventory(source)["symbols"]
        self.assertEqual([s["name"] for s in symbols], ["outer", "outer::nested", "outer::closure_5_34"])
        self.assertEqual([complexity(s["raw"]) for s in symbols], [4, 2, 2])
        self.assertEqual([s["test"] for s in symbols], [False, False, False])
        self.assertEqual(symbols[0]["raw"]["nested_functions"], 1)
        self.assertNotIn("and_and", symbols[0]["raw"])
        controls = self.inventory('''fn controls(mut xs: impl Iterator<Item=bool>) -> Option<()> {
    if xs.next()? && (true || false) {}
    if let Some(x) = xs.next() { let _ = x; }
    let Some(_) = xs.next() else { return None; };
    while xs.next().is_some() { break; }
    while let Some(_) = xs.next() { break; }
    for _ in 0..2 { loop { break; } }
    match xs.next() { Some(true) if true => (), Some(false) => (), _ => () }
    Some(())
}''')["symbols"][0]
        self.assertEqual(controls["raw"], {
            "if": 3, "question_mark": 1, "and_and": 1, "or_or": 1,
            "while": 2, "for": 1, "loop": 1, "match": 1,
            "match_arms": 3, "match_decisions": 2, "guards": 1,
        })
        self.assertEqual(complexity(controls["raw"]), 14)

    def test_vec_repeat_visits_value_length_and_closure_decisions(self):
        symbols = self.inventory('fn f(x: bool) { let _ = vec![|| if x { 1 } else { 2 }; if x { 2 } else { 1 }]; }')["symbols"]
        self.assertEqual([s["kind"] for s in symbols], ["function", "closure"])
        self.assertEqual([complexity(s["raw"]) for s in symbols], [2, 2])

    def test_unknown_macro_fails_closed(self):
        with self.assertRaises(subprocess.CalledProcessError):
            self.inventory("fn f() { custom!(name => |x| x); }")

    def test_empty_match_and_macro_decisions(self):
        symbols = self.inventory('fn f(x: !) { match x {} } fn g(x: bool) { println!("{}", if x { 1 } else { 2 }); }')["symbols"]
        self.assertEqual([complexity(s["raw"]) for s in symbols], [1, 2])

    def test_generic_closure_type_is_not_a_closure_function(self):
        self.assertFalse(closure_name("crate::run::<crate::parent::{closure#0}>"))
        self.assertTrue(closure_name("crate::run::<crate::parent::{closure#0}>::{closure#1}"))
        self.assertTrue(closure_name("crate::parent::{{closure}}"))

    def test_ratchet_preserves_uncovered_unchanged_closure_debt(self):
        rows = []
        for source, names in HOTSPOTS.items():
            for name in names:
                rows.append(dict(source=source, name=name, kind="function", syntax_sha256=digest(name.encode()),
                                 source_sha256="a" * 64, span=[1, 1, 1, 20], cc=1,
                                 crap_exact=[1, 1], passed=True))
        closure = dict(rows[0], name="run_check::closure_1_1", kind="closure", syntax_sha256="b" * 64,
                       passed=False, crap_exact=[2, 1])
        base = {"series": SERIES, "configuration": compiler_configuration(), "tools": {"fixture": "same"}, "functions": rows + [closure]}
        head = copy.deepcopy(base)
        head["functions"][-1]["name"] = "check_path::closure_2_1"
        head["functions"][-1]["span"] = [2, 1, 2, 20]
        result = compare(base, head)
        self.assertEqual(result["failures"], [])
        identity = result["identities"][-1]
        self.assertFalse(identity["changed"])
        self.assertTrue(identity["coverage_debt"])
        self.assertEqual(identity["base"]["name"], closure["name"])
        head["functions"][0]["passed"] = False
        self.assertEqual(len(compare(base, head)["failures"]), 1)
        head["series"] = dict(SERIES, mapping="different")
        with self.assertRaisesRegex(ValueError, "incompatible"):
            compare(base, head)
        head = copy.deepcopy(base)
        head["functions"].pop(1)
        with self.assertRaisesRegex(ValueError, "missing extracted hotspot"):
            compare(base, head)

    def test_production_outside_src_and_absent_base_are_explicit(self):
        with tempfile.TemporaryDirectory(dir=ROOT / "target") as temp:
            crate = Path(temp)
            (crate / "src").mkdir()
            (crate / "quality-core").mkdir()
            source = "pub fn evaluate() { let f = |x: bool| x; }"
            (crate / "quality-core/core.rs").write_bytes(source.encode())
            paths = ["../quality-core/core.rs", "app/quality.rs"]
            manifest = prepare(crate, self.binary, paths)
            self.assertEqual(manifest["absent_sources"], ["app/quality.rs"])
            self.assertEqual(manifest["files"][paths[0]]["original"], source)
            self.assertEqual(len(manifest["files"][paths[0]]["inventory"]["symbols"]), 2)
            transformed, _ = instrument(source, manifest["files"][paths[0]]["inventory"])
            self.assertEqual((crate / "quality-core/core.rs").read_bytes(), transformed.encode())
            with self.assertRaisesRegex(ValueError, "missing selected source"):
                prepare(crate, self.binary, ["app/commands.rs"])

    def test_unicode_nested_source_map(self):
        source = 'fn f() { let é = "é"; let f = |x| |y| x + y; }'
        inventory = self.inventory(source)
        transformed, edits = instrument(source, inventory)
        self.assertEqual(transformed.count("black_box"), 2)
        for token in ("x + y", "; }"):
            old = len(source[:source.index(token)].encode()) + 1
            new = len(transformed[:transformed.index(token)].encode()) + 1
            self.assertEqual(original_point([1, new], edits), (1, old))

    def test_instrumentation_distinguishes_unexecuted_closure_and_rejects_missing_evidence(self):
        source = '''#[cfg(unix)] fn unix_value() -> u8 { 1 }
#[cfg(windows)] fn windows_value() -> u8 { 2 }
struct Step { passed: bool }
#[inline(never)]
fn field_only(steps: &[Step]) -> bool {
    #[cfg(INACTIVE_HOST)] {
        let hidden = || if true { 10 } else { 20 };
        unsupported!(a => b);
    }
    steps.iter().all(|step| step.passed)
}
#[cfg(test)] mod tests { #[test] fn ignored() { unsupported!(a => b); } }
fn main() {
    #[cfg(unix)] { assert_eq!(unix_value(), 1); }
    #[cfg(windows)] { assert_eq!(windows_value(), 2); }
    let empty = std::env::args().nth(1).unwrap() == "empty";
    if empty { assert!(field_only(&[])); }
    else { assert!(!field_only(&[Step { passed: false }])); }
}
'''.replace('INACTIVE_HOST', 'unix' if 'windows' in compiler_configuration()['cfg'] else 'windows')
        with tempfile.TemporaryDirectory(dir=ROOT / "target") as temp:
            crate = Path(temp)
            (crate / "src").mkdir()
            (crate / "quality-core").mkdir()
            file = crate / "quality-core/fixture.rs"
            file.write_bytes(source.encode())
            inventory = ast(file, self.binary)
            self.assertEqual(len([s for s in inventory['symbols'] if s['kind'] == 'closure']), 1)
            transformed, edits = instrument(source, inventory)
            file.write_bytes(transformed.encode())
            manifest = {"series": SERIES, "configuration": compiler_configuration(), "files": {"../quality-core/fixture.rs": {
                "original": source, "original_sha256": digest(source.encode()),
                "instrumented_sha256": digest(transformed.encode()), "edits": edits, "inventory": inventory}}}
            executable = crate / ("fixture" + (".exe" if os.name == "nt" else ""))
            compile_command = ["rustc", "--edition=2021", "--target", manifest["configuration"]["target"],
                               "-C", "instrument-coverage", "-C", "opt-level=0", str(file), "-o", str(executable)]
            subprocess.run(compile_command, check=True, capture_output=True)
            sysroot = subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip()
            host = next(l.split(": ")[1] for l in subprocess.check_output(["rustc", "-vV"], text=True).splitlines() if l.startswith("host:"))
            llvm_bin = Path(sysroot) / "lib/rustlib" / host / "bin"
            suffix = ".exe" if os.name == "nt" else ""
            llvm_tools = {name: llvm_bin / (name + suffix) for name in ("llvm-profdata", "llvm-cov")}
            evidence = Path(os.environ["RUST_MEASURE_NATIVE_EVIDENCE"]) / 'cfg-closure' if os.environ.get("RUST_MEASURE_NATIVE_EVIDENCE") else None
            if evidence:
                evidence.mkdir(parents=True, exist_ok=False)
                (evidence / "instrumentation-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
                (evidence / "fixture.instrumented.rs").write_bytes(transformed.encode())
            results = {}
            for mode in ("empty", "executed"):
                raw, profile = crate / f"{mode}.profraw", crate / f"{mode}.profdata"
                subprocess.run([str(executable), mode], check=True, env={**os.environ, "LLVM_PROFILE_FILE": str(raw)})
                subprocess.run([str(llvm_tools["llvm-profdata"]), "merge", "-sparse", str(raw), "-o", str(profile)], check=True, capture_output=True)
                llvm = json.loads(subprocess.check_output([str(llvm_tools["llvm-cov"]), "export", str(executable), f"-instr-profile={profile}"]))
                if evidence:
                    shutil.copyfile(raw, evidence / raw.name)
                    shutil.copyfile(profile, evidence / profile.name)
                    (evidence / f"{mode}.llvm.json").write_text(json.dumps(llvm) + "\n")
                results[mode] = measure(manifest, llvm, crate, self.binary)
                active = 'windows_value' if 'windows' in manifest['configuration']['cfg'] else 'unix_value'
                inactive = 'unix_value' if active == 'windows_value' else 'windows_value'
                self.assertIn(active, {row['name'] for row in results[mode]})
                self.assertNotIn(inactive, {row['name'] for row in results[mode]})
                parent = next(row for row in results[mode] if row['name'] == 'field_only')
                self.assertLessEqual(parent['lines']['count'], 3, 'uncompiled body lines must leave the denominator')
            closures = {mode: next(s for s in rows if s["kind"] == "closure") for mode, rows in results.items()}
            self.assertEqual(closures["empty"]["instances"][0]["count"], 0)
            self.assertEqual(closures["empty"]["lines"]["covered"], 0)
            self.assertFalse(closures["empty"]["passed"])
            self.assertEqual(closures["executed"]["instances"][0]["count"], 1)
            self.assertTrue(closures["executed"]["passed"])
            missing = copy.deepcopy(llvm)
            missing["data"][0]["functions"].pop(closures["executed"]["instances"][0]["index"])
            with self.assertRaisesRegex(ValueError, "missing LLVM function"):
                measure(manifest, missing, crate, self.binary)
            missing_active = copy.deepcopy(llvm)
            active_row = next(row for row in results['executed'] if row['name'] == active)
            missing_active['data'][0]['functions'].pop(active_row['instances'][0]['index'])
            with self.assertRaisesRegex(ValueError, "missing LLVM function"):
                measure(manifest, missing_active, crate, self.binary)
            unsupported = copy.deepcopy(llvm)
            unsupported["data"][0]["functions"][closures["executed"]["instances"][0]["index"]]["regions"][0][7] = 1
            with self.assertRaisesRegex(ValueError, "unsupported macro expansion"):
                measure(manifest, unsupported, crate, self.binary)
            file.write_bytes((transformed + "\n// changed after collection\n").encode())
            with self.assertRaisesRegex(ValueError, "instrumented source mismatch"):
                measure(manifest, llvm, crate, self.binary)
            file.write_bytes(transformed.encode())
            manifest["files"]["../quality-core/fixture.rs"]["original_sha256"] = "0" * 64
            with self.assertRaisesRegex(ValueError, "original digest mismatch"):
                measure(manifest, llvm, crate, self.binary)
            if evidence:
                report = {"series": SERIES, "configuration": manifest["configuration"],
                          "checkout_sha": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                          "tools": provenance(self.binary), "compile_command": compile_command,
                          "fixture_binary_sha256": digest(executable.read_bytes()),
                          "test_source_sha256": digest(Path(__file__).read_bytes()),
                          "llvm_tools": {name: {"sha256": digest(path.read_bytes()),
                              "version": subprocess.check_output([str(path), "--version"], text=True).strip()}
                              for name, path in llvm_tools.items()},
                          "results": results,
                          "validated": ["inactive function and body exclusion", "zero-hit closure",
                                        "executed closure", "missing active function fails",
                                        "missing closure fails", "macro expansion fails",
                                        "changed instrumented bytes fail", "changed original digest fails"]}
                (evidence / "native-result.json").write_text(json.dumps(report, indent=2) + "\n")
                hashes = {path.name: digest(path.read_bytes()) for path in sorted(evidence.iterdir())}
                (evidence / "sha256.json").write_text(json.dumps(hashes, indent=2) + "\n")


if __name__ == "__main__":
    unittest.main()
