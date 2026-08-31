# Adapter Protocol

## ADDED Requirements

### Requirement: Signed, capability-scoped process boundary

An adapter SHALL declare protocol/result versions, source digest and
signature, timeout/cancellation behavior, artifacts, network, resources, and
permissions before execution. Unsupported or unsigned declarations SHALL fail
closed.

#### Scenario: Reject an unsigned or unsupported declaration

- **WHEN** the host receives an adapter declaration without a trusted
  signature or with an unsupported protocol version
- **THEN** the host rejects it before starting the adapter and emits a
  structured `ADAPTER_PROTOCOL_FAILURE`

### Requirement: Failure isolation

An adapter crash, timeout, malformed response, or artifact path escape SHALL
fail only its node and SHALL preserve scheduler dependency, cancellation,
redaction, lease, and evidence rules.

#### Scenario: Isolate a crashed adapter

- **WHEN** an adapter exits unexpectedly after its node starts
- **THEN** the node is marked failed, dependent nodes follow normal scheduler
  rules, and unrelated nodes continue without sharing logs or artifacts

#### Scenario: Reject an escaped artifact

- **WHEN** an adapter response references a path outside its invocation root
- **THEN** publication fails closed, the path is not exported, and the
  invocation retains a reportable protocol failure
