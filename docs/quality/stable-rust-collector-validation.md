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

The candidate binary is **1,681,064 bytes**, SHA-256
`a7e348c64089cbb592e1b094558b937066748c9979ba253218521511c1693a79`.
ELF `NEEDED` entries are `libgcc_s.so.1` and `libc.so.6`; no Python or compiler
private shared library appears. This observation alone is not a transitive
process audit. The plain capture occupies 42,063 bytes. Its temporary Cargo
build directory is removed and persistent candidate cache is zero bytes.
The local test-only signed package and one installed version occupy 1,682,216
bytes each. The two retained versions occupy 3,364,432 bytes. Interrupted staging
and verification metadata bring the exercised root to 5,047,257 bytes. Local input
operations downloaded zero bytes; a network downloader is not implemented. These
are test-package measurements, not a production package or complete license inventory. Build targets and acceptance logs are
repository development artifacts and are not proposed runtime payloads.

The checked-in [acceptance summary](stable-rust-candidate-evidence/acceptance.json)
contains actual tool hashes, binary identity, all 23 checks, raw totals and
capture/request anchors. [Environment probes](stable-rust-candidate-evidence/environment.json)
record exact commands, errors and outputs. [ELF dependencies](stable-rust-candidate-evidence/dynamic-libraries.txt)
record the binary inspection. The [log identity index](stable-rust-candidate-evidence/log-identities.json)
records hashes and sizes of retained workspace logs. These are new candidate observations, not repackaged
historical evidence. The separate [plain](stable-rust-candidate-evidence/core-plain.json),
[boundaries](stable-rust-candidate-evidence/core-boundaries.json) and
[features](stable-rust-candidate-evidence/core-features.json) summaries record
authenticated Core acceptance using an explicitly test-only key, not production signing. Full captures/logs remain
under `target/gh-259/acceptance-08/` in the execution workspace; the summary is
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

The 23 actual checks include valid capture/integrity verification, stale output,
wrong request/manifest anchors, damaged and mixed artifacts, changed source,
restored source, boundary/features captures, wrong tool identity, injected Cargo
configuration, missing tools, inherited compiler flags, unavailable project pin,
symlink source and a real failing test subprocess. These checks confirm the
implemented failure boundaries; the separate real Core acceptance below exercises signature/replay protection.
Rust unit tests separately exercise nonzero exit, timeout/process-group cleanup,
missing executable, source decisions/unsupported owners, symlinks, tampering,
malformed LLVM shape/version and strict JSON duplicate-key rejection.
The execution-audit scanner has synthetic regression tests; those are logic tests,
not cross-host acceptance.

## Actual Core and migration checks

The same collector binary was used by all three real Core acceptance cases at
`target/gh-259/core-acceptance-{plain,boundaries,features}-05/`. Each invokes the
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

The [lifecycle summary](stable-rust-candidate-evidence/lifecycle.json) records 22
executed checks with the same collector binary. RSA-2048/SHA-256 signatures are
real, generated by repository-only OpenSSL automation using a test-only key.
The installed binary is actually executed after install and upgrade. Both versions
contain the same program bytes with different signed version metadata; this proves
selection behavior, not compatibility between independently released programs.

Checks cover both-signature requirements, malformed RSA, payload/inventory mixing,
extra assets, symlinks, duplicate JSON, missing/changed verifiers, nonzero verifier
exit, a real 60-second timeout, corrupt/nonexecutable rollback targets, concurrent
lock rejection and SIGKILL during verification followed by retry and rollback.
Each rejected upgrade/rollback checks that the prior selection and binary remain
unchanged. Sigstore is an explicitly mocked external command checking fixed
arguments and exit behavior; neither Sigstore cryptography nor production signing
is established. No mock flag exists in the candidate executable.

The [lifecycle contract](stable-rust-collector-lifecycle.md) documents the exact
flat package, externally pinned trust and atomic selection. Local test packages
contain only the program and necessary test metadata. Generated private keys,
mock verifier, logs and acceptance artifacts stay outside those packages. Their
TEST ONLY license/support documents are not production distribution metadata.

## Commands and results

Commands ran from the assigned workspace. `CARGO_TARGET_DIR` was redirected into
this workspace because the default `/home/gem/cargo-target` is read-only.

| Command | Actual result and log under `target/gh-259/` |
| --- | --- |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/stable-target" cargo +1.97.1 build --manifest-path tools/quality/rust-stable-collector/Cargo.toml --release --locked --offline` | Passed; `stable-lifecycle-build-02.log` |
| Same target, `cargo +1.97.1 test --manifest-path tools/quality/rust-stable-collector/Cargo.toml --locked --offline` | 8 passed; `stable-final-tests.log` |
| `cargo +1.97.1 fmt --manifest-path tools/quality/rust-stable-collector/Cargo.toml -- --check` | Passed; `stable-final-fmt.log` |
| Same target, `cargo +1.97.1 clippy --manifest-path tools/quality/rust-stable-collector/Cargo.toml --all-targets --locked --offline -- -D warnings` | Passed; `stable-final-clippy.log` |
| `python3 tools/quality/rust-stable-collector/validate_stable_candidate.py --binary target/gh-259/stable-target/release/harness-gate-rust-stable-collector --output target/gh-259/acceptance-08` | 23 passed; `acceptance-08.log` |
| `env -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u http_proxy -u https_proxy -u all_proxy CARGO_TARGET_DIR="$PWD/target/gh-259/core-target" cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | 397 passed; `core-nextest-02.log` |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Passed; `core-fmt-02.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/core-target" cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Passed; `core-clippy-02.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/quality-target" python3 -m unittest discover -s tools/quality/tests -v` | 444 tests OK, 36 skipped; `quality-tests-04.log` |
| `python3 -m unittest discover -s tools/release/tests -v` | 85 passed; `release-tests.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/core-target" python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Passed; `docs-consistency-04.log` |
| Same Core target, `cargo +1.97.1 build --manifest-path tools/harness-gate/Cargo.toml --locked --offline --bin harness-gate --example stable_collector_acceptance` | Passed; `core-adapter-build.log` |
| `target/gh-259/core-target/debug/examples/stable_collector_acceptance target/gh-259/stable-target/release/harness-gate-rust-stable-collector target/gh-259/core-target/debug/harness-gate target/gh-259/acceptance-08 target/gh-259/core-acceptance-${fixture}-05 $fixture` for `plain boundaries features` | All three passed; matching `core-acceptance-${fixture}-05.log` |
| `python3 tools/quality/rust-stable-collector/compare_historical_fixture.py --binary target/gh-259/stable-target/release/harness-gate-rust-stable-collector --output target/gh-259/historical-02` | Passed; `historical-02.log` |
| `python3 tools/quality/rust-stable-collector/validate_lifecycle.py --binary target/gh-259/stable-target/release/harness-gate-rust-stable-collector --output target/gh-259/lifecycle-03` | 22 passed; real RSA, mocked Sigstore; `lifecycle-03.log` |
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
  transaction tests. Protected packaging, license completeness, trusted bootstrap,
  downloader/cache accounting and full release acceptance remain unimplemented.
  The Sigstore verifier is unavailable locally (`cosign version`: no such file
  or directory). The [continuation probes](stable-rust-candidate-evidence/continuation-environment.json)
  retain this error and the installed toolchain list.
  No production signing was simulated
  or claimed; no user installation was changed. The release hold remains.

GH-259 must remain open after this checkpoint. T8 and production publication
cannot proceed on this evidence.
