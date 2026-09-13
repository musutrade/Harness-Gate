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

The candidate binary is **1,408,760 bytes**, SHA-256
`4baf3660d4d4bb0e1400c6f2e9a1ba2e52ac2a769607a33c90719b27bcbb645c`.
ELF `NEEDED` entries are `libgcc_s.so.1` and `libc.so.6`; no Python or compiler
private shared library appears. This observation alone is not a transitive
process audit. The plain capture occupies 42,063 bytes. Its temporary Cargo
build directory is removed and persistent candidate cache is zero bytes.
Signed package, installed footprint, upgrade/download bytes and rollback are
**not implemented/measured**, not zero. Build targets and acceptance logs are
repository development artifacts and are not proposed runtime payloads.

The checked-in [acceptance summary](stable-rust-candidate-evidence/acceptance.json)
contains actual tool hashes, binary identity, all 23 checks, raw totals and
capture/request anchors. [Environment probes](stable-rust-candidate-evidence/environment.json)
record exact commands, errors and outputs. [ELF dependencies](stable-rust-candidate-evidence/dynamic-libraries.txt)
record the binary inspection. The [log identity index](stable-rust-candidate-evidence/log-identities.json)
records hashes and sizes of retained workspace logs. These are new candidate observations, not repackaged
historical evidence or authenticated Core certificates. Full captures/logs remain
under `target/gh-259/acceptance-04/` in the execution workspace; the summary is
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
implemented failure boundaries; they do not prove signature/replay protection.
Rust unit tests separately exercise nonzero exit, timeout/process-group cleanup,
missing executable, source decisions/unsupported owners, symlinks and tampering.
The execution-audit scanner has synthetic regression tests; those are logic tests,
not cross-host acceptance.

## Commands and results

Commands ran from the assigned workspace. `CARGO_TARGET_DIR` was redirected into
this workspace because the default `/home/gem/cargo-target` is read-only.

| Command | Actual result and log under `target/gh-259/` |
| --- | --- |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/stable-target" cargo +1.97.1 build --manifest-path tools/quality/rust-stable-collector/Cargo.toml --release --locked --offline -vv` | Passed; `stable-build.log` |
| Same target, `cargo +1.97.1 test --manifest-path tools/quality/rust-stable-collector/Cargo.toml --locked --offline` | 7 passed; `stable-tests.log` |
| `cargo +1.97.1 fmt --manifest-path tools/quality/rust-stable-collector/Cargo.toml -- --check` | Passed; `stable-fmt.log` |
| Same target, `cargo +1.97.1 clippy --manifest-path tools/quality/rust-stable-collector/Cargo.toml --all-targets --locked --offline -- -D warnings` | Passed; `stable-clippy.log` |
| `python3 tools/quality/validate_stable_candidate.py --binary target/gh-259/stable-target/release/harness-gate-rust-stable-collector --output target/gh-259/acceptance-04` | 23 passed; `acceptance-04.log`; driver was subsequently moved into the crate directory without changing capture logic |
| `env -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u http_proxy -u https_proxy -u all_proxy CARGO_TARGET_DIR="$PWD/target/gh-259/core-target" cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | 397 passed; `core-nextest-no-proxy.log` |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Passed; `core-fmt.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/core-target" cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Passed; `core-clippy.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/quality-target" python3 -m unittest discover -s tools/quality/tests -v` | 444 tests OK, 36 skipped; `quality-tests.log` |
| `python3 -m unittest discover -s tools/release/tests -v` | 85 passed; `release-tests.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/core-target" python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Passed; `docs-consistency-final.log` |
| `harness-gate config check`; `harness-gate verify --profile ci --all` | Not applicable: no `.harness-gate/flow.toml` declaring `ci`; neither was run |

The initial Core nextest run hit a local webhook failure with inherited proxies;
removing proxy variables resolved it. Initial candidate probes exposed LLVM's
Rust-specific version suffix, cargo subcommand argument handling and export
schema 3.1.0; those were fixed before the recorded successful release runs.
The intermediate quality run found a missing draft documentation link and a
new development script in the production-module inventory directory. The link
was completed and the development driver moved into its crate directory.
Compiler-private test classes now require an explicit manual-experiment opt-in;
required CI retains all other existing checks and does not bootstrap that backend.

## Acceptance still blocked or unimplemented

- Rust 1.98.1 is not installed. Fetching the official stable manifest failed
  `curl: (56) Proxy CONNECT aborted`. The installed 1.97.1 was actually executed;
  release status and interfaces for the second candidate remain unverified.
  Doctor's candidate allowlist is not evidence of support for both versions.
- A second runnable system was unavailable: Docker socket access was denied.
  Only the environment above has been exercised; there is no released support
  platform yet and no claim covering all Linux systems.
- `strace` is installed but the sandbox rejects `PTRACE_TRACEME` and
  `PTRACE_SEIZE`. Local command records, build logs and ELF inspection passed;
  a complete transitive `execve` audit did not run. Required CI now requests real
  release-build and runtime traces and fails if tracing fails; CI is pending.
- Core v2 signature, nonce, expiry, capability validation and authenticated
  normalized response consumption are not implemented. Integrity manifests do
  not substitute for those checks. No required measurement can use this output.
- General dependency projects, certified source/coverage owners, malformed-format
  adversarial coverage, complete negative matrices and the same-fixture historical
  comparison still need implementation/acceptance. No MIR backend was rerun and
  original historical anchors remain untouched.
- Rust signed installation, upgrade interruption, rollback failure and independent
  download accounting are not implemented. No production signing was simulated
  or claimed; no user installation was changed. The release hold remains.

GH-259 must remain open after this checkpoint. T8 and production publication
cannot proceed on this evidence.
