# Tasks: Harness-Gate and DevRail Capability Contracts

**Parent:** [proposal.md](proposal.md), [design.md](design.md), and
[capability-contracts specification](specs/capability-contracts/spec.md)
**Status:** Proposed; no task is complete until its acceptance evidence is
reviewed in a green CI run.
**Implementation restriction:** Keep DevRail business controls out of the CLI,
preserve the serial default, and do not add an unstable in-process plugin API.

Every task is bounded to less than four hours and has an explicit priority,
effort, and acceptance criterion.

## 1. Baseline and Contract Inventory

- [ ] **1.1 (P0, S)** Capture frozen serial invocation fixtures.
  **Acceptance:** Fixtures include commit, scope, effective config, command/env
  summary, step order, report paths, status/error mapping, cleanup observations,
  and platform/toolchain metadata.
- [ ] **1.2 (P0, S)** Approve result and artifact JSON schemas.
  **Acceptance:** Schemas define `schema_version`, stable IDs, statuses,
  attempts, artifacts, evidence completeness, compatibility policy, and separate
  parser/zero/partial/report-write failures.
- [ ] **1.3 (P0, S)** Define the invocation ID and path allocation rules.
  **Acceptance:** Collision, normalization, report-root containment, retention,
  and atomic-write fixtures have deterministic expected outcomes.
- [ ] **1.4 (P0, S)** Implement the compatibility launcher contract.
  **Acceptance:** Existing DevRail input can be replayed in serial mode with no
  change to current gate ordering, report names, or public error mappings.

## 2. Runner and Isolation (P0)

- [x] **2.1 (P0, M)** Add versioned runner configuration parsing.
  **Acceptance:** Threads, environment mapping, argument insertion, result
  format, and isolation fields validate before side effects; unknown fields fail
  with a diagnostic.
- [ ] **2.2 (P0, M)** Implement shared/schema/database isolation adapters.
  **Acceptance:** Shared mode is rejected for concurrency greater than one;
  schema/database workers receive unique identities and cleanup state.
- [ ] **2.3 (P0, M)** Record effective invocation inputs.
  **Acceptance:** Machine results contain effective args, declared env keys and
  redacted values, runner version, isolation mode, and migration/lock decisions.
- [ ] **2.4 (P0, M)** Test worker cancellation and abnormal exit cleanup.
  **Acceptance:** Cancelled or crashed workers cannot leave reusable schemas,
  containers, ports, or connections for a later invocation.

## 3. Invocation Evidence (P0)

- [ ] **3.1 (P0, M)** Create invocation-scoped report and artifact directories.
  **Acceptance:** Concurrent invocations produce disjoint paths with stable
  step/attempt names and no shared fixed filenames.
- [ ] **3.2 (P0, M)** Add atomic report, manifest, and log publication.
  **Acceptance:** Temporary-file/rename writes either publish complete files or
  return a blocking report failure; partial files are cleaned up.
- [ ] **3.3 (P0, S)** Add artifact manifest and digest generation.
  **Acceptance:** Every exported log/report/artifact has size and SHA-256,
  references resolve inside the invocation directory, and tampering is detected.
- [ ] **3.4 (P0, S)** Apply redaction and retention boundaries.
  **Acceptance:** Tokens, cookies, private keys, connection strings, and full
  request headers are absent from exported evidence; cleanup follows policy.

## 4. Cross-Process Resource Leases (P0)

- [ ] **4.1 (P0, M)** Define owner markers and lease records.
  **Acceptance:** Resource identity, invocation/process-start identity, heartbeat,
  expiry, kind, and schema version are persisted and validated.
- [ ] **4.2 (P0, M)** Implement idempotent acquire/renew/release.
  **Acceptance:** Concurrent owners receive one lease and a structured conflict;
  retries of lifecycle operations do not corrupt ownership.
- [ ] **4.3 (P0, M)** Implement stale-owner detection and reclaim.
  **Acceptance:** Killed owners are reclaimable within the configured bound;
  PID reuse and host restart do not cause false ownership transfer.
- [ ] **4.4 (P0, S)** Add `doctor/cleanup --dry-run` evidence.
  **Acceptance:** Dry-run lists only marked Harness-Gate resources; reclaim
  failures remain blocking structured results and preserve audit evidence.
- [ ] **4.5 (P0, M)** Add Linux/macOS/Windows lifecycle tests.
  **Acceptance:** Process-tree termination, service cleanup, lease expiry, and
  no-leak assertions run on each supported CI platform.

## 5. Result Protocol and Test Semantics

- [ ] **5.1 (P0, M)** Emit the unified machine-result schema.
  **Acceptance:** Scope, step, service, warning, failure, artifact, and
  skipped/cancelled fields are stable and ordered by validated plan order.
- [ ] **5.2 (P0, S)** Add schema contract and compatibility tests.
  **Acceptance:** DevRail fixture consumers map results without parsing Markdown
  or logs; unsupported schema versions fail closed.
- [ ] **5.3 (P1, M)** Add waiver validation and `WAIVED` status.
  **Acceptance:** Expired, out-of-scope, and self-approved waivers fail before
  execution; valid waivers include approval evidence and remain machine-distinct.
- [ ] **5.4 (P1, M)** Add standard JUnit/TRX/JSON test result ingestion.
  **Acceptance:** Parser mode/version and completeness are recorded; malformed,
  zero, and partial results have distinct failures.
- [ ] **5.5 (P1, M)** Add bounded retry/flaky/sharding semantics.
  **Acceptance:** Attempts, retry class/count, flaky state, shard identity, and
  merge identity are replayable; missing or duplicate shards fail merge.

## 6. Release Integrity (P0)

- [ ] **6.1 (P0, M)** Generate checksum manifests and SBOMs for every asset.
  **Acceptance:** SPDX or CycloneDX output binds the source commit, lockfile,
  dependencies, and toolchain to each release asset.
- [ ] **6.2 (P0, M)** Sign release metadata and publish provenance.
  **Acceptance:** Clean-environment verification validates asset, manifest,
  SBOM, signature, and workflow provenance; any byte mutation fails.
- [ ] **6.3 (P0, S)** Pin release actions and enforce least privilege.
  **Acceptance:** Workflow references and permissions are reviewed, and
  published assets cannot be overwritten without an auditable release event.
- [ ] **6.4 (P0, S)** Document offline consumer verification.
  **Acceptance:** Installation/upgrade docs provide commands and trust material
  for digest, signature, SBOM, and provenance checks.

## 7. Migration and Adapter Evolution

- [ ] **7.1 (P0, M)** Build normalized shadow comparison.
  **Acceptance:** Old/new results are correlated by request and invocation IDs;
  differences are classified and raw evidence is retained for the audit window.
- [ ] **7.2 (P0, M)** Run one bounded canary slice.
  **Acceptance:** Required-check ownership remains in DevRail until equivalent
  results, no resource/artifact leaks, and offline release verification pass.
- [ ] **7.3 (P0, S)** Verify rollback and post-run cleanup.
  **Acceptance:** Disabling the launcher returns to the frozen path without
  deleting evidence or reclaiming unowned resources.
- [ ] **7.4 (P2, L)** Propose the signed out-of-process adapter protocol.
  **Acceptance:** Separate ADR/OpenSpec records protocol versioning, capability
  declarations, crash isolation, permissions, compatibility, upgrade, and
  rollback; this task does not block built-in cutover.

## Evidence Review

After implementation, link the contract-test run, release verification output,
shadow/canary comparison, and rollback drill here. Until those links exist and
required CI checks are green, this change and ADR-0032 remain **Proposed**.
