from __future__ import annotations

import copy
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tools/quality"))
from source_measure import HOTSPOTS, SERIES, ast, closure_name, compare, complexity, digest, instrument, measure, original_point


class SourceMeasureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        target = ROOT / "target/gh-94-measure"
        subprocess.run(["cargo", "build", "--locked", "--manifest-path",
                        str(ROOT / "tools/quality/rust-measure/Cargo.toml")],
                       env={**os.environ, "CARGO_TARGET_DIR": str(target)}, check=True, capture_output=True)
        cls.binary = target / "debug/harness-gate-rust-measure"

    def inventory(self, source):
        with tempfile.TemporaryDirectory() as temp:
            file = Path(temp) / "input.rs"
            file.write_text(source)
            return ast(file, self.binary)

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
        self.assertEqual([s["name"] for s in symbols], ["outer", "outer::nested", "outer::closure_5_34", "tests::only_test"])
        self.assertEqual([complexity(s["raw"]) for s in symbols], [4, 2, 2, 1])
        self.assertEqual([s["test"] for s in symbols], [False, False, False, True])
        self.assertEqual(symbols[0]["raw"]["nested_functions"], 1)
        self.assertNotIn("and_and", symbols[0]["raw"])

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
        base = {"series": SERIES, "tools": {"fixture": "same"}, "functions": rows + [closure]}
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
        source = '''struct Step { passed: bool }
#[inline(never)]
fn field_only(steps: &[Step]) -> bool { steps.iter().all(|step| step.passed) }
fn main() {
    let empty = std::env::args().nth(1).unwrap() == "empty";
    if empty { assert!(field_only(&[])); }
    else { assert!(!field_only(&[Step { passed: false }])); }
}
'''
        with tempfile.TemporaryDirectory(dir=ROOT / "target") as temp:
            crate = Path(temp)
            (crate / "src").mkdir()
            file = crate / "src/fixture.rs"
            file.write_text(source)
            inventory = ast(file, self.binary)
            transformed, edits = instrument(source, inventory)
            file.write_text(transformed)
            manifest = {"series": SERIES, "files": {"fixture.rs": {
                "original": source, "original_sha256": digest(source.encode()),
                "instrumented_sha256": digest(transformed.encode()), "edits": edits, "inventory": inventory}}}
            executable = crate / "fixture"
            subprocess.run(["rustc", "--edition=2021", "-C", "instrument-coverage", "-C", "opt-level=0",
                            str(file), "-o", str(executable)], check=True, capture_output=True)
            sysroot = subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip()
            host = next(l.split(": ")[1] for l in subprocess.check_output(["rustc", "-vV"], text=True).splitlines() if l.startswith("host:"))
            llvm_bin = Path(sysroot) / "lib/rustlib" / host / "bin"
            results = {}
            for mode in ("empty", "executed"):
                raw, profile = crate / f"{mode}.profraw", crate / f"{mode}.profdata"
                subprocess.run([str(executable), mode], check=True, env={**os.environ, "LLVM_PROFILE_FILE": str(raw)})
                subprocess.run([str(llvm_bin / "llvm-profdata"), "merge", "-sparse", str(raw), "-o", str(profile)], check=True, capture_output=True)
                llvm = json.loads(subprocess.check_output([str(llvm_bin / "llvm-cov"), "export", str(executable), f"-instr-profile={profile}"]))
                results[mode] = measure(manifest, llvm, crate, self.binary)
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
            unsupported = copy.deepcopy(llvm)
            unsupported["data"][0]["functions"][closures["executed"]["instances"][0]["index"]]["regions"][0][7] = 1
            with self.assertRaisesRegex(ValueError, "unsupported macro expansion"):
                measure(manifest, unsupported, crate, self.binary)
            file.write_text(transformed + "\n// changed after collection\n")
            with self.assertRaisesRegex(ValueError, "instrumented source mismatch"):
                measure(manifest, llvm, crate, self.binary)
            file.write_text(transformed)
            manifest["files"]["fixture.rs"]["original_sha256"] = "0" * 64
            with self.assertRaisesRegex(ValueError, "original digest mismatch"):
                measure(manifest, llvm, crate, self.binary)


if __name__ == "__main__":
    unittest.main()
