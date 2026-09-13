# GH-259 candidate validation record

This is a partial implementation checkpoint, not release acceptance. T2 is
complete; T3–T8 remain open. The [design](stable-rust-collector.md) defines the
boundary. No Core required binding, threshold, baseline or historical evidence
was changed. No compiler-private backend was built or executed for these results.

## Actual environment and artifact

The same release binary ran every candidate check on Ubuntu 26.04, Linux x86_64
GNU, glibc 2.43. The host kernel is recorded only as an observation, not an
installation fingerprint. Rust 1.97.1 reports commit
`8bab26f4f68e0e26f0bb7960be334d5b520ea452`, LLVM 22.1.6;
matching external LLVM tools report `22.1.6-rust-1.97.1-stable`.
External cargo-llvm-cov is 0.9.0. The actual export format is LLVM JSON 3.1.0.

The candidate binary is **1,727,880 bytes**, SHA-256
`06ee8edb7b5a85813c20b6b5fa68b501dec6b4a7ef7e398e3a3285e0831bcbf3`.
ELF `NEEDED` entries are `libgcc_s.so.1` and `libc.so.6`; no Python or compiler
private shared library appears. This observation alone is not a transitive
process audit. The plain capture occupies 42,063 bytes. Its temporary Cargo
build directory is removed and persistent candidate cache is zero bytes.
The actual unsigned candidate package occupies 2,781,383 bytes: program 1,727,880,
collected licenses 1,052,044, support metadata 896 and inventory 563 bytes. The
[preparation record](stable-rust-candidate-evidence/preparation.json) binds the
locked build, 67 registry dependency notice inventories, Rust library notices and
acceptance for that exact executable. It retains conservative build-dependency
notices; production license review remains open. Full notices and build logs are
in the workspace review directory, without repackaged raw dependency archives.

The local test-only signed package and one installed version occupy 2,781,852
bytes each. Two retained versions occupy 5,563,704 bytes. Interrupted staging
occupies 2,781,852 bytes; the exercised root totals 8,346,171 bytes including
verification metadata. Local input operations downloaded zero bytes; a network
downloader is not implemented. The prepared package is deliberately unsigned;
the lifecycle package uses test RSA and mocked Sigstore metadata. These sizes are
actual candidate measurements, not a production archive/download claim. Build
targets and acceptance logs are not runtime payloads.

The checked-in [acceptance summary](stable-rust-candidate-evidence/acceptance.json)
contains actual tool hashes, binary identity, all 36 checks, raw totals and
capture/request anchors. [Environment probes](stable-rust-candidate-evidence/environment.json)
record exact commands, errors and outputs. [ELF dependencies](stable-rust-candidate-evidence/dynamic-libraries.txt)
record the binary inspection. The [log identity index](stable-rust-candidate-evidence/log-identities.json)
records hashes and sizes of retained workspace logs. These are new candidate observations, not repackaged
historical evidence. The separate [plain](stable-rust-candidate-evidence/core-plain.json),
[boundaries](stable-rust-candidate-evidence/core-boundaries.json) and
[features](stable-rust-candidate-evidence/core-features.json) summaries record
authenticated Core acceptance using an explicitly test-only key, not production signing. Full captures/logs remain
under `target/gh-259/acceptance-11/` in the execution workspace; the summary is
not a standalone verifiable capture archive. CI uploads its own complete captures.

## Real fixture observations

Both fixtures are dependency-free Cargo projects checked into
`tools/quality/fixtures/rust-stable`. They compile and test using the project's
selected stable toolchain. Each capture records subprocess arguments, environment,
stdout/stderr, exit status and hashes. Positive collection does not use mocks.

| Capture | Raw lines | Raw regions | Raw functions | Source observation |
| --- | --- | --- | --- | --- |
| Plain | 9/12 | 12/15 | 2/3 | `classify` lexical CC 3; `never_called` CC 1 and actual zero execution |
| Boundaries, default features | 16/18 | 30/37 | 6/8 | Async constructor executes once, body zero; closure/generic/macro owners unsupported |
| Boundaries, `extra` feature | 18/20 | 35/42 | 7/9 | Feature function appears in raw export; lexical cfg owner remains unsupported |

Raw coverage is test-inclusive, while source analysis lexically excludes tests.
These denominators must not be joined for CRAP. The boundary fixture also
executes macro-expanded and build-generated functions, derives traits and leaves
an async future unpolled. Its assertions inspect known fixture symbols solely
to establish observations; that suffix matching is not a certified production
owner mapping. Uninstantiated generic ownership remains unproven. All candidate
function CRAP and normalized Core coverage remain unsupported.

The 36 actual checks include valid capture/integrity verification, stale output,
wrong request/manifest anchors, damaged and mixed artifacts, changed source,
restored source, boundary/features captures, wrong tool identity, injected Cargo
configuration, missing tools, inherited compiler flags, unavailable project pin,
symlink source and a real failing test subprocess. These checks confirm the
implemented failure boundaries; the separate real Core acceptance below exercises signature/replay protection.
The raw LLVM validator previously accepted a negative function execution count
when the test manifest was deliberately re-anchored; the [reproduction](stable-rust-candidate-evidence/coverage-reproduction.json)
records exit 0 from the preceding binary (`f4f71cc58f51194ab4b9c9c976bd34360ddb0bf471c414f7c7439949353ee81e`).
This exposed a semantic check missing after integrity verification, not a forged
signature. The current release rejects that input. Thirteen additional checks
mutate actual LLVM output, update the test-only manifest anchor, and assert
nonzero failure: negative/overflow/fractional counts, missing regions, invalid
file IDs, reversed spans, unknown region kinds, impossible coverage totals,
inconsistent percentages/notcovered, invalid segment booleans, MC/DC capability
and duplicate object keys. Both original files are restored after each suite.
Unsigned package preparation requires these named checks in the pinned acceptance.

These checks validate the exercised JSON format, scalar domains and local summary
arithmetic. They do not reconstruct summaries from counters, certify cross-file
aggregate totals, or establish source/coverage owners. Nonempty branch/expansion
layouts have structural checks but no real producer acceptance in this matrix;
MC/DC is explicitly unsupported. Required coverage and CRAP stay blocked.

Rust unit tests separately exercise nonzero exit, timeout/process-group cleanup,
missing executable, source decisions/unsupported owners, symlinks, tampering,
malformed LLVM shape/version and strict JSON duplicate-key rejection.
The execution-audit scanner has synthetic regression tests; those are logic tests,
not cross-host acceptance.

## Actual Core and migration checks

The same collector binary was used by all three real Core acceptance cases at
`target/gh-259/core-acceptance-{plain,boundaries,features}-07/`. Each invokes the
actual Core CLI with a signed v2 request, then runs Core's generic evidence
validation and requiredness evaluator. Plain produces two verified complexity
counts, 3 and 1. Boundary fixtures produce unavailable evidence with unsupported
capabilities and no metrics. Required CRAP blocks in every case.

All three cases reject invalid signatures, replay, expiry, altered project/series,
unknown owners, changed source identity, wrong context target, missing claims,
wrong capture anchor and a stale configuration binding. Core independently rejects
stale context and corrupted normalized artifacts. Test keys and synthetic fixture
context commits establish protocol behavior, not protected production signing or
user-project Git provenance. No Core production implementation was changed.

The [same-source historical comparison](stable-rust-collector-migration.md) also
ran with that binary. Original archive/source/report anchors remain intact; new
raw line/region/function totals differ. Its summary is retained separately from
Core acceptance and does not recertify the archived historical report.

## Actual offline lifecycle checks

The [lifecycle summary](stable-rust-candidate-evidence/lifecycle.json) records 33
executed checks with the same collector binary. RSA-2048/SHA-256 signatures are
real, generated by repository-only OpenSSL automation using a test-only key.
The installed binary is actually executed after install and upgrade. Both versions
contain the same program bytes with different signed version metadata; this proves
selection behavior, not compatibility between independently released programs.

Checks cover unsigned-package rejection, ten re-signed invalid support documents,
both-signature requirements, malformed RSA, payload/inventory mixing,
extra assets, symlinks, duplicate JSON, missing/changed verifiers, nonzero verifier
exit, a real 60-second timeout, corrupt/nonexecutable rollback targets, concurrent
lock rejection and SIGKILL during verification followed by retry and rollback.
Each rejected upgrade/rollback checks that the prior selection and binary remain
unchanged. Sigstore is an explicitly mocked external command checking fixed
arguments and exit behavior; neither Sigstore cryptography nor production signing
is established. No mock flag exists in the candidate executable.

The [lifecycle contract](stable-rust-collector-lifecycle.md) documents the exact
flat package, externally pinned trust and atomic selection. Local test packages
contain the prepared program, real collected licenses and typed support metadata.
Generated private keys, mock verifier, logs and acceptance artifacts stay outside
those packages. Test versions and signature envelopes remain development inputs.
Unsigned preparation fails if an acceptance pin, binary identity, archived license
or unpacked build source does not match; six automated preparation tests cover
these boundaries. The first actual preparation rejected acceptance for an earlier
binary (`package-01.log`); the final binary was recaptured before successful
preparation (`package-03.log`).

## Commands and results

Commands ran from the assigned workspace. Existing Core code/check results remain
unchanged; candidate execution, packaging, Python suites and documentation were
validated again for this checkpoint. `CARGO_TARGET_DIR` was redirected into
this workspace because the default `/home/gem/cargo-target` is read-only.

| Command | Actual result and log under `target/gh-259/` |
| --- | --- |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/stable-target" RUSTFLAGS= CARGO_ENCODED_RUSTFLAGS= RUSTC_WRAPPER= RUSTC_WORKSPACE_WRAPPER= cargo +1.97.1 build --manifest-path tools/quality/rust-stable-collector/Cargo.toml --release --locked --offline` | Passed initial build; final artifact rebuilt by preparation below; `coverage-build-01.log` |
| Same target, `cargo +1.97.1 test --manifest-path tools/quality/rust-stable-collector/Cargo.toml --locked --offline` | 9 passed; `coverage-tests-02.log` |
| `cargo +1.97.1 fmt --manifest-path tools/quality/rust-stable-collector/Cargo.toml -- --check` | Passed; `coverage-fmt-01.log` |
| Same target, `cargo +1.97.1 clippy --manifest-path tools/quality/rust-stable-collector/Cargo.toml --all-targets --locked -- -D warnings` | Passed; `coverage-clippy-01.log` |
| `python3 tools/quality/rust-stable-collector/validate_stable_candidate.py --binary target/gh-259/stable-target/release/harness-gate-rust-stable-collector --output target/gh-259/acceptance-11` | 36 passed; `acceptance-11.log` |
| `env -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u http_proxy -u https_proxy -u all_proxy CARGO_TARGET_DIR="$PWD/target/gh-259/core-target" cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | 397 passed; `core-nextest-02.log` |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Passed; `core-fmt-02.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/core-target" cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Passed; `core-clippy-02.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/quality-target" python3 -m unittest discover -s tools/quality/tests -v` | 444 tests OK, 36 skipped; `quality-tests-06.log` |
| `python3 -m unittest discover -s tools/release/tests -v` | 91 passed; `release-tests-04.log` |
| `python3 -m unittest discover -s tools/release/tests -p test_stable_collector_policy.py -v` | 2 passed after CI build environment alignment; `package-ci-policy-01.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/quality-target" python3 -m unittest discover -s tools/quality/tests -p test_ci_topology.py -v` | 7 passed after CI build environment alignment; `package-ci-topology-01.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/core-target" python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Passed; `docs-consistency-07.log` |
| Same Core target, `cargo +1.97.1 build --manifest-path tools/harness-gate/Cargo.toml --locked --offline --bin harness-gate --example stable_collector_acceptance` | Passed; `core-adapter-build.log` |
| `target/gh-259/core-target/debug/examples/stable_collector_acceptance target/gh-259/stable-target/release/harness-gate-rust-stable-collector target/gh-259/core-target/debug/harness-gate target/gh-259/acceptance-11 target/gh-259/core-acceptance-${fixture}-07 $fixture` for `plain boundaries features` | All three passed; matching `core-acceptance-${fixture}-07.log` |
| `python3 tools/quality/rust-stable-collector/compare_historical_fixture.py --binary target/gh-259/stable-target/release/harness-gate-rust-stable-collector --output target/gh-259/historical-04` | Passed; `historical-04.log` |
| `python3 tools/quality/rust-stable-collector/validate_lifecycle.py --binary target/gh-259/stable-target/release/harness-gate-rust-stable-collector --package target/gh-259/package-03/unsigned-package --output target/gh-259/lifecycle-05` | 33 passed; real RSA, mocked Sigstore; `lifecycle-05.log` |
| `python3 tools/quality/rust-stable-collector/prepare_release.py --output target/gh-259/package-03 --target-dir "$PWD/target/gh-259/stable-target" --toolchain 1.97.1 --acceptance target/gh-259/acceptance-11/summary.json 9ebdeb90c794789859914fd7b6eac6c8d75615f945318df56802d35483670da6` | Passed locked offline build and package preparation; `package-03.log` and `package-03/preparation.json` |
| `harness-gate config check`; `harness-gate verify --profile ci --all` | Not applicable: no `.harness-gate/flow.toml` declaring `ci`; neither was run |

The initial Core nextest run hit a local webhook failure with inherited proxies;
removing proxy variables resolved it. Initial candidate probes exposed LLVM's
Rust-specific version suffix, cargo subcommand argument handling and export
schema 3.1.0; those were fixed before the recorded successful release runs.
The first checkpoint quality run found a missing draft documentation link and a
new development script in the production-module inventory directory. The link
was completed and the development driver moved into its crate directory.
The continuation quality run started before the new migration document existed
and reported a documentation link failure (`quality-tests-02.log`); the final
full rerun uses the completed documentation. Compiler-private test classes now
require an explicit manual-experiment opt-in;
required CI retains all other existing checks and does not bootstrap that backend.

## Acceptance still blocked or unimplemented

- Rust 1.98.1 is not installed. The [official release index](https://blog.rust-lang.org/releases/)
  was checked on 2026-09-13: 1.97.1 was released on July 16 and 1.98.1 on
  September 3. Fetching the installable stable manifest from this workspace failed
  `curl: (56) Proxy CONNECT aborted`. Only installed 1.97.1 was executed;
  the second candidate's interfaces and runtime remain unverified.
  Doctor's candidate allowlist is not evidence of support for both versions.
- A second runnable system was unavailable: Docker socket access was denied.
  Only the environment above has been exercised; there is no released support
  platform yet and no claim covering all Linux systems.
- `strace` is installed but the sandbox rejects `PTRACE_TRACEME` and
  `PTRACE_SEIZE`. Local command records, build logs and ELF inspection passed;
  a complete transitive `execve` audit did not run. Required CI now requests real
  release-build and runtime traces and fails if tracing fails; CI is pending.
- The Core candidate only certifies lexical complexity for the declared limited
  owners. It does not certify normalized coverage/CRAP or replace existing required
  measurements. Production request signing and project integration remain open.
- General dependency projects, certified source/coverage owners, comprehensive
  adversarial coverage formats, complete negative matrices and reviewed migration
  still need implementation/acceptance. No MIR backend was rerun and
  original historical anchors remain untouched.
- Rust offline verification/install/upgrade/rollback now pass real RSA and
  transaction tests. Unsigned package preparation and an authenticated notice inventory
  are implemented. Protected signing/publication, production license review, trusted
  bootstrap, downloader/cache accounting and full release acceptance remain open.
  The Sigstore verifier is unavailable locally (`cosign version`: no such file
  or directory). The [continuation probes](stable-rust-candidate-evidence/continuation-environment.json)
  retain this error and the installed toolchain list.
  No production signing was simulated
  or claimed; no user installation was changed. The release hold remains.

GH-259 must remain open after this checkpoint. T8 and production publication
cannot proceed on this evidence.
