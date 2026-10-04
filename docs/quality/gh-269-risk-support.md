# GH-269 target-specific risk measurement

PR #293 adds adapter replay state with Unix/Windows implementations. The
required risk series now also inventories `process/replay.rs`,
`project/discovery.rs`, and `project/mod.rs`. This changes measurement support;
it does not change adapter behavior, risk thresholds, or CI stage policy.

The AST analyzer is `harness-gate-rust-measure/0.3.1`. Both base and head must
be measured again with this analyzer, the same tool fingerprints, and the same
compiler configuration. Retained 0.3.0 reports are not reinterpreted as 0.3.1.
The McCabe counting rule, closure instrumentation, and incremental ratchet
remain unchanged.

`source_measure.py` reads `rustc --print cfg --target <triple>`. The required
risk collector passes that same triple to snapshot preparation and native
coverage collection/reporting. The exact compiler configuration is retained in
the manifest, AST inventory, each measurement report, and comparison. Reparse
checks and base/head comparison reject changed or missing configuration.

Supported predicates are only `unix`, `windows`, `test`, `not(atom)`, and
`all(atoms)`. The latter handles the existing `cfg(all(test, unix))` test module.
The production boundary sets `test` false and excludes `#[test]` functions.
Inactive modules, functions, methods, fields, and removable statements are
filtered before complexity counting and closure instrumentation. This includes
conditional blocks inside functions. Their original ranges are also excluded
from line-coverage denominators; an enclosing native region cannot manufacture
coverage for them. Original source bytes and positions remain bound to the
measurement, and every remaining production callable still needs its own
native record.

Feature/name-value predicates, `cfg_attr`, `any`, nested predicate expressions,
unsupported attribute positions, and conditional syntax inside opaque macro
arguments fail explicitly without emitting a partial inventory. A disabled
branch is not inspected for runtime macros, but every predicate on its own
attributes is validated. This is not a general macro or Cargo-feature resolver.

Focused checks:

```sh
python3 -B -m unittest discover -s tools/quality/tests -p test_source_measure.py -v
python3 -B -m unittest discover -s tools/quality/tests -p test_ci_quality.py -v
CARGO_TARGET_DIR=target/gh-94-measure cargo clippy \
  --manifest-path tools/quality/rust-measure/Cargo.toml --all-targets --locked -- -D warnings
cargo fmt --manifest-path tools/quality/rust-measure/Cargo.toml -- --check
```

The fixtures read real compiler configurations for Linux and Windows. Only the
Linux fixture is compiled and executed with LLVM coverage locally; configuration
filtering for a Windows triple is not a native Windows runtime measurement.
Production validation runs only the existing risk stage's base/head snapshots,
coverage, measurement, and comparison. It does not run the other Gate stages,
change host network policy, or replay Relay tasks.
