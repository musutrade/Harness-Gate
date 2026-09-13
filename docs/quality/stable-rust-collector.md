# Stable Rust collector: implementation contract and candidate

GH-259 is **not complete**. The executable in
[`tools/quality/rust-stable-collector`](../../tools/quality/rust-stable-collector)
performs real stable coverage capture and Rust syntax analysis. It is a developer
candidate with a Core v2 adapter for verified lexical complexity and bounded function execution coverage and an offline
Rust installation transaction. It is not a supported release. Capture alone reports `state: unsupported`,
`core_acceptance: pending`; the separate authenticated conversion is described below.
Do not configure it as a replacement for required measurements.

This implements the direction read from
[PR 260](https://github.com/musutrade/Harness-Gate/pull/260), commit
`c2f7c14fbee0245e3a48176f7a768f6d3fd1a041`. The policy, ADR 0053 and legacy
publication hold are included here because that policy branch was not merged into
this checkout. [Local evidence and remaining acceptance](stable-rust-collector-validation.md)
distinguish actual results from planned work.

## T2 inventory and replacement plan

The existing [delivery contract](rust-collector-delivery-contract.md) remains the
reference for accepted boundaries and Core authority. The implementation inventory is:

| Existing implementation | Responsibility | Replacement decision |
| --- | --- | --- |
| `rust_collector_entry.py`, `rust_collector_project.py` | Entry, doctor, Cargo orchestration, authenticated request bindings, normalized evidence and exits | Rust `main`, `process`, `tools`, `collect`, `adapter`; Core authenticates requests and Rust normalizes verified lexical owners |
| `rust_collector_contract.py`, `collector_runner.py` | Development protocol and typed outcomes | Preserve semantics; do not confuse the development envelope with released Core v2 |
| `rust_native_driver.py`, `rust-native-driver/`, `rust_native_classify.py` | Compiler-private inventory, native counter capture, source ownership/classification | Historical/manual experiments only; no required build or runtime dependency |
| `rust_native.py`, `rust-measure/src/native.rs` | Unstable MIR text capture/parsing and replay | No reuse in the candidate |
| `rust-measure/` source analyzer | Stable syntax parsing plus broader metric aggregation | Reuse the parser approach only; new analyzer has an explicit lexical series and unsupported owners |
| `rust-collector-runtime/`, `build_rust_collector.py`, `rust_collector_delivery.py` | Python/shell launchers, compiler/C/Python environment assembly, exact host checks | Replace installed implementation with one Rust executable; no bundled toolchain or publisher kernel fingerprint |
| `tools/release/install_collector.py`, `friendly_collector_install.py`, `collector_light_install.py` | Verification, shared component store, install/update/select/rollback | Rust `release` implements offline verification/install/upgrade/rollback; unsigned preparation is implemented; downloader and protected signing remain pending |
| `tools/release/*collector*`, `rust-collector-release.yml` | Signing packets, RSA/Sigstore verification, publication policy and rehearsal | Repository automation may remain Python; hold remains until the replacement contract and T8 acceptance |
| Core `process/adapter.rs`, `config/quality/collectors.rs`, quality evidence/policy modules | Signature, replay protection, generic envelope, series/claims, thresholds and final decisions | Existing Core remains authoritative; no policy or baseline changes in this stage |

The final installed program is planned to remain one executable,
`harness-gate-rust-stable-collector`, containing entry, doctor, capture, source
analysis, normalization, verification, install, upgrade and rollback modules.
No additional implementation daemon, private compiler helper, shell or Python
launcher is planned. A target payload needs this executable, licenses/notices,
support/series metadata and authenticated release inventory/signature. The adapter
binding is project-specific and is not a bundled user configuration.
Acceptance archives and external compiler archives are not runtime payloads.
The current crate is a standalone workspace, release-stripped with LTO, MSRV 1.97.1.
No artifact from it is currently approved for installation.

The executable also statically links our ordinary Rust shared generator for the
two identified macro templates. [Macro source observations](stable-rust-macro-observation.md)
bind source/version, invocation, requested configuration and generated owners.
They remain separate diagnostic facts: generated-function execution coverage and
CRAP are unsupported and cannot replace authenticated Core evidence.

### External dependencies and selection

Users provide their project's supported stable Rust (Cargo, rustc, rustdoc), a
system linker/C runtime, cargo-llvm-cov 0.9.0 and matching `llvm-cov`/`llvm-profdata`.
The current candidate finds LLVM in the selected Rust sysroot and checks its
reported LLVM version. `llvm-tools-preview` is the distribution component name;
the instrumentation interface is stable `-C instrument-coverage`. No private
compiler component is used. CLI and export-format compatibility still require
validation on each proposed Rust version.

Doctor runs the Rust proxy in the project directory, preserving project pins and
explicit `RUSTUP_TOOLCHAIN`. It sets `RUSTUP_AUTO_INSTALL=0`; no default toolchain
or system package changes occur. After selection it records and invokes the exact
sysroot executables. Cargo runs locked and offline. LLVM version mismatch and
missing tools fail with explicit installation hints. For an already reviewed
project toolchain, the user can deliberately run:

```sh
rustup toolchain install 1.97.1 --profile minimal
rustup component add llvm-tools-preview --toolchain 1.97.1
cargo +1.97.1 install cargo-llvm-cov --version 0.9.0 --locked
```

These commands install dependencies; the plugin does not execute them. Linux
users also need their distribution's native linking tools (for example an
explicit `sudo apt-get install build-essential` on Ubuntu). Future authentication
and lifecycle code must implement verification in Rust or document stable public
verification tools; it must not invoke existing Python verifier modules. Their
dependency requirements are not yet finalized or accepted.

### Identity and Core protocol (T4, partial)

The candidate request records canonical project/output roots, hashes of source
files and lockfiles, effective Cargo configuration files, exact tool bytes/version
output, explicit features and a bounded per-command timeout. It rejects symlinks,
git/custom-registry/outside-workspace dependencies, outside-workspace target roots and
unreviewed Cargo configuration. Only `build.target-dir` configuration is currently
accepted and overridden with a temporary capture directory. Compiler flags and
wrapper injection are rejected. The sanitized environment, commands, output,
exit status and duration are retained. Sources/configuration/tools are rechecked
after collection. Output must be fresh. Failed capture produces no manifest.

For a locked crates.io dependency, explicitly provide the original `.crate`
archive via `prepare PROJECT OUTPUT DOCTOR --registry-archives ARCHIVES.json`.
The JSON object maps each dependency's `Cargo.lock` SHA-256 checksum to its
canonical absolute archive path, for example:

```json
{"<checksum from Cargo.lock>": "/absolute/user-provisioned/itoa-1.0.18.crate"}
```

Obtain the exact locked archive from the registry's download interface when
provisioning dependencies; retain it for later evidence verification. The plugin
does not download archives or infer Cargo's private cache layout. Cargo must
already be able to build the project offline. Using Cargo's public metadata
format 1, the Rust collector binds each package to lock format 4, verifies archive
SHA-256, streams gzip/tar without extraction, and compares every file in Cargo's extracted package directory with the archive. Only Cargo's two generated cache markers
are omitted from that equality; symlinks, special files, unsafe/duplicate archive
members, extra source files and oversized inputs are rejected. The limits are
16 MiB per file and 128 MiB decompressed archive. `dependencies.json` is anchored
in the capture manifest, and the proof is recomputed before/after compilation
and during verification. Archive paths and source files must remain available.

This bounded candidate requires an exact lock/metadata package inventory and
rejects inactive lock entries, unused archive inputs, registry build scripts and
proc macros. Git, custom registries and external path dependencies remain outside
its support boundary. Source replacement configuration remains rejected. The real
registry fixture exercises one `itoa` dependency, not every dependency graph.
The same Rust executable also serves as Cargo's public `RUSTC_WRAPPER`. It
forwards compiler arguments unchanged and records the pinned compiler, working
directory, arguments and exit status. Stable Makefile dep-info records must match
those observed producers exactly; relative input paths resolve from the recorded
working directory, including registry packages with the same source filenames.
Every observed file must match the authenticated workspace or registry inventory.
External `include!`, `include_bytes!` and `#[path]` inputs therefore block collection
before a manifest exists. Files generated under the fresh build directory are
retained by content hash; they do not acquire certified source owners.

The candidate retains raw dep-info and compiler invocation records in the capture
and rechecks their identities during verification. It accepts only the exercised
Linux dep-info escaping and public compiler output arguments; unknown forms fail
closed. This follows the documented [Cargo wrapper protocol](https://doc.rust-lang.org/cargo/reference/config.html#buildrustc-wrapper)
and [rustc dep-info output](https://doc.rust-lang.org/rustc/command-line-arguments.html).
It does not parse Cargo fingerprint internals. The internal build layout is not a
compatibility promise; every observed `.d` must have an authenticated producer.

This proves the observed compiler file-input inventory, not a complete build-input
closure. Arbitrary build-script reads, proc-macro execution, environment closure
and direct compiler invocations bypassing Cargo remain uncertified. Environment
dependency comments are retained verbatim without a completeness claim.
Pre/post checks detect observed changes; they do not provide a filesystem snapshot
or defend against a concurrent actor changing and restoring files during a build.

These hashes establish local integrity against externally supplied anchors, not
trust, expiry, complete dependency provenance or adversarial build isolation.
Build scripts are target project code and are not sandboxed by this candidate.
Generated files remain raw coverage inputs with unsupported source ownership.
Root-level test/example/bench paths and build.rs are omitted from lexical analysis;
workspace-wide production classification is still pending. No secret signing key
or credentials belong in the capture environment.

The candidate adapter consumes Core protocol version 2 with result schema
`1`: adapter declaration/signature, invocation/step identity, timeout, configuration
digest, artifact root, nonce/time window, args/environment/capabilities and input.
Core verifies Ed25519 signatures over its canonical request, executable digest,
capabilities and replay state. The collector preserves the corresponding
bindings when reading `harness-project-collector-request/v1` (project, collector,
context, roots, selection and subject/capability/series bindings).

The generic response requires schema version `1`, matching invocation, transport
`status: PASS`, retained artifacts and a
`harness-project-collector-response/v1` collection. This transport status cannot
mean quality acceptance. `describe` verifies the anchored capture, recomputes source
analysis, and produces a Core series and lexical owner inventory. The series binds
collector bytes, exact tool identities, rule, runtime, target and metric contracts.
The `adapter --binding FILE --binding-sha256 SHA256` arguments are signed by Core's
caller. The pinned `rust-stable-core-binding/v1` file binds that description, the
project, capture anchors, complete input, configuration digest and invocation.
No trust key comes from the capture. Core authenticates before spawning the adapter;
direct execution by itself does not authenticate a request.

The Rust adapter rejects duplicate JSON keys, changed bindings, mismatched executable,
source, context target, workspace, series, claims and ambiguous/missing owners. It
requires all five metric contracts for each selected function. Verified ordinary
lexical owners can emit `complexity.cyclomatic`. Eligible owners also emit
`coverage.function` as executed functions / functions (1/1 or 0/1 for one owner).
`coverage.region` counts nonzero LLVM code regions / all code regions for that same
certified owner. Line coverage and CRAP explicitly emit `unsupported`, with no fabricated values. A file containing
uncertified activation/expansion also makes its complexity unavailable. Per-owner
artifacts bind source facts, context, series and capture anchors. Input identities
are rechecked after conversion; failure returns nonzero and no successful envelope.

The repository's Rust `stable_collector_acceptance` example exercises the actual
Core CLI signature/nonce/expiry transport, then Core's evidence validation and
requiredness evaluation. Plain complexity and function execution coverage are accepted as the new series; required
CRAP remains blocked. This uses a clearly named test-only signing key, not protected
production signing. Complete compiler input provenance, broader coverage normalization
and full T4 acceptance remain open.

### Source/coverage boundaries

`rust-source-decisions/v1-candidate` is lexical syntax, not MIR complexity. Each
ordinary function starts at one. Add one for if, while, for, loop, `?`, each `&&`
or `||`, let-else, each match guard, and match arm count minus one (minimum zero).
Spans are one-based start/end line and column, with an exclusive end. The analyzer
does not claim compilation, reachability or expansion of every parsed source.

| Construct | Current real behavior | Certification boundary |
| --- | --- | --- |
| Unannotated root or inline-module free function, including never-called | Lexical complexity, function execution and code-region ratios | ASCII, single-file LLVM owner; CRAP model and migration remain unaccepted |
| Direct `#[test]` or `#[cfg(test)]` function/module | Explicit source spans excluded from certified function owners | Raw file totals still include tests; normalized production owners exclude their regions |
| Macro/derive/include-generated code | Expansion/attribute recorded as unsupported | No generated owner inferred from parent/file totals |
| Async/closure/nested function | Affected lexical function is unsupported with null complexity | Constructor, future body and closure owners are never merged |
| Generic function, `impl Trait`, generic/trait impl, trait default | Unsupported owner with null complexity | No instantiation or unused-generic denominator claim |
| cfg/features and other unresolved attributes | Cargo features are recorded/executed; affected lexical owner unsupported | Parsed inactive source is not called active production code |
| External module declaration | Activation/ownership unsupported | No implicit module-to-coverage certification |
| Build script output | Build script runs; no instrumentation of the host build script | Generated paths/raw export retained; production owner unsupported |
| Parse/tool/identity/format error | `measurement_error`, nonzero process exit | No successful evidence manifest |

Function ownership uses `rust-llvm-exact-free-owner/v3-candidate`: every function
in an eligible source file must be an unannotated, nongeneric free function at
the root or inside unannotated inline modules, with no uncertified syntax.
Qualified names distinguish nested modules and repeated function names; matching
still uses exact source spans. Non-ASCII files, external or annotated modules,
impl/trait methods and annotated functions remain unsupported for this mapping. LLVM records must name exactly that source
file and contain only code regions. Their exact minimum start / maximum exclusive
end must uniquely equal one source function span or lie wholly inside one explicit
test exclusion. Each source owner requires exactly one LLVM record; symbols must
be unique. The first region must start at the owner boundary and agree with the
execution count. A zero execution count cannot accompany executed regions. Missing,
duplicate, cross-file or ambiguous owners fail with `measurement_error`; no missing
function is assigned a zero. Collection and verification recompute source analysis
from pinned files, including its complete file set. Symbol suffixes are never used
as the mapping algorithm. This narrow contract was actually exercised only with
the recorded Rust 1.97.1 fixtures; broader syntax/toolchain certification remains open.

Within that exact owner, region coverage counts each distinct LLVM code-region
span once and counts it covered only when its own counter is nonzero. Duplicate
spans fail measurement. All owner regions, including explicitly excluded test
owners, must reconcile with the file's region count and covered summary. This
narrow rule follows LLVM 22.1.6's
[function region statistics](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.6/llvm/tools/llvm-cov/CoverageSummaryInfo.cpp).
The real partial fixture distinguishes function execution 1/1 from region
coverage 5/6; an exported never-called function is 0/1 and 0/3 respectively.
The inline-module fixture separately verifies `left::classify` at 4/5 regions
and one execution, `right::classify` at 5/5 and two executions, and
`left::nested::never_called` at 0/3 and zero executions. Annotated/cfg module
variants compile and run but remain unsupported; missing or duplicate owners
and inherited positive counts fail measurement. The v3 rule broadens owner
placement without changing the metric definitions or authorizing baseline adoption.
The binary-bound Core normalization identity changes with this executable; it
therefore produces a different full measurement-series identity.
This is standard LLVM region coverage, not MIR basic-block coverage, branch
coverage, or a claim about arbitrary segment/expansion semantics.

All function CRAP is currently unsupported, including simple functions. CRAP model selection
and measurement migration require review; the candidate does not derive CRAP
or enable an existing required binding from these new ratios. Raw LLVM
JSON `3.1.0` is preserved under `rust-llvm-source-coverage/v1-candidate`; schema
checks do not certify owners. No branch denominator is synthesized when standard
instrumentation exports zero branch regions.

The raw validator follows the public [LLVM 22.1.6 JSON exporter](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.6/llvm/tools/llvm-cov/CoverageExporterJson.cpp).
The exercised invocation requires one nonempty export object, rejects duplicate
JSON keys, and validates signed-64 execution-count bounds, region coordinates and
filename IDs, ordered segments and summary arithmetic. Finite floating-point
percentages are accepted only in raw coverage parsing; Core and release documents
retain integer-only strict parsing. Unknown formats/region kinds and MC/DC fail
with `measurement_error` and an unsupported-format explanation. A valid raw capture
can still declare a metric capability `unsupported`; malformed data never produces
a successful capture. Branch/expansion layout checks do not establish a verified
producer capability. Cross-file aggregate reconciliation, counter-to-summary
reconstruction and certified ownership remain pending T4 work.

### Independent lifecycle and migration (T5–T8)

The Rust `release` module now verifies local flat bundles and atomically selects
installed versions. Its [offline lifecycle contract](stable-rust-collector-lifecycle.md)
defines the five assets, externally pinned trust and commands. Both in-process
RSA and external Sigstore verification must succeed; no unsigned fallback exists.
A per-root lock serializes transactions. Verified files are synced before the
`current` symlink is atomically replaced; rollback re-verifies its target.
The real fixture demonstrates RSA verification, upgrade/rollback, corruption and
interruption recovery. Sigstore invocation is mocked in this fixture, explicitly
not production signature acceptance. The downloader, trusted bootstrap,
protected signing, production license review and real Sigstore acceptance remain
open. Interrupted staging has no automatic garbage collection yet.

Collector version, external Rust/LLVM selection and Core version are independent.
An accepted collector update should download only its changed program/metadata;
an external tool update should not download a collector environment. Neither
operation automatically changes series bindings, thresholds, requiredness, debt,
ratchets or baselines. New source/coverage series need explicit migration review.

| Measurement | Historical series | Candidate series/difference |
| --- | --- | --- |
| Complexity | `rust-native-production-mir-block/1`, `typed-normal-mir-cfg-common-exit/1` | `rust-source-decisions/v1-candidate`; syntactic decisions with unsupported expansion, not compiled CFG |
| Coverage | `independent-mir-basic-block-counter/1` with certified production classification | `rust-llvm-source-coverage/v1-candidate`; ordinary source regions/lines, test-inclusive |
| Function risk | Native same-owner complexity/counters and accepted CRAP rules | Unsupported for all owners; no replacement required result |
| Existing standard coverage history | `llvm-file-summary-unfiltered/1` | Remains its own historical series; candidate is not a relabelled equivalent |

Historical `docs/quality/rust-native-inventory-replay.json` and associated original
paths/anchors are unchanged. No old backend ran for this stage. The
[same-source historical probe](stable-rust-collector-migration.md) records actual
new capture against an original GH-220 fixture and its anchored archived report;
it does not recertify the historical backend or establish equivalence. T8 cannot
remove the publication hold until full Core,
cross-toolchain/system, signed lifecycle and migration acceptance are complete.

## Running the developer candidate

After explicitly provisioning dependencies, run from this repository. Outputs
must not be inside the measured fixture. Choose new output directories each time.

```sh
CARGO_TARGET_DIR="$PWD/target/stable-build" cargo +1.97.1 build \
  --manifest-path tools/quality/rust-stable-collector/Cargo.toml --release --locked
mkdir -p target/stable-review
target/stable-build/release/harness-gate-rust-stable-collector doctor \
  tools/quality/fixtures/rust-stable/plain target/stable-review/doctor
target/stable-build/release/harness-gate-rust-stable-collector prepare \
  tools/quality/fixtures/rust-stable/plain target/stable-review/capture \
  target/stable-review/doctor/doctor.json > target/stable-review/request.json
target/stable-build/release/harness-gate-rust-stable-collector collect \
  target/stable-review/request.json
```

Use the printed manifest and request hashes with
`verify ABSOLUTE_CAPTURE_PATH MANIFEST_SHA256 REQUEST_SHA256` for integrity checking.
The repository-only `tools/quality/rust-stable-collector/validate_stable_candidate.py`
runs the real positive/negative
fixture suite. Python is used by this development automation, never by the binary.
Required CI adds a fresh release build and positive runtime `execve` traces, checks
those traces for forbidden subprocesses/flags, and uploads evidence. Existing
Core jobs and required aggregate names remain unchanged. Compiler-private native
test classes now require `HARNESS_GATE_LEGACY_EXPERIMENT=1` explicitly; their old
host/tool/archive requirements still apply in manual experiments.

### Authenticated candidate and historical acceptance

The following development commands use fresh output directories and the actual Core
executable. The example creates a test-only key; users must not adopt that key.

```sh
CARGO_TARGET_DIR="$PWD/target/stable-build" cargo +1.97.1 build \
  --manifest-path tools/harness-gate/Cargo.toml --locked \
  --bin harness-gate --example stable_collector_acceptance
python3 tools/quality/rust-stable-collector/validate_stable_candidate.py \
  --binary target/stable-build/release/harness-gate-rust-stable-collector \
  --output target/stable-review/acceptance
for fixture in plain boundaries features; do
  target/stable-build/debug/examples/stable_collector_acceptance \
    target/stable-build/release/harness-gate-rust-stable-collector \
    target/stable-build/debug/harness-gate target/stable-review/acceptance \
    "target/stable-review/core-${fixture}" "$fixture"
done
python3 tools/quality/rust-stable-collector/compare_historical_fixture.py \
  --binary target/stable-build/release/harness-gate-rust-stable-collector \
  --output target/stable-review/historical
```

These checks do not install a collector, replace project configuration or accept a
baseline. Required CI traces the Core/example/adapter processes as well as capture.
