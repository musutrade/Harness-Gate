# Generated owners from a real source file (stable alternative)

## Current line and migration checkpoint (2026-09-14)

The v4 candidate adds authenticated per-function line coverage, including real
generated files. Core computes an exact `crap-line-1` migration preview from
validated same-owner evidence. The preview has no gate or baseline authority;
required CRAP remains blocked. Source/region reconciliation, mutation checks,
original historical anchors and old/new series rejection remain mandatory.
See [the line/CRAP contract](stable-rust-line-crap-migration.md).


This checkpoint addresses the generated-function coverage gap recorded in
[stable-rust-macro-observation.md](stable-rust-macro-observation.md) with a
**stable-interface alternative**, rather than waiting on an unmerged compiler
change. It does not adopt a new metric, change the token-stream boundary, or
remove the release hold.

## The gap and why it is not a version artifact

On Rust 1.98.1 with LLVM 22.1.8, a function-like proc macro that emits whole
functions produces **no function owners** in the LLVM source-coverage export.
Only the macro implementation and the consumer test functions appear. This was
first recorded on 1.97.1 and re-checked on 1.98.1: both `default` and `branching`
still omit `plain`, `branch`, `unexecuted` and `configured`. Missing owners are
not zero executions. See `generated-source-owner-repro.json`.

## The stable alternative

When the generated source is a **real file** brought into the crate with
`include!`, each generated function gets a distinct owner with real counters,
and a deliberately unexecuted function gets an **explicit zero record** rather
than being absent. Two shapes were verified end to end on 1.98.1:

- a committed/generated `.rs` file included directly, and
- a file produced by a build script into `OUT_DIR` and included by path.

`tools/quality/fixtures/rust-generated-owners` fixes the second shape as a real
regression. A dependency-free generator (`generator/`) is called by
`consumer/build.rs`, which writes `OUT_DIR/generated_owners.rs`; the consumer
pulls it in with `include!`. One generation path serves the writer and any
observer, matching the "shared generation library" requirement in Engineering
Policy section 10.

Observed owners (LLVM JSON 3.1.0), identical across `default` and `branching`:

| Generated function | Default | Branching | Cyclomatic (source) |
| --- | --- | --- | --- |
| `plain` | 1 | 1 | 1 |
| `branch` | 2 | 2 | 2 |
| `configured` | 1 | 1 | 1 or 2 (selected by feature) |
| `unexecuted` | 0 | 0 | 2 |

`configured`'s generated body differs between configurations (identity vs.
single-`if`), so the feature genuinely selects different generated source.
`unexecuted` is never called and still has a real zero entry.

## Core source workspace integration

The collector now has an explicit source export for the existing Core contract.
Engineering-policy semantics, Core source validation, thresholds, requiredness,
CRAP and historical baselines are unchanged. This remains a candidate series.

```sh
harness-gate-rust-stable-collector export-core-source CAPTURE MANIFEST_SHA256 REQUEST_SHA256 NEW_SOURCE_WORKSPACE
harness-gate-rust-stable-collector describe CAPTURE MANIFEST_SHA256 REQUEST_SHA256 --source-workspace SOURCE_WORKSPACE
```

The new workspace must be outside both the measured project and capture and must
not already exist. It contains byte-verified project inputs under `project/` and
captured generated Rust inputs under `generated/`. Each generated path includes
an identity for its scratch-relative original path as well as its content digest.
Identical bytes at different generated paths therefore remain distinct subjects.
No source is rebuilt or modified during export. The original capture and project
remain the authority for re-verification; this is not a relocated historical capture.

`source-workspace.json` binds the complete file inventory, original project root,
original capture and request digests, generated paths and compilation records.
The collector recomputes the expected workspace from the verified capture before
and after adapter conversion. Missing, added, altered or symlinked files, forged
workspace manifests and cross-configuration substitutions fail. A partial export
has no completed manifest; retry it at a fresh output path.

With `--source-workspace`, `describe` returns workspace-relative source paths for
both project and generated functions, including source complexity and exact
function/code-region coverage where certified. The signed Core input uses that
same `workspace_root`; adapter conversion validates it against the original
capture. Core retains its ordinary canonical path, readable source and digest
checks and needs no production code or schema change. The source identity is
explicitly `rust-authenticated-source-workspace/1-candidate`, distinct from the
original workspace-only series. Existing bindings and baselines are not adopted.
Without the option, `describe` retains the original workspace-only scope.

Generated-input provenance retains every consuming compiler dep-info record,
not just one record per content digest. These records authenticate which compiler
invocations consumed the generated bytes; they do not establish arbitrary
build-script input/environment closure or identify a general macro generator.

## Certified boundary and remaining work

- Verified real `.rs` inputs are exported in `generated-owners.json`; verification
  re-analyzes their authenticated bytes and recomputes all owners, compilation
  bindings, counts and code-region ratios.
- The explicit source workspace enables authenticated Core evidence for those
  generated functions. Unexecuted functions retain actual zero counters.
- Proc-macro token streams and built-in derive coverage are still uncertified.
  The `include!` fixture is a bounded alternative, not automatic rewriting of
  arbitrary target projects or general macro support.
- The v4 candidate adds authenticated line coverage. Core still blocks required CRAP;
  measurement migration, full Linux matrix and protected release acceptance remain
  incomplete. No release-hold removal or publication is authorized.

## Regression coverage

`validate_generated_owners.py` exercises both default and branching configurations,
then a duplicate-source configuration with two byte-identical generated files and
different execution counts. It checks capture, source export, describe, recomputed
verification, forged/dropped owners and compilation bindings, independent source
paths, snapshot mutation, missing/extra files, symlinks, self-reanchoring and mixed
configurations. Each configuration is then passed through the real signed Core
adapter and evidence validator by `stable_collector_acceptance` in required CI.
That acceptance asserts four generated Core records (eight for duplicate-source),
non-null complexity and coverage, real zero counters, rejected source substitution
and unchanged required-CRAP blocking. These drivers remain repository automation;
the installed collector and source export are entirely Rust.

Local acceptance on Rust 1.98.1 is recorded in
[`core-source-workspace.json`](stable-rust-candidate-evidence/core-source-workspace.json):
58 generated-source checks and nine signed Core cases pass for the final local
executable. Core runtime validation is unchanged. Local execution was not traced;
the required CI job performs the observer checks for the submitted head. The
record distinguishes earlier raw capture acceptance from final-binary Core
verification and retains the initial temporary-filesystem quota failure.
