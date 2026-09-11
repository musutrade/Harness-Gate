# Proposal: Release Harness-Gate 0.4.0

**Status:** In progress (2026-09-11)

## Why

The authentic latest published Core v0.3.7 lacks `quality evaluate`, while
protected main contains the accepted generic Rust quality Core. GH-230 cannot
complete released-Core acceptance with a locally built binary reporting 0.3.7.
Publish the already accepted main implementation under a distinct minor version
only after candidate validation and normal protected release gates.

## Scope

Synchronize package/lockfile, CLI version contract, snapshots, installation
examples and changelog. Validate candidate native policy integration, all
applicable repository gates, exact-main CI and the existing signed release
inventory/provenance workflow. Then verify the actual distributed Linux binary
and provide it to GH-230. Relevant Engineering Policy semantics are unchanged.
Adapter protocol v2 requires compatible producers; no legacy security fallback.

## Non-goals

No GH-230 in-progress source changes, independent collector publication,
clean-host acceptance claim, Arc-Admin writes, GH-215 activation, baseline reset,
policy delta, existing tag mutation or release-gate bypass.

## Acceptance

Candidate `quality evaluate` must consume real native evidence and preserve
policy failures. Required PR and exact protected-main CI must pass before tag
v0.4.0. The existing release environment, multi-platform builds, signature,
provenance and package checks remain mandatory. Verify published asset bytes and
actual invocation before resuming GH-230. Record exact evidence as it completes.
