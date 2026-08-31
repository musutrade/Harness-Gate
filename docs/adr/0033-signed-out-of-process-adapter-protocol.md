# ADR-0033: Signed Out-of-Process Adapter Protocol

## Status

**Proposed** (2026-08-31)

## Decision

Future organization-specific runners and scanners use a signed, out-of-process
adapter protocol. Harness-Gate does not load dynamic libraries into the main
process and does not expose an unstable in-process plugin API.

An adapter receives one JSON request on stdin and returns one JSON response on
stdout. The request declares `protocol_version`, adapter identity, invocation
and step IDs, timeout, cancellation endpoint, configuration digest, and the
allowed resource/network/permission set. The response uses machine-result
schema version 1 and may reference only artifacts below the invocation root.
Logs are streamed on stderr and captured through the same redaction and
manifest pipeline as built-in steps.

Before dispatch, Harness-Gate verifies the adapter binary digest and signature,
checks the supported protocol/result-schema matrix, and rejects undeclared
capabilities. A timeout or crash fails only the adapter node; dependency
propagation, cancellation, leases, and report publication remain scheduler
responsibilities. Upgrades require a new signed digest and can be rolled back
by selecting the previous adapter record without deleting evidence.

## Compatibility Contract

Protocol and result schema versions are independent. Additive fields are
backward-compatible within a major protocol version; breaking changes require
a new major version and an explicit migration. The adapter must declare the
minimum and maximum Harness-Gate versions it supports. Contract fixtures cover
PASS, FAIL, CANCELLED, timeout, malformed response, artifact escape, and crash
cases.

## Security Boundary

Adapters run with a per-invocation working directory, no inherited secret
environment, and only declared network/resource permissions. The host records
the source digest, signature identity, capability declaration, exit status,
and cleanup outcome. An invalid signature, unsupported capability, malformed
response, or artifact escape is a blocking `ADAPTER_PROTOCOL_FAILURE`.

## Rollout

This ADR is a P2 evolution track. Built-in runner cutover does not depend on
adapter support. A future implementation must add an OpenSpec change and pass
the protocol fixtures before enabling an adapter for a canary slice.
