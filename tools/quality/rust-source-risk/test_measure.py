import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from fractions import Fraction
from measure import crap, line_coverage, measure, strict_json

HERE = Path(__file__).resolve().parent

class ArithmeticTests(unittest.TestCase):
    def test_exact_threshold(self):
        self.assertEqual(crap(10, 100, 100), {'numerator': 10, 'denominator': 1})
        self.assertEqual(crap(11, 100, 100), {'numerator': 11, 'denominator': 1})
        self.assertEqual(crap(4, 1, 2), {'numerator': 6, 'denominator': 1})
        self.assertIsNone(crap(1, 0, 0))

    def test_nested_zero_region_overrides_covered_parent(self):
        source = b'outer\nmissed\nend\n'
        self.assertEqual(line_coverage(source, [(0, len(source), 1), (6, 13, 0)], []), (2, 3))

    def test_nested_callable_does_not_borrow_parent_hits(self):
        source = b'outer\nclosure\nend\n'
        self.assertEqual(line_coverage(source, [(0, len(source), 1)], [(6, 14)]), (2, 2))

    def test_crossing_regions_rejected(self):
        with self.assertRaisesRegex(ValueError, 'crossing'):
            line_coverage(b'1234567890', [(0, 7, 1), (3, 10, 0)], [])

    def test_duplicate_json_and_unsafe_integer(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp) / 'data.json'
            for text in ('{"a":1,"a":2}', '{"a":9007199254740992}', '{"a":NaN}'):
                p.write_text(text)
                with self.assertRaises(ValueError): strict_json(p)

class NativeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory(prefix='rust-source-native-test-')
        cls.root = Path(cls.tmp.name)
        cls.source = cls.root / 'sample.rs'
        cls.source.write_text('''fn ten(x: bool) { if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} }
fn eleven(x: bool) { if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} }
async fn unpolled() { if true { std::hint::black_box(1); } }
fn main() { ten(true); eleven(true); std::mem::drop(unpolled()); }
''')
        binary = cls.root / 'sample'
        subprocess.run(['rustc', '--edition=2024', '-C', 'instrument-coverage', str(cls.source), '-o', str(binary)], check=True, capture_output=True)
        import os
        subprocess.run([str(binary)], check=True, env=dict(os.environ, LLVM_PROFILE_FILE=str(cls.root / 'raw-%p.profraw')))
        sysroot = Path(subprocess.check_output(['rustc', '--print', 'sysroot'], text=True).strip())
        host = next(s.split(': ')[1] for s in subprocess.check_output(['rustc', '-vV'], text=True).splitlines() if s.startswith('host: '))
        tools = sysroot / 'lib/rustlib' / host / 'bin'
        profile = cls.root / 'merged.profdata'
        subprocess.run([str(tools / 'llvm-profdata'), 'merge', '-sparse', *map(str, cls.root.glob('*.profraw')), '-o', str(profile)], check=True, capture_output=True)
        cls.llvm = cls.root / 'llvm.json'
        with cls.llvm.open('wb') as output:
            subprocess.run([str(tools / 'llvm-cov'), 'export', '-instr-profile=' + str(profile), str(binary)], check=True, stdout=output, stderr=subprocess.PIPE)

    @classmethod
    def tearDownClass(cls): cls.tmp.cleanup()

    def result(self, path=None):
        return measure(self.root, ['sample.rs'], path or self.llvm, HERE / 'inventory')

    def test_real_native_threshold_and_unpolled_async(self):
        rows = {f['name']: f for f in self.result()['functions']}
        self.assertEqual(rows['ten']['risk.crap'], {'numerator': 10, 'denominator': 1})
        self.assertEqual(rows['eleven']['risk.crap'], {'numerator': 11, 'denominator': 1})
        self.assertEqual(rows['unpolled']['coverage.function']['numerator'], 0)
        self.assertEqual(rows['unpolled']['coverage.line']['numerator'], 0)

    def mutate(self, action):
        data = json.loads(self.llvm.read_text())
        action(data['data'][0]['functions'])
        p = self.root / 'tampered.json'
        p.write_text(json.dumps(data))
        return p

    def test_missing_function_rejected(self):
        p = self.mutate(lambda rows: rows.pop(next(i for i, f in enumerate(rows) if f['regions'][0][:2] == [1, 1])))
        with self.assertRaisesRegex(ValueError, 'missing native mapping'): self.result(p)

    def test_wrong_anchor_rejected(self):
        def change(rows):
            f = next(f for f in rows if f['filenames'][0] == str(self.source))
            f['regions'][0][1] += 1
        with self.assertRaisesRegex(ValueError, 'exact source anchor'): self.result(self.mutate(change))

    def test_duplicate_native_instance_rejected(self):
        def change(rows):
            rows.append(next(f for f in rows if f['filenames'][0] == str(self.source)))
        with self.assertRaisesRegex(ValueError, 'duplicate native'): self.result(self.mutate(change))

if __name__ == '__main__': unittest.main()
