# Design: Harden Gate Boundaries and Delivery Contracts

## Scanner Boundary

Working-tree scanners inspect Git-listed paths only after rejecting symlink
file entries. Audit traversal also verifies that a candidate is a regular file
below the repository root before reading it. Staged secret scanning continues
to read Git blobs, which do not follow filesystem links.

## Cancellation and Process Ownership

Unix keeps the existing session/process-group implementation. Windows uses
`taskkill /PID <pid> /T /F` to terminate the complete descendant tree, with a
direct-kill fallback when the command is unavailable. Secret and audit loops
check the shared cancellation flag between files and return a typed
cancellation error. Platform-specific tests cover descendant cleanup where
the host supports it.

## Compiled Configuration

Scope rules expose a compiled matcher cache built when the flow configuration is
loaded. Audit allowlist regexes are compiled once per rule and passed into the
scanner. Rule violations carry the originating rule name separately from the
pattern label used for display.

## User-Facing Contracts

All remediation text and headings use `harness-gate`; the installer points to
the `musutrade/Harness-Gate` release repository. The Linux textual contract is
regenerated for the current package version. Doctor file checks resolve
repository-relative paths against `Project::root`.

## Rollback

Revert the implementation and this change record together. No persistent data
format or report schema migration is required.
