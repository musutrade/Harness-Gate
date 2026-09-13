# GH-259 same-source historical comparison

This is a measured migration probe, not equivalence certification or baseline
adoption. The stable candidate actually compiled and ran the original source
bytes with Rust 1.97.1 and matching LLVM 22.1.6. No legacy backend ran.

## Preserved original evidence

The original archive is [GH-220 native fixtures](gh-220/native-fixtures.tar.gz),
SHA-256 `0ceb9051ec35c0047bb2c083fab25fb1ba0f72a120b147d1ad3307889ef10391`.
Its [original index](gh-220/native-fixtures.json) still records the `complete`
fixture at `target/gh-220/native-tests/tmps4u3czr1/complete`.
The probe reads only these existing members without modifying or repacking them:

| Original member | SHA-256 |
| --- | --- |
| `complete/evidence.json` | `8ddb8b0fd9f5bd5c402e0754ec0846a54b6e5da8c13876462d1b28bb4479e683` |
| `complete/report.json` | `e728a77f257929c7dc4463df778814998e8699f76af9c22c3bb79a2435f71f4d` |
| `complete/raw/source.rs` | `5848ae7e66cdec35029a945b55d6f4b972c4f1b51256d1e050bba5cb45f0722d` |

The index, report and original manifest anchor agree; the source matches the
manifest and report hashes. The report remains an archived summary, not a newly
certified historical result. Its `backend_complete` and
`source_provenance_complete` are both false. These limitations are preserved.

## Actual differences

The historical series is `rust-native-production-experiment/1`, with
`mir-normal-cfg/1`, `same-mir-owner-sum-counters/1`, `any-owned-code-region/1`,
`mir-instrument-coverage-exact-regions/1` and `single-crate-no-test-cfg/1`.
The stable series uses `rust-source-decisions/v1-candidate` and ordinary
`rust-llvm-source-coverage/v1-candidate`. Its complete Core series and raw totals
are in the [comparison record](stable-rust-candidate-evidence/historical-comparison.json).

| Observation | Original archived report | Actual stable capture |
| --- | --- | --- |
| Lines covered/total | 17/19 | 18/21 |
| Regions covered/total | 28/36 | 24/31 |
| Functions covered/total | 7/8 MIR owners | 6/7 LLVM functions; instantiations 8/9 |
| Async | Constructor CC 1/count 1; body CC 2/count 0/CRAP 6 | Lexical async owner unsupported; no inherited constructor coverage |
| Closure | Closure body CC 2/count 1; region threshold failed | Closure owner and parent complexity unsupported |
| Generic | CC 1/count 2 after historical instance aggregation | Instantiation ownership unsupported |
| Macros/cfg | Historical compiled owners include `first` and `second` | Expansion/activation uncertified; no fabricated missing owners or zero values |
| Required risk | Archived failures include closure body and async body | CRAP unsupported; remains blocking when required |

For the new capture, a dependency-free Cargo `harness=false` test target runs the
unchanged source `main`. This is a new build configuration, not a claim of identical
native flags. All five lexical owners in the source are unavailable because their
own syntax or file activation/expansion cannot be certified. The LLVM totals are
diagnostic and have no certified production/test split. Different denominators
and owner models prohibit direct trend, threshold or CRAP comparison.

The separate plain fixture now certifies exact root-function execution coverage
as 1/1 or 0/1. This is a new binary-bound normalization and does not provide the
intra-function fraction needed by CRAP. The historical fixture contains unsupported
owners, so its test-inclusive totals remain diagnostic.

The separate plain/boundaries/features tests exercise actual authenticated Core
reading; this historical probe performs capture/integrity/description checks and
does not claim a new Core certificate for the archived report. Test-only signatures
in those tests are not protected production signatures.

## Migration decision still required

Keep existing required series, thresholds, debt/ratchets and baselines unchanged.
No candidate binding is automatically substituted. Review must identify which
required claims the new series can certify, approve source/coverage ownership and
activation boundaries, and retain blocking for unmet claims. No transition or new
baseline is approved by this comparison. T6 remains open for reviewed migration,
the second stable toolchain and second runnable system.

Reproduce with `tools/quality/rust-stable-collector/compare_historical_fixture.py`
as documented in the [candidate commands](stable-rust-collector.md). This is
repository development automation; the measured plugin executes only Rust and
its declared external tools. New raw captures remain at
`target/gh-259/historical-05/`; runtime payloads do not include the old archive or
these acceptance captures.
