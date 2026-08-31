# Proposal: Signed Out-of-Process Adapter Protocol

**Status:** Proposed
**Date:** 2026-08-31

Define a versioned process boundary for organization-specific test runners and
scanners. The adapter must be independently signed, capability-scoped, and
isolated from the Harness-Gate scheduler while emitting the existing
machine-result schema.

This change is P2 and does not block built-in runner or DevRail migration work.
