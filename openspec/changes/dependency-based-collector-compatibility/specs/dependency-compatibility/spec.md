# Dependency compatibility

## ADDED Requirements

### Requirement: Installation follows actual runtime dependencies

New v3 manifests MUST declare the architecture, minimum glibc, host libraries and
commands required by their runtime. Compatible kernel or host-library byte changes
MUST NOT independently reject installation. Missing private components MUST use
existing authenticated acquisition and shared storage.

#### Scenario: Compatible clean host

- GIVEN the signed runtime requirements are met on a different kernel/glibc release
- WHEN package authentication and native self-tests succeed
- THEN installation may activate without publisher-host fingerprint equality.

#### Scenario: Incompatible or modified dependency

- GIVEN a required dependency is unavailable or a pinned private tool is modified
- WHEN installation checks the runtime
- THEN it reports the failure and preserves the previous active version.

### Requirement: Preserve authenticated compatibility and measurement identity

v3 MUST retain exact manifest, Core/protocol and private tool receipt binding.
Legacy manifests MUST retain exact-host matching. Capture, configuration and
series/lineage rules MUST remain unchanged.

#### Scenario: Missing reviewed receipt

- GIVEN a compatible runtime but no unique authenticated matching receipt
- WHEN collection preflight runs
- THEN collection fails closed.
