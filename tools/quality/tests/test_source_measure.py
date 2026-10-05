from __future__ import annotations

import copy
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tools/quality"))
from source_measure import HOTSPOTS, SERIES, SOURCE_FILES, prepare, ast, closure_name, compare, compiler_configuration, complexity, digest, instrument, measure, original_point, provenance


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
            evidence = Path(os.environ["RUST_MEASURE_NATIVE_EVIDENCE"]) if os.environ.get("RUST_MEASURE_NATIVE_EVIDENCE") else None
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
