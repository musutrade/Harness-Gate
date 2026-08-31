# Design: Adapter Protocol

## Transport

One UTF-8 JSON request is written to stdin. The adapter writes one JSON result
to stdout and diagnostic logs to stderr. A cancellation signal is delivered by
closing stdin followed by a bounded process-tree termination.

## Request

The request contains `protocol_version`, adapter name/version, invocation ID,
step ID, timeout, configuration digest, artifact root, and explicit arrays for
network, resource, and environment capabilities. Unknown fields and
capabilities are rejected before process start.

## Response

The response contains machine-result schema version 1 fields, final status,
attempt history, parser/completeness evidence, and invocation-relative artifact
references. Harness-Gate applies redaction, size/digest manifest generation,
and path containment checks before publication.

## Failure and upgrade

Signature, version, malformed-response, timeout, crash, and artifact-escape
failures map to `ADAPTER_PROTOCOL_FAILURE` and fail only the adapter node. A
new adapter digest is activated through a reviewed configuration change; the
previous signed record remains available for rollback.
