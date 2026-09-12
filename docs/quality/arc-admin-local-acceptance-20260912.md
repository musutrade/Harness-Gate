# Arc Admin local acceptance and private C toolchain repair

Engineering-policy semantics are unchanged. Core requiredness, coverage/CRAP,
baseline/ratchet, signatures and delivery preflight retain their existing rules.

The published Core v0.4.0 binary at SHA-256
`8e3df8303ca8f650d4ef768b29cfefb60ca115122a47b116245cc19e4649bbdd`
ran the execution configuration imported from Arc Admin commit
`2faa1ca6c1a2b1a45956540e97e68a01532a40f7` in a disposable worktree. All 25
configured steps passed, including Rust tests, 85 frontend tests, browser E2E,
and the Angular/Axum/PostgreSQL smoke test. Secrets and audit gates also passed.
The report has `status: PASS`, `passed: true`, `evidence_complete: true` and no
quality object: this proves execution acceptance, not generic-quality approval.
A separate workflow-component run replacing the required hook syntax command
with `false` produced the expected overall FAIL.

## Defect found with the actual project

The existing private collector runtime failed during Arc Admin's `ring` build:
`gcc: fatal error: cannot execute 'cc1': posix_spawnp: No such file or directory`.
The distribution carried GCC's link driver and linker, but no C frontend,
assembler, development headers or archive tools. Single-file Rust fixtures
therefore did not cover a prerequisite of real Cargo dependency builds.

The repair inventories and ships `cc1`, `as`, `ar`, `ranlib`, GCC headers and
package-owned libc/Linux/OpenSSL development inputs, plus private OpenSSL
libraries and pkgconf. The C launcher searches explicit private include roots.
A pkg-config launcher confines package discovery to the private sysroot. These
inputs participate in the existing byte pins, dependency inventory and notices;
no global toolchain or Arc Admin source was changed. New payloads still require
their normal release review before distribution.

## Validation

- The original real Arc Admin capture retains the missing-cc1 failure.
- The added regression fails against the original runtime; the repaired runtime
  compiles C using standard/OpenSSL headers, archives it, links and executes it.
- All 12 standalone runtime tests pass with real private tools and the published
  Core, including signed-request, replay, malformed-response and capture-anchor
  rejection cases. No runtime test was skipped.
- A read-only, network-disabled container with neither system Python nor a C
  compiler performs the C regression and a fresh Rust capture with two identical
  reexports. Independent execve tracing is retained.
- Collector transport: 6 passed; delivery preflight: 4 passed; production helper
  contracts: 12 passed; installation lifecycle: 21 passed.
- Documentation/schema consistency and `git diff --check` pass.

The first exported-source test attempt omitted preset/workflow fixture inputs.
Those files were restored from the same main commit; the affected tests were
rerun successfully, followed by the full runtime and lifecycle runs above. This
was a test setup repair, not a collector behavior change.

## Installation boundary

The real production installer was run with the existing host trust v2 against
the source-bound candidate. It rejected the directory because provenance,
release inventory and release signature assets are absent. Local runtime and
fixture results do not substitute for those files or authorize compatibility.
No production signature, publication or project gate transfer is claimed.

Original logs, native captures, build-input lock, repaired runtime, container
trace and the positive/negative Core reports are retained under
`/mnt/dev-ssd/workspaces/harness-gate-acceptance-20260912/` on the acceptance host.
