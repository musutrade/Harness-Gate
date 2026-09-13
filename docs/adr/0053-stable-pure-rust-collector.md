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
lexical complexity in a separate candidate series. Coverage/CRAP remains unsupported;
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

The T4 raw-evidence checkpoint validates the exercised public LLVM JSON structure,
integer domains, region IDs, segments and summary arithmetic with duplicate-key
rejection. Thirteen mutations of real exports now fail after test-only manifest
re-anchoring, and package preparation requires those checks. This remains a raw
format contract: aggregate/counter reconciliation, certified source owners and
normalized coverage/CRAP are not complete. T4, T6–T8 remain unchecked; required
metrics, migration review and the release hold are unchanged.
