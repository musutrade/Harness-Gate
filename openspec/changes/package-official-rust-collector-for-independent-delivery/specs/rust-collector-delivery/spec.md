# Capability: Rust Collector Independent Delivery

## ADDED Requirements

### Requirement: Independent immutable bundle
The official collector SHALL have an independent exact version and self-contained declared runtime for the certified Linux x86_64 host ABI, without a source checkout or global toolchain mutation.

#### Scenario: Clean installation
- **GIVEN** a supported host has no Harness-Gate source tree or Python installation
- **WHEN** the verified bundle is installed
- **THEN** doctor and real fixture collection use the private runtime; unsupported ABI fails before sampling.

### Requirement: Delivery manifest compatibility
The signed manifest SHALL bind payload identities, tools, capabilities and explicit Core/protocol compatibility. Package SemVer SHALL NOT imply measurement-series compatibility.

The [GH-227 contract](../../../../../docs/quality/rust-collector-delivery-contract.md)
specifies the strict schema and exact reviewed matrix. Unknown combinations SHALL
fail before sampling. Declared versions alone SHALL NOT establish tested support.

#### Scenario: Malformed manifest or wrong runtime tool
- **GIVEN** missing/unknown manifest fields, duplicate keys, unsafe payload paths, or mismatched observed tool bytes/version
- **WHEN** delivery preflight runs
- **THEN** it rejects the input before any producer launch and supplies no usable measurement evidence.

#### Scenario: Unknown Core or ABI combination
- **GIVEN** no unique reviewed receipt matches the exact manifest, Core binary/commit/version, protocols, tools and host ABI
- **WHEN** preflight evaluates the combination
- **THEN** sampling is blocked, including when the tested matrix is empty; no version or policy fallback is selected.

#### Scenario: Relocated historical capture
- **GIVEN** a capture records different tool paths or incompatible tool/series identities
- **WHEN** comparison or re-export is requested
- **THEN** the incompatible history is rejected without rewriting its anchor or baseline.

#### Scenario: Proposed series transition lacks complete evidence
- **GIVEN** relocation or repackaging is proposed without retained original bytes, independent anchors, fresh native re-exports and reviewed identity/lineage comparison
- **WHEN** historical compatibility is requested
- **THEN** the transition remains rejected; package version/path changes do not establish equivalence, reset a baseline or alias a measurement series.

### Requirement: Measurement-only transport
The collector SHALL reuse the existing v1 request/response and normalized evidence contracts, preserving all error and capability states. Final quality decisions SHALL remain in released Rust Core.

#### Scenario: Valid low coverage
- **GIVEN** complete authenticated collection has low coverage
- **WHEN** the collector responds and Core evaluates
- **THEN** valid facts are delivered without a collector release verdict and Core retains the real policy failure.

### Requirement: Operational failure remains blocking
The product entry SHALL distinguish measurement completion from operational failure without translating arbitrary legacy nonzero exits to success.

#### Scenario: Failed legacy subprocess
- **GIVEN** a producer fails and leaves partial output
- **WHEN** the standalone collector processes its result
- **THEN** no usable success evidence is emitted and required verification remains blocked.

### Requirement: Checked installation lifecycle
Installation SHALL verify exact-tag origin, complete inventory, checksums and provenance before safe extraction and atomic activation. Rollback and uninstall SHALL preserve project evidence and unrelated assets.

#### Scenario: Interrupted or malicious installation
- **GIVEN** an active verified version exists
- **WHEN** installation is interrupted or the archive contains traversal or undeclared payloads
- **THEN** activation is refused and the prior version and project evidence remain unchanged.

### Requirement: Protected independent publication
Publication SHALL use an explicit independent inventory, SBOM and protected release approval while preserving existing Core required checks.

#### Scenario: Incomplete release assets
- **GIVEN** a required signature, attestation or inventory subject is missing
- **WHEN** RC publication is attempted
- **THEN** publication fails without weakening checks or substituting unsigned assets.

### Requirement: Capture trust and retention
Delivery signatures SHALL NOT substitute for host capture authentication. Native re-export acceptance SHALL retain all original required bytes under reviewed anchors.

#### Scenario: Missing original binaries
- **GIVEN** a historical report is available but original binaries are absent
- **WHEN** native re-export acceptance is evaluated
- **THEN** archive replay is identified as such and does not satisfy fresh re-export acceptance.

### Requirement: Bounded rollout
RC delivery SHALL NOT transfer project authority, accept baselines, complete GH-215 or certify additional platforms. Stable promotion SHALL await separately accepted fresh Arc-Admin integration.

#### Scenario: Installed RC but incomplete project inputs
- **GIVEN** the RC is installed but trusted workflow state or lineage is missing
- **WHEN** the project attempts required verification
- **THEN** verification stays blocked and its original required gates remain authoritative.

## Implementation and review

Sequence and effort are in [design](../../design.md) and [tasks](../../tasks.md).
The design includes proposed command examples, alternatives, success metrics and rollback.
This capability adds delivery constraints; existing collector protocol requirements are unchanged.
The development v1 protocol and released Core project-collector/adapter envelopes
remain distinct as inventoried in the contract; integration must validate the
applicable envelope without moving final policy authority. Relevant Engineering
Policy semantics remain unchanged. Synthetic contract tests are not native positive
measurement or independently delivered Core/ABI certification.
