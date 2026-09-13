# Stable Rust collector: implementation contract and candidate

GH-259 is **not complete**. The executable in
[`tools/quality/rust-stable-collector`](../../tools/quality/rust-stable-collector)
performs real stable coverage capture and Rust syntax analysis. It is a developer
candidate, not an authenticated Core adapter, supported release or installer.
Its successful capture reports `state: unsupported`, `core_acceptance: pending`.
Do not configure it as a replacement for required measurements.

This implements the direction read from
[PR 260](https://github.com/musutrade/Harness-Gate/pull/260), commit
`65a0787d24096c0122edf34ad50f09a0eaa269d9`. The policy, ADR 0053 and legacy
publication hold are included here because that policy branch was not merged into
this checkout. [Local evidence and remaining acceptance](stable-rust-collector-validation.md)
distinguish actual results from planned work.

## T2 inventory and replacement plan

The existing [delivery contract](rust-collector-delivery-contract.md) remains the
reference for accepted boundaries and Core authority. The implementation inventory is:

| Existing implementation | Responsibility | Replacement decision |
| --- | --- | --- |
| `rust_collector_entry.py`, `rust_collector_project.py` | Entry, doctor, Cargo orchestration, authenticated request bindings, normalized evidence and exits | Rust `main`, `process`, `tools`, `collect`; protocol authentication/normalization still T4 |
| `rust_collector_contract.py`, `collector_runner.py` | Development protocol and typed outcomes | Preserve semantics; do not confuse the development envelope with released Core v2 |
| `rust_native_driver.py`, `rust-native-driver/`, `rust_native_classify.py` | Compiler-private inventory, native counter capture, source ownership/classification | Historical/manual experiments only; no required build or runtime dependency |
| `rust_native.py`, `rust-measure/src/native.rs` | Unstable MIR text capture/parsing and replay | No reuse in the candidate |
| `rust-measure/` source analyzer | Stable syntax parsing plus broader metric aggregation | Reuse the parser approach only; new analyzer has an explicit lexical series and unsupported owners |
| `rust-collector-runtime/`, `build_rust_collector.py`, `rust_collector_delivery.py` | Python/shell launchers, compiler/C/Python environment assembly, exact host checks | Replace installed implementation with one Rust executable; no bundled toolchain or publisher kernel fingerprint |
| `tools/release/install_collector.py`, `friendly_collector_install.py`, `collector_light_install.py` | Verification, shared component store, install/update/select/rollback | Port user lifecycle to Rust; retain protected release verification principles, not Python runtime calls |
| `tools/release/*collector*`, `rust-collector-release.yml` | Signing packets, RSA/Sigstore verification, publication policy and rehearsal | Repository automation may remain Python; hold remains until the replacement contract and T8 acceptance |
| Core `process/adapter.rs`, `config/quality/collectors.rs`, quality evidence/policy modules | Signature, replay protection, generic envelope, series/claims, thresholds and final decisions | Existing Core remains authoritative; no policy or baseline changes in this stage |

The final installed program is planned to remain one executable,
`harness-gate-rust-stable-collector`, containing entry, doctor, capture, source
analysis, normalization, verification, install, upgrade and rollback modules.
No additional implementation daemon, private compiler helper, shell or Python
launcher is planned. A target payload needs this executable, licenses/notices,
an adapter descriptor, support/series metadata and authenticated release inventory.
Acceptance archives and external compiler archives are not runtime payloads.
The current crate is a standalone workspace, release-stripped with LTO, MSRV 1.97.1.
No artifact from it is currently approved for installation.

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

### Identity and Core protocol work remaining (T4)

The candidate request records canonical project/output roots, hashes of source
files and lockfiles, effective Cargo configuration files, exact tool bytes/version
output, explicit features and a bounded per-command timeout. It rejects symlinks,
registry/git/outside-workspace dependencies, outside-workspace target roots and
unreviewed Cargo configuration. Only `build.target-dir` configuration is currently
accepted and overridden with a temporary capture directory. Compiler flags and
wrapper injection are rejected. The sanitized environment, commands, output,
exit status and duration are retained. Sources/configuration/tools are rechecked
after collection. Output must be fresh. Failed capture produces no manifest.

These hashes establish local integrity against externally supplied anchors, not
trust, expiry, complete dependency provenance or adversarial build isolation.
Build scripts are target project code and are not sandboxed by this candidate.
Generated files remain raw coverage inputs with unsupported source ownership.
Root-level test/example/bench paths and build.rs are omitted from lexical analysis;
workspace-wide production classification is still pending. No secret signing key
or credentials belong in the capture environment.

The replacement adapter must consume Core protocol version 2 with result schema
`1`: adapter declaration/signature, invocation/step identity, timeout, configuration
digest, artifact root, nonce/time window, args/environment/capabilities and input.
Core verifies Ed25519 signatures over its canonical request, executable digest,
capabilities and replay state. The collector must preserve the corresponding
bindings when reading `harness-project-collector-request/v1` (project, collector,
context, roots, selection and subject/capability/series bindings).

The generic response requires schema version `1`, matching invocation, transport
`status: PASS`, retained artifacts and a
`harness-project-collector-response/v1` collection. This transport status cannot
mean quality acceptance. Rust normalization must validate every claimed source,
configuration, file, tool, owner and series; unknown mappings produce unsupported
or measurement errors. It must preserve `supported`, `unsupported`, `not_configured`, `not_collected`,
`not_applicable` and `measurement_error`, and propagate tool failures without usable partial measurements. The current candidate
implements none of this envelope and cannot be accepted by Core as evidence.

### Source/coverage boundaries

`rust-source-decisions/v1-candidate` is lexical syntax, not MIR complexity. Each
ordinary function starts at one. Add one for if, while, for, loop, `?`, each `&&`
or `||`, let-else, each match guard, and match arm count minus one (minimum zero).
Spans are one-based start/end line and column, with an exclusive end. The analyzer
does not claim compilation, reachability or expansion of every parsed source.

| Construct | Current real behavior | Certification boundary |
| --- | --- | --- |
| Ordinary function, including never-called | Lexical complexity supported; raw LLVM may report count zero | No function coverage/CRAP join |
| Direct `#[test]` or `#[cfg(test)]` function/module | Excluded from lexical analysis | Raw coverage still includes tests; production coverage unsupported |
| Macro/derive/include-generated code | Expansion/attribute recorded as unsupported | No generated owner inferred from parent/file totals |
| Async/closure/nested function | Affected lexical function is unsupported with null complexity | Constructor, future body and closure owners are never merged |
| Generic function, generic/trait impl, trait default | Unsupported owner with null complexity | No instantiation or unused-generic denominator claim |
| cfg/features and other unresolved attributes | Cargo features are recorded/executed; affected lexical owner unsupported | Parsed inactive source is not called active production code |
| External module declaration | Activation/ownership unsupported | No implicit module-to-coverage certification |
| Build script output | Build script runs; no instrumentation of the host build script | Generated paths/raw export retained; production owner unsupported |
| Parse/tool/identity/format error | `measurement_error`, nonzero process exit | No successful evidence manifest |

All function CRAP is currently unsupported, including simple functions. Raw LLVM
JSON `3.1.0` is preserved under `rust-llvm-source-coverage/v1-candidate`; schema
checks do not certify owners. No branch denominator is synthesized when standard
instrumentation exports zero branch regions.

### Independent lifecycle and migration (T5–T8)

The Rust lifecycle must verify the approved release inventory, signature identity,
integrity and provenance before unpacking/selecting a target. Protected signing
permissions, pinned trust roots and no unsigned fallback remain requirements.
Install into a fresh version directory, sync verified content, then atomically
select it; interrupted upgrades leave the previous selection usable. Reverify
rollback targets before switching, preserve a working selection on failure, and
test tampering, interrupted extraction/selection and rollback failure. These
operations do not yet exist in this candidate.

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
paths/anchors are unchanged. No old backend ran for this stage. There is not yet
a same-fixture authenticated old/new comparison; the semantic table above is not
T6 numerical acceptance. T8 cannot remove the publication hold until Core,
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
