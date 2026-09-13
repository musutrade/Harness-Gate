# ADR 0053: Stable interfaces and a pure-Rust collector

Status: Accepted direction; replacement implementation and release acceptance pending.

## Context

The old Rust collector links compiler-private libraries using instability escape
hatches and carries Python orchestration plus private compiler/runtime components.
This couples upgrades and CI to compiler internals and makes delivery unnecessarily
large. The user explicitly requires stable interfaces, a fully Rust plugin, external
user-installed dependencies and a durable engineering convention.

## Decision

Engineering Policy section 10 is the normative delta. Required CI and official
plugins must use stable compiler interfaces. Our installed Rust collector must be
entirely Rust, with no Python runtime. Publish our precompiled code by tested target;
document/check external dependencies without automatically changing users' systems.

Use stable coverage instrumentation plus source analysis, validate a new measurement
series and preserve Core authority, thresholds, historical evidence and baseline rules.
Never claim source metrics reproduce MIR counters or silently drop required assurance.

Suspend new legacy collector/installer signing and publication through the protected
workflow while the replacement is implemented. Keep independent Core release, existing
stable gates, legacy read-only verification and explicitly isolated experiments.
Removing the hold requires reviewed replacement code and real acceptance; changing
only the old plugin's host check is insufficient.

## Consequences and validation

The legacy workflow rejects mutation requests before environment approval. Repository
contracts guard this hold and direct unstable dependencies in required CI. The pure-Rust
implementation task must add transitive build/runtime checks, supported-toolchain tests,
clean-host install/update tests and measurement transition evidence. User-facing artifacts
must not contain a Python interpreter or depend on executing Python code.

This ADR records the decision and migration boundary; it does not claim the replacement
has been implemented, certified or published.

A runnable stable Rust capture/source-analysis candidate now exercises the
[implementation contract](../quality/stable-rust-collector.md). Its
[actual validation](../quality/stable-rust-collector-validation.md) is partial:
real Core request authentication and evidence validation now accept verified
lexical complexity plus bounded function execution and code-region coverage in a separate
candidate series. Line coverage and CRAP remain unsupported;
Rust offline signed lifecycle transactions now have real RSA and interruption tests;
real Sigstore verification, protected release preparation and the cross-toolchain/system
acceptance matrix remain pending.
The same-source historical comparison preserves the original report anchors.
This does not change the publication hold or accepted series.

The next T5 checkpoint adds a Rust-validated support schema and locked offline
unsigned candidate preparation with archive/source-authenticated dependency
notices. Real lifecycle checks consume that payload and reject unsigned packages
and re-signed overstated support metadata. This is review preparation only:
protected production signing, trust bootstrap, downloader, license review and
multi-toolchain/system acceptance remain open. T5 and T8 stay unchecked.

Activation now requires a bounded successful launch of the authenticated staged
program and an exact signed-version response, after both signatures pass. Payload
and trust identities are rechecked before selection changes. A separate stable
test-version build exercises executable upgrade and rollback; this is local
compatibility within this implementation, not historical or production acceptance.
Signature failures must never reach the launch check. T5/T8 and the release hold
remain unchanged until the remaining acceptance and release trust work is reviewed.

The T4 raw-evidence checkpoint validates the exercised public LLVM JSON structure,
integer domains, region IDs, segments and summary arithmetic with duplicate-key
rejection. Thirteen mutations of real exports now fail after test-only manifest
re-anchoring, and package preparation requires those checks. This remains a raw
format contract: complete counter reconciliation, broad certified source owners and
normalized coverage/CRAP are not complete. T4, T6–T8 remain unchecked; required
metrics, migration review and the release hold are unchanged.

The earlier bounded T4 owner checkpoint added `rust-llvm-exact-root-owner/v1-candidate`.
ASCII source files containing unannotated, nongeneric root functions can join an
exact unique LLVM region envelope to a source span; explicit test spans are
excluded. Core accepts the resulting per-function execution ratio (1/1 or 0/1),
not an intra-function coverage fraction. Missing/duplicate/cross-file owners and
inconsistent entry counts are measurement errors; unsupported syntax and `impl
Trait` never acquire coverage. At that checkpoint line/region coverage and all CRAP
were unsupported; the v2 code-region checkpoint below supersedes the region limit.
Actual owner mutations and authenticated Core checks are recorded in the validation
record. T3–T8, migration review and the legacy release hold remain open.

The bounded registry checkpoint extends T3/T4 with explicit user-provided `.crate`
archive paths keyed by Cargo.lock SHA-256. Pure Rust streaming gzip/tar validation
compares every cached dependency source file with the authenticated archive and
rechecks the manifest-anchored dependency proof at collection and verification.
Cargo metadata format 1 and lock format 4 must describe exactly the same packages;
crates.io normal libraries are eligible, while registry build scripts/proc macros,
inactive lock entries, custom registries, Git and external path dependencies remain
unsupported. Runtime code does not infer Cargo's private cache layout or download
inputs. The isolated real itoa fixture and cache/archive/proof mutations extend
acceptance without changing Core thresholds, requiredness or migration policy.
T3–T8 remain unchecked pending the complete matrix and release contract.

The bounded compiler-input checkpoint extends T3/T4 with the same Rust executable
acting through Cargo's public RUSTC_WRAPPER protocol. It records unchanged compiler
arguments, pinned rustc, actual working directory and exit status, then reconciles
stable Makefile dep-info with workspace/registry source identities. External
includes and path modules fail before evidence publication; generated input bytes
are retained without certifying generated owners. Verification requires the same
producer and input sets, even after a test manifest is re-anchored. Real registry
builds establish relative-path resolution from the observed compiler cwd. This is
not arbitrary build-script/environment closure or a transitive process audit.
The validation record retains the original external-input reproduction and all
limits. T3–T8 and the publication hold remain open.

The bounded T4 aggregate checkpoint reconciles public LLVM JSON total count and
covered values against the sum of all exported file summaries, using checked
integer addition. Percentages and notcovered retain their independent arithmetic
checks. LLVM 22.1.6 CoverageExporterJson::renderRoot and
CoverageReport::prepareFileReports define this relation. Eleven internally valid
but mutually inconsistent mutations of real exports are required acceptance
cases. This does not certify region/segment counter semantics, broaden function
owners or enable intra-function coverage/CRAP. T3–T8 remain open.

## Bounded code-region checkpoint (T4 remains incomplete)

`rust-llvm-exact-root-owner/v2-candidate` extends the previous execution-only
checkpoint with standard LLVM code-region coverage for the same exact root
owners. Each distinct code-region span contributes one denominator entry; only
its own nonzero counter contributes to covered. Duplicate spans and disagreement
between all owner regions (including excluded tests) and the file summary fail
measurement. Source activation/expansion boundaries do not broaden. Core receives
`coverage.region` alongside lexical complexity and `coverage.function`.

The real partial fixture exercises function execution 1/1 with region coverage
5/6, plus an exported never-called owner at 0/1 and 0/3. Test code contributes to
raw summaries but never to these production-owner ratios. This supersedes the
execution-only checkpoint's region-unsupported statement; line coverage and all
CRAP remain unsupported. The candidate does not select Core's CRAP model, change
requiredness/thresholds, or authorize migration/baseline adoption. LLVM region
coverage is not the historical MIR basic-block metric. T3–T8 remain open.
