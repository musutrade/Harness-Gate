# ADR 0052: Dependency-based collector compatibility

Status: Accepted for implementation; release acceptance remains pending.

## Context

Exact kernel, glibc version and host library fingerprints reject machines whose
actual dependencies can run the packaged collector. A user should install and
upgrade through one entry point without recreating the publisher machine.

## Decision

Introduce signed delivery v3 and runtime schema 2 with explicit dependency bounds.
Derive minimum glibc from required symbols of packaged runtime ELF files. Verify
architecture, loadable host libraries and actual compiler/coverage behavior.
Treat build-host kernel and library inventory equality as diagnostics for v3.
Acquire missing private components through the existing digest-addressed store;
reuse verified components on subsequent versions. Bootstrap an independently
pinned OpenSSL verifier instead of demanding the publisher's host OpenSSL path.

This is the explicit compatibility policy delta: a reviewed receipt binds the
manifest, exact Core/protocol and private tools, while host acceptance follows
the signed dependency bounds. Existing v1/v2 contracts keep exact-host matching.
Authentication, exact payload/tool hashes, capture integrity, Core policy ownership,
configuration, series transition and baseline/lineage rules are unchanged.
Missing or incompatible dependencies fail with actionable diagnostics before
activation. No global package or toolchain defaults are modified automatically.

## Validation and release limits

Exercise differing compatible hosts, actual native execution, missing dependencies,
tampered packages/tools and absent or ambiguous compatibility receipts. A development
runtime test does not certify protected publication, production signatures or a new
measurement series. Published installer pins remain unchanged until release acceptance.
