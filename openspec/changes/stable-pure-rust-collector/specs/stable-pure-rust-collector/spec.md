# Stable pure-Rust collector

## ADDED Requirements

### Requirement: Rust-only installed implementation

The first-party Rust collector MUST use Rust for all its installed implementation
and MUST NOT require Python, rustc-dev, rustc_private, RUSTC_BOOTSTRAP or -Z flags.

#### Scenario: Clean supported host

- GIVEN only the documented external dependencies are installed
- WHEN the verified precompiled collector is installed and run
- THEN it collects supported measurements through stable interfaces without a private interpreter or compiler environment.

### Requirement: No implicit measurement migration

New source/coverage measurements MUST use a distinct series and reviewed boundaries.
Core policy authority, requiredness, trust, thresholds and historical baselines MUST
remain intact. Unsupported or ambiguous measurement MUST NOT become success.

#### Scenario: Incompatible historical evidence

- GIVEN historical MIR evidence and a new stable-source measurement
- WHEN a comparison lacks a reviewed compatible transition
- THEN Core rejects the comparison and no baseline is automatically rewritten.

### Requirement: Stable required CI and explicit release transition

Required CI MUST NOT build/run a compiler-private backend. New legacy publication
MUST fail until the pure-Rust stable replacement has reviewed actual acceptance.

#### Scenario: Legacy publication request during migration

- GIVEN a signing, publication or installer packet for the old workflow
- WHEN the workflow starts
- THEN it rejects the request before release environment approval and publication side effects.

### Requirement: Certified function execution owners

A candidate function-execution ratio MUST require a unique verified LLVM record
for the same source function span. It MUST NOT serve as the intra-function
coverage fraction for CRAP. Missing or ambiguous records MUST fail measurement;
uncertified syntax MUST remain unsupported.

#### Scenario: Exported but unexecuted ordinary function

- GIVEN an eligible source function and exactly one matching LLVM record with zero execution and region counts
- WHEN Core authenticates and validates its candidate evidence
- THEN its function-execution ratio is 0/1, with the source/tool/series identities retained and required CRAP still blocked.

#### Scenario: Duplicate owner or inherited parent count

- GIVEN two LLVM records claiming one source span or an execution count inconsistent with that owner's entry region
- WHEN the collector verifies the export
- THEN measurement fails without a successful evidence response or a replacement baseline.

### Requirement: Locked registry source provenance

Registry package provenance MUST bind an explicit archive input to its lockfile
checksum and to every source file Cargo compiles. Runtime code MUST NOT infer
private Cargo cache layouts. Uncertified source kinds and incomplete dependency
inventories MUST block collection; verification MUST recompute provenance.

#### Scenario: Modified cache under an unchanged lock

- GIVEN a matching locked archive and a dependency source file modified in the local cache
- WHEN the collector prepares compilation or verifies captured evidence
- THEN measurement fails before emitting a successful evidence response.

#### Scenario: Omitted package proof

- GIVEN otherwise anchored evidence whose dependency proof omits a locked package
- WHEN the collector verifies the manifest and dependency inventory
- THEN the missing provenance is rejected, even if the test manifest is re-anchored.
