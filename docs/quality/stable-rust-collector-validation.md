# GH-259 candidate validation record

This is a partial implementation checkpoint, not release acceptance. T2 is
complete; T3–T8 remain open. The [design](stable-rust-collector.md) defines the
boundary. No Core required binding, threshold, baseline or historical evidence
was changed. No compiler-private backend was built or executed for these results.

## Actual environment and artifact

The same release binary ran all collection and Core checks on Ubuntu 26.04, Linux x86_64
GNU, glibc 2.43. The host kernel is recorded only as an observation, not an
installation fingerprint. Rust 1.97.1 reports commit
`8bab26f4f68e0e26f0bb7960be334d5b520ea452`, LLVM 22.1.6;
matching external LLVM tools report `22.1.6-rust-1.97.1-stable`.
External cargo-llvm-cov is 0.9.0. The actual export format is LLVM JSON 3.1.0.

The candidate binary is **1,995,304 bytes**, SHA-256
`e020ad59faf78db0133f4a83105b18f0a39d9676ea21c24386dcb662ac476f73`.
ELF `NEEDED` entries are `libgcc_s.so.1` and `libc.so.6`; no Python or compiler
private shared library appears. This observation alone is not a transitive
process audit. The plain capture occupies 48,545 bytes. Its temporary Cargo
build directory is removed and persistent candidate cache is zero bytes.
The actual unsigned candidate package occupies 3,125,938 bytes: program 1,995,304,
collected licenses 1,129,147, support metadata 924 and inventory 563 bytes. The
[preparation record](stable-rust-candidate-evidence/preparation.json) binds the
locked build, 74 registry dependency notice inventories, Rust library notices and
acceptance for that exact executable. It retains conservative build-dependency
notices; production license review remains open. Full notices and build logs are
in the workspace review directory, without repackaged raw dependency archives.

The local test-only signed package and one installed version occupy 3,126,407
bytes each. Two retained versions occupy 6,252,856 bytes. Interrupted staging
occupies 3,126,449 bytes; the exercised root totals 9,379,933 bytes including
verification metadata. Local input operations downloaded zero bytes; a network
downloader is not implemented. The prepared package is deliberately unsigned;
the lifecycle package uses test RSA and mocked Sigstore metadata. These sizes are
actual candidate measurements, not a production archive/download claim. Build
targets and acceptance logs are not runtime payloads.

The checked-in [acceptance summary](stable-rust-candidate-evidence/acceptance.json)
contains actual tool hashes, binary identity, all 85 checks, raw totals and
capture/request anchors. [Environment probes](stable-rust-candidate-evidence/environment.json)
record exact commands, errors and outputs. [ELF dependencies](stable-rust-candidate-evidence/dynamic-libraries.txt)
record the binary inspection. The [log identity index](stable-rust-candidate-evidence/log-identities.json)
records hashes and sizes of retained workspace logs. These are new candidate observations, not repackaged
historical evidence. The separate [plain](stable-rust-candidate-evidence/core-plain.json),
[boundaries](stable-rust-candidate-evidence/core-boundaries.json) and
[features](stable-rust-candidate-evidence/core-features.json) and
[registry](stable-rust-candidate-evidence/core-registry.json) summaries record
authenticated Core acceptance using an explicitly test-only key, not production signing. Full captures/logs remain
under `target/gh-259/acceptance-22/` in the execution workspace; the summary is
not a standalone verifiable capture archive. CI uploads its own complete captures.

## Aggregate consistency checkpoint

The [reproduction](stable-rust-candidate-evidence/aggregate-reproduction.json)
records exit 0 from the preceding binary when a real export's line total was
changed from 9/12 to 1/1. Only the test manifest was reanchored; originals were
restored. The Rust verifier now checks count and covered totals against every
exported file summary with overflow rejection. Individual percentages and
notcovered counts remain independently validated. This follows LLVM 22.1.6's
[JSON exporter](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.6/llvm/tools/llvm-cov/CoverageExporterJson.cpp)
and [file report aggregation](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.6/llvm/tools/llvm-cov/CoverageReport.cpp).

Eleven new real-export mutations preserve each summary's internal arithmetic
while changing totals or file summaries; all must fail verification. Package
preparation requires these cases. A Rust regression covers multiple-file sums,
covered/count disagreement and integer overflow. These checks establish summary
consistency only: full segment/region counter reconciliation and intra-function
coverage/CRAP remain incomplete. No supported owner or measurement series changes.

## Real fixture observations

Plain and boundary fixtures are dependency-free Cargo projects checked into
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
owner mapping. Uninstantiated generic ownership remains unproven. The exact root-owner rule separately certifies plain function execution ratios
(1/1 and 0/1) after excluding explicit test spans. All candidate function CRAP
and normalized line/region coverage remain unsupported.

The 85 actual checks include valid capture/integrity verification, stale output,
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
arithmetic. They do not reconstruct summaries from counters or certify cross-file
aggregate totals. Nonempty branch/expansion
layouts have structural checks but no real producer acceptance in this matrix;
MC/DC is explicitly unsupported. Required line/region coverage and CRAP stay blocked.

The exact root-owner validator additionally rejects seven real-export mutations:
missing LLVM function, duplicate symbol, multiple records for one source function,
cross-file owner, unknown source span, inconsistent entry count and executed
regions under a zero-count function. Two further checks omit the source function
inventory or alter excluded test spans after test-only manifest re-anchoring; both
fail because the verifier recomputes the complete source analysis. Together with
the positive owner description these bring the suite to 46 checks. Unexecuted
functions require their own actual zero-count record. No missing owner is filled
with zero, and no function borrows its parent's execution count. The rule is
limited to ASCII, unannotated, nongeneric root functions with exactly one LLVM
record and exact region envelope. Opaque/argument-position `impl Trait`, Unicode,
modules, attributes and unresolved expansion/activation remain unsupported.
The binary digest in Core normalization identity distinguishes this checkpoint
from earlier candidate evidence; no existing series is silently upgraded.

The [registry acceptance record](stable-rust-candidate-evidence/registry.json)
added 13 checks; compiler-input checks brought the suite to 74; aggregate reconciliation adds 11 for 85. The preceding binary rejected the real
locked `itoa` fixture before implementation. This binary compiles/tests it offline,
certifies its root owner, and authenticates 14 cached package files against the
explicit `.crate` checksum. Changed archives, modified/extra/symlinked cached files,
missing archives, missing proof packages and omitted metadata packages all fail.
The poisoned-cache case also fails before a successful collection manifest exists.
Test-only manifest re-anchoring exercises semantic proof validation; no signature
is forged. Tests mutate and restore only the isolated workspace cache.

That fixture cache occupies 134,385 bytes
(15,935 archive, 51,103
extracted files, 9,826 index); its capture occupies
64,675 bytes. These are user dependency/test inputs, separate
from the plugin's zero-byte persistent cache and four-file release payload. Test
setup copies the already provisioned crate locally; zero network bytes were fetched.
Observed external `include!`/path inputs now block capture, as described below.
Arbitrary build-script reads and environment closure remain uncertified. Registry build scripts/proc macros, inactive lock packages, Git and
custom registries are rejected; this is a bounded package provenance implementation.

The [compiler-input record](stable-rust-candidate-evidence/compiler-inputs.json)
retains a real preceding-binary reproduction: an external `include_bytes!` file
changed after capture, yet verification returned exit 0. The new collector checks
observed stable dep-info against authenticated workspace/registry files. Three
real captures using external `include_bytes!`, `include!` and `#[path]` fail before
emitting a manifest. A project-internal data file is captured and verified.
Re-anchored empty proofs, missing inputs, Make expressions, wrong compiler cwd
and unmatched producer sets fail. Generated source bytes from the boundary fixture
are retained by hash while their owners remain unsupported. These add 15 command
checks, bringing that checkpoint to 74 total. Compiler wrapper records include actual rustc arguments,
working directory and exit status; they do not prove arbitrary build-script reads,
environment completeness or transitive process tracing.

The first dep-info implementation rejected the real registry's relative
`src/lib.rs` as ambiguous (`acceptance-17.log`). The public Cargo wrapper supplies
the actual producer cwd, resolving that failure. `acceptance-18.log` records a
negative test expecting a producer-set error after deleting its only record; the
empty-proof check correctly fired first. The revised test retains an unmatched
record and exercises the intended check. Package preparation then correctly
rejected a binary/acceptance digest mismatch (`package-07.log`); the final
preparation artifact was revalidated in `acceptance-22`, all four Core cases,
historical comparison and lifecycle checks. No acceptance anchor was bypassed.

Rust unit tests separately exercise nonzero exit, timeout/process-group cleanup,
missing executable, source decisions/unsupported owners, symlinks, tampering,
malformed LLVM shape/version and strict JSON duplicate-key rejection.
The execution-audit scanner has synthetic regression tests; those are logic tests,
not cross-host acceptance.

## Actual Core and migration checks

The same collector binary was used by all four real Core acceptance cases. The
plain/boundary cases are at
`target/gh-259/core-acceptance-{plain,boundaries,features}-15/`. Each invokes the
actual Core CLI with a signed v2 request, then runs Core's generic evidence
validation and requiredness evaluator. Plain produces two verified complexity
counts, 3 and 1, plus verified per-function execution ratios 1/1 and 0/1. Boundary fixtures produce unavailable evidence with unsupported
capabilities and no metrics. Required CRAP blocks in every case. The registry fixture at
`target/gh-259/core-acceptance-registry-15/` adds CC 1 and execution coverage 1/1.
It runs with the same isolated `CARGO_HOME` used for capture. The first attempt
with the host Cargo home correctly failed configuration identity verification
(`core-acceptance-registry-10.log`); no identity check was bypassed.

All four cases reject invalid signatures, replay, expiry, altered project/series,
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

The [lifecycle summary](stable-rust-candidate-evidence/lifecycle.json) records 38
executed checks. RSA-2048/SHA-256 signatures are real, generated by repository-only
OpenSSL automation using a test-only key. A retained [reproduction](stable-rust-candidate-evidence/lifecycle-reproduction.json)
shows the preceding binary accepting signed version `.2` while the program reports
`.1`. Authentication now precedes a bounded staged-program `--version` launch;
exact version matching and a repeated payload/trust check precede activation.

The [upgrade build](stable-rust-candidate-evidence/upgrade-build.json) independently
compiles the same implementation with test version `0.1.0-candidate.1.upgrade-test`
using stable Rust 1.97.1. Only the package and lockfile versions change. The two
program hashes differ: the installed first program performs the upgrade, and the
installed second program performs rollback. This validates executable switching,
not historical or production release compatibility. Its signed package occupies
3,126,449 bytes; the repository-only second build target occupies 178,705,200 bytes
and is excluded from runtime packages and downloads.

Checks cover unsigned-package rejection, ten re-signed invalid support documents,
both-signature requirements, malformed RSA, payload/inventory mixing,
extra assets, symlinks, duplicate JSON, missing/changed verifiers, nonzero verifier
exit, a real 60-second verifier timeout, wrong program versions, failed program
launches, program exit 23, a real 60-second program timeout, staged support-file
mutation, corrupt/nonexecutable rollback targets, concurrent
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
preparation (`package-10.log`).

## Commands and results

Commands ran from the assigned workspace. Existing Core code/check results remain
unchanged; candidate execution, packaging, Python suites and documentation were
validated again for this checkpoint. `CARGO_TARGET_DIR` was redirected into
this workspace because the default `/home/gem/cargo-target` is read-only.

| Command | Actual result and log under `target/gh-259/` |
| --- | --- |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/stable-target" RUSTFLAGS= CARGO_ENCODED_RUSTFLAGS= RUSTC_WRAPPER= RUSTC_WORKSPACE_WRAPPER= cargo +1.97.1 build --manifest-path tools/quality/rust-stable-collector/Cargo.toml --release --locked --offline` | Passed development build; final artifact rebuilt with preparation environment and revalidated below; `aggregate-build-01.log` |
| Same target, `cargo +1.97.1 test --manifest-path tools/quality/rust-stable-collector/Cargo.toml --locked --offline` | 17 passed; `aggregate-tests-01.log` |
| `cargo +1.97.1 fmt --manifest-path tools/quality/rust-stable-collector/Cargo.toml -- --check` | Passed; `aggregate-fmt-01.log` |
| Same target, `cargo +1.97.1 clippy --manifest-path tools/quality/rust-stable-collector/Cargo.toml --all-targets --locked --offline -- -D warnings` | Passed; `aggregate-clippy-01.log` |
| `python3 tools/quality/rust-stable-collector/validate_stable_candidate.py --binary target/gh-259/stable-target/release/harness-gate-rust-stable-collector --output target/gh-259/acceptance-22` | 85 passed; `acceptance-22.log` |
| `env -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u http_proxy -u https_proxy -u all_proxy CARGO_TARGET_DIR="$PWD/target/gh-259/core-target" cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | 397 passed; `core-nextest-02.log` |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Passed; `core-registry-fmt-01.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/core-target" cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Passed; `core-registry-clippy-01.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/quality-target" python3 -m unittest discover -s tools/quality/tests -v` | 444 tests OK, 36 skipped; `quality-tests-11.log` |
| `python3 -m unittest discover -s tools/release/tests -v` | 91 passed; `release-tests-12.log` |
| `python3 -m unittest discover -s tools/release/tests -p test_stable_collector_policy.py -v` | 2 passed after CI build environment alignment; `package-ci-policy-01.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/quality-target" python3 -m unittest discover -s tools/quality/tests -p test_ci_topology.py -v` | 7 passed after CI build environment alignment; `package-ci-topology-01.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/core-target" python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Passed; `docs-consistency-12.log` |
| Same Core target, `cargo build --manifest-path tools/harness-gate/Cargo.toml --example stable_collector_acceptance --locked --offline` | Passed; `core-registry-build-01.log` |
| `target/gh-259/core-target/debug/examples/stable_collector_acceptance target/gh-259/stable-target/release/harness-gate-rust-stable-collector target/gh-259/core-target/debug/harness-gate target/gh-259/acceptance-22 target/gh-259/core-acceptance-${fixture}-15 $fixture` for `plain boundaries features` | All three passed; matching `core-acceptance-${fixture}-15.log` |
| `CARGO_HOME="$PWD/target/gh-259/acceptance-22/registry-cargo-home" target/gh-259/core-target/debug/examples/stable_collector_acceptance target/gh-259/stable-target/release/harness-gate-rust-stable-collector target/gh-259/core-target/debug/harness-gate target/gh-259/acceptance-22 target/gh-259/core-acceptance-registry-15 registry` | Passed; `core-acceptance-registry-15.log` |
| `python3 tools/quality/rust-stable-collector/compare_historical_fixture.py --binary target/gh-259/stable-target/release/harness-gate-rust-stable-collector --output target/gh-259/historical-10` | Passed; `historical-10.log` |
| `python3 tools/quality/rust-stable-collector/validate_lifecycle.py --binary target/gh-259/stable-target/release/harness-gate-rust-stable-collector --package target/gh-259/package-10/unsigned-package --output target/gh-259/lifecycle-11` | 38 passed; real RSA, mocked Sigstore; `lifecycle-11.log` |
| `python3 tools/quality/rust-stable-collector/prepare_release.py --output target/gh-259/package-10 --target-dir "$PWD/target/gh-259/stable-target" --toolchain 1.97.1 --acceptance target/gh-259/acceptance-22/summary.json d672ef27b0966ac54b085143464907b1ce38def3e38f6f3b75c14286395f934d` | Passed locked offline build and package preparation; `package-10.log` and `package-10/preparation.json` |
| `harness-gate config check`; `harness-gate verify --profile ci --all` | Not applicable: no `.harness-gate/flow.toml` declaring `ci`; neither was run |

The registry suite first hit an offline test-setup failure while resolving the
collector's full graph (`acceptance-15.log`); setup now resolves only the fixture
graph and retains its command output. The complete 59-check rerun passed.

The owner-checkpoint release suite initially rejected the preceding checked-in
acceptance, which lacked the newly required owner checks (`release-tests-06.log`).
After refreshing the real 46-check evidence, the complete release suite passed.

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
- The Core candidate certifies lexical complexity and bounded function execution
  coverage for the declared limited owners. It does not certify line/region
  coverage or CRAP and does not replace existing required measurements. Production request signing and project integration remain open.
- Complete compiler input provenance, broader certified source/coverage owners, comprehensive
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

The aggregate-checkpoint lifecycle invocation first named a nonexistent package
subdirectory (`lifecycle-11-setup-failed.log`, FileNotFoundError). No lifecycle
operation ran. The corrected `unsigned-package` invocation passed all 38 checks.

The first docs-consistency invocation used the default Cargo target and failed.
A direct `cargo run --quiet --locked --manifest-path tools/harness-gate/Cargo.toml
-- --help` probe recorded `failed to open: /home/gem/cargo-target/debug/.cargo-build-lock`,
`Read-only file system (os error 30)` in `docs-default-target-probe.log`.
The documented workspace-local `CARGO_TARGET_DIR` invocation passed; the failed
report and log remain in `docs-consistency-12-default-target-failed.{json,log}`.
