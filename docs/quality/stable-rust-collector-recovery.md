# GH-259 bounded recovery: current stable and remaining blockers

This checkpoint keeps GH-259, T3–T8 and the legacy release hold open. New candidate
builds, required plugin CI and package preparation use **Rust 1.98.1** with matching
LLVM 22.1.8; historical 1.97.1 evidence does not impose a second-version gate.
The policy baseline is [PR 260](https://github.com/musutrade/Harness-Gate/pull/260)
at `c2f7c14fbee0245e3a48176f7a768f6d3fd1a041`. This continues the preserved
implementation and remote operator patches, without resetting local Git history.

## CI and operator evidence

The current operator checkpoint supersedes the pending-CI actions below:
[run 34792991105](https://github.com/musutrade/Harness-Gate/actions/runs/34792991105)
at `9dec7e7b10bde2f9a5b09f557d51ba755462a093` passes all required jobs. Its
successful Quality Script Tests log records 10 actual Rust exec-observer
regressions, the traced candidate build, 125 capture checks, 26 macro checks,
six authenticated Core fixtures, package preparation, 38 lifecycle checks and
29 HTTPS checks. The observer follows kernel successful-exec stops; it is
repository automation, not a payload or user dependency. The original strace
unknown records remain rejected. Upgrade compilation now explicitly uses 1.98.1.

The operator handoff is `target/operator-exec-audit/handoff.json`; its 13 copied
file hashes were checked locally. Original anchors remain unchanged and no
external workspace was accessed. These results belong to the operator candidate,
not automatically to a subsequently rebuilt executable. The umask 0022 lifecycle
pass did not fix the independently reproduced umask 0002 defect. The focused
Rust fix and new real installation evidence are recorded in
[install-permissions.json](stable-rust-candidate-evidence/install-permissions.json).

The focused continuation passes the stable 1.98.1 release build, both projects'
fmt/Clippy checks, quality unittest discovery (449 tests, 36 manual legacy skips)
and docs consistency. Core's initial nextest command failed one local webhook
test with `Connection refused` and left 38 tests unrun. The failing test passes
with inherited proxy variables removed for that process; the 38 previously unrun
tests also pass. All 397 distinct tests therefore have passing evidence across
these runs, while the original command remains recorded as failed. The new binary
has nine focused install checks, not a renewed full capture/release acceptance.

The following cancelled-run details are retained as history:

[Run 34787905972](https://github.com/musutrade/Harness-Gate/actions/runs/34787905972)
at `cc1681ed085469e02b8115c87e22839b9e2dd7f1` has a **cancelled** Quality Script
Tests job, `103806637783`. Its actual log records 125 capture checks, 26 bounded
macro checks and six Core fixture successes, followed by cancellation at
`2026-09-13T22:57:14.3414194Z`. It does not show a new trace-parser failure or
establish completed CI. The original failed trace is not waived: complete exit
records remain required, and unknown, detached or truncated records remain errors.

The next operator commit, `231edba33548db64ee5f7644c2da7a2e6fbaf161`, already pins
CI's build/install/runtime/package toolchain to 1.98.1. This follow-up fixes the
remaining two diagnostic defaults and supplies explicit 1.98.1 arguments so those
commands cannot combine a 1.97.1 compiler with 1.98.1 LLVM tools. Package preparation
now rejects historical compiler selection and requires current-stable acceptance
anchored to the exact newly built binary. Historical acceptance decoding remains.

[Operator evidence](stable-rust-candidate-evidence/operator-unblock.json) identifies
binary `62feafa4831df83843fa71e7384d15c7bd79fb6c679525bcccfb853d47607409`:

- 125 actual capture checks and six authenticated Core fixtures on target 1.98.1,
  with audited process traces. Required CRAP remains blocked.
- The same program ran doctor/prepare/collect/verify in Ubuntu 24.04 with matching
  tools. Eight audited traces cover both recorded compiler versions; only 1.98.1
  is now required. This is a plain-fixture second-userspace observation, on the
  same kernel, not the full two-system acceptance matrix.
- All 1,656 local copied files matched `target/operator-gh259-evidence-sha256.json`.
  Original absolute paths and anchors are preserved. No original external workspace
  was accessed, no evidence was recaptured or rebound to the copies.
- Cosign 3.1.3 and its public trust input are provisioned. No private production
  signing key or production signing authorization was supplied.

## Fresh 1.98.1 build and focused validation

The new release executable is
`60db68068ea8fee348e0b9e00fb3619ef060a0fc7e1ec2a6a6f525760b1b06f7`,
3,497,880 bytes. Its locked offline release build and 125 actual capture checks
passed with Rust 1.98.1 and LLVM 22.1.8. These checks are untraced: local strace
failed with `PTRACE_TRACEME: Operation not permitted` and `PTRACE_SEIZE: Operation
not permitted` before the traced build could start. The operator's traces and
Core/Ubuntu receipts describe the earlier binary, not this new executable.
ELF dynamic dependencies are `libgcc_s.so.1` and `libc.so.6`; there is no dynamic
compiler-private or Python library. ELF inspection is not a substitute for the
missing new-binary transitive process audit.

Unsigned package preparation passed with the new binary's pinned 1.98.1 capture
summary. The four-file directory contains the executable, LICENSE, support.json
and release-inventory.json: **6,044,708 bytes**, with **0 download bytes** during
preparation. The license file accounts for 2,545,341 bytes. The captured plain
fixture occupies 50,203 bytes; persistent collector cache is zero. The new unsigned
candidate was not installed or upgraded: its installed bytes, signed-package bytes
and upgrade download bytes are **not measured**, not zero. Historical lifecycle
measurements remain separate. No acceptance archive or toolchain is in the payload.

The initial capture invocation used the wrong executable basename and ran no
collector; the corrected invocation then failed doctor because the build harness
had exported empty RUSTFLAGS. Both errors are retained. The fresh `capture-clean`
run unsets the prohibited inherited flags and wrappers and passes all 125 checks.
It does not change target-project toolchain files or any baseline.

Commands, hashes and outcomes are in
[recovery-198.json](stable-rust-candidate-evidence/recovery-198.json); raw logs and
unmodified outputs remain at `target/gh-259/recovery-198/`. Candidate Rust tests pass 22; quality discovery passes 449 with 36 explicit
manual legacy skips; release discovery passes 92. Docs consistency passes after
writing the linked recovery evidence; the initial missing-link failure is retained.
Unchanged Core results are
reused from the earlier validation checkpoint (397 nextest tests, formatting and
clippy), not rerun or represented as new 1.98.1 acceptance. Project config check
and verify are not applicable because `.harness-gate/flow.toml` is absent.
Repository Python
scripts orchestrate these development checks and package preparation only; none
are included in the plugin payload.

## Smallest critical capability gaps and next actions

1. **Generated function owners and execution counters.** The fixed shared-generator
   fixture was run four times on 1.98.1: original/diagnostic wrapper under default
   and branching features. Tests exercise differing invocations/results; one
   generated function is deliberately unexecuted. All four exports contain consumer
   test owners but omit `plain`, `branch`, `unexecuted` and `configured` business
   owners. Generator source/version, invocation input, configuration, output tokens,
   source spans, compiler and LLVM identities are recorded in the diagnostic.
   Equal body/interior source ranges reproduce on current stable. Stable spans do
   not expose compiler-internal expansion context, so these observations do not
   prove an internal root cause or justify a guessed syn/quote/proc-macro2 patch.
2. **Built-in derive instrumentation exclusion.** At the exact installed rustc
   commit `48a229ceaefd4985c50990b14116b6d856af0985`, the
   [built-in derive generator](https://github.com/rust-lang/rust/blob/48a229ceaefd4985c50990b14116b6d856af0985/compiler/rustc_builtin_macros/src/deriving/generic/mod.rs#L800)
   still adds `automatically_derived`, and the
   [coverage eligibility query](https://github.com/rust-lang/rust/blob/48a229ceaefd4985c50990b14116b6d856af0985/compiler/rustc_mir_transform/src/coverage/query.rs#L59)
   excludes those implementations and nested bodies from instrumentation. This is
   source-confirmed compiler policy. The earlier six-test derive reproduction is
   historical 1.97.1 evidence. The complete 1.98.1 diagnostic stopped at missing
   rustfmt; it is not counted as passed. A separate runtime-only continuation used
   the unchanged fixture with matching 1.98.1 LLVM tools: default/extra each passed
   three tests and exported eight ordinary/manual owners, including an unexecuted
   wrapper with count zero. Both exports omit all four derived clone owners and
   the annotated control. This independently confirms the runtime boundary on
   current stable without changing business code or waiving the formatting check.
   Source/tool identities, commands, exports and the standalone driver identity
   are retained under `target/gh-259/continuation-2/derive-runtime/` and summarized
   in `recovery-198.json`. These runs have no process trace and do not certify CRAP.
3. **Required CRAP and migration.** Shared source generation supplies bounded
   complexity, not missing execution evidence. The current Core adapter does not
   certify any function CRAP, including plain owners. It must continue to block
   required CRAP until the approved same-function coverage denominator/model and
   measurement migration are reviewed. AST decisions and LLVM code-region ratios
   cannot silently replace the historical MIR series. Thresholds, requiredness,
   ratchets and baselines remain unchanged.

The concrete compiler capability request is stable source-coverage records for
these generated functions, with unambiguous invocation/configuration/target-bound
ownership and explicit zero-execution records for emitted unexecuted functions;
derived methods additionally need a supported instrumentation path. Reproduce with:

```sh
python3 tools/quality/rust-stable-collector/diagnose_macro_spans.py \
  --toolchain 1.98.1 --output target/repro-macro-198 \
  --llvm-cov "$LLVM_COV" --llvm-profdata "$LLVM_PROFDATA"
python3 tools/quality/rust-stable-collector/diagnose_derive_coverage.py \
  --toolchain 1.98.1 --output target/repro-derive-198 \
  --llvm-cov "$LLVM_COV" --llvm-profdata "$LLVM_PROFDATA"
```

Here LLVM_COV/LLVM_PROFDATA point to the matched 1.98.1 LLVM tools, not arbitrary
PATH tools. These commands use existing pinned fixtures; no business rewrite,
nightly expansion, unstable compiler helper or new user tracing tool is proposed.
A candidate compiler fix must first recover real owners/counters for different
invocations/features and execution outcomes, then test unexecuted/nested/duplicate
owners and reject wrong mappings before collector adoption. General macro support
is not implied. For these two capability requests, **submitted: no; merged: no;
adopted fix version: none**. This is a prepared request and reproduction, not an
upstream submission. The separately tracked attribute-macro PR is not an adopted
fix for these fixtures.

The successful current-head CI supersedes the old request for a permitted build/
runtime trace and the earlier local diagnostic setup limitation. Neither a passing
diagnostic nor its trace creates the missing generated owners. No global toolchain
default was changed here.

The smallest remaining T4 action is a compiler capability request using the pinned
function-like and built-in Clone reproductions above: expose real, distinct
generated-function owners and counters (including emitted unexecuted functions)
through stable coverage, with a supported derive instrumentation path. The
function-like internal cause is still unproven; the derive exclusion is
source-confirmed. There is no justified third-party parser patch or adopted stable
compiler fix. Submission, merge and adoption remain separate pending states.

T6 separately requires explicit review of the same-owner code-region denominator
for CRAP and the historical-to-AST measurement transition. No authority to adopt
that model, change requiredness or reset a baseline is inferred. Full same-binary
Linux acceptance and protected signature/release review also remain open. GH-259,
T3–T8 and the release hold stay open; no completion/merge handoff is declared.
