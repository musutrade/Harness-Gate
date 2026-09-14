# Generated owners from a real source file (stable alternative)

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

## What is certified and what is not

- Certified by the collector: generated source files that are authenticated
  compiler inputs are analyzed by `rust-llvm-exact-free-owner-generated/v1-candidate`
  and exported to `generated-owners.json`. Each function carries a real owner,
  execution count and code-region ratio, and each owner is bound to the file
  digest and the producing invocation. `describe` exposes them as owners with
  `"generated": true`, and `verify` recomputes them so forged counts, a swapped
  producer or a dropped owner fail with `generated owner facts differ from
  recomputed facts`.
- Not certified: the proc-macro **token-stream** path stays `unsupported` for
  coverage and CRAP. Nothing here inherits a calling function's count, fills a
  zero by default, or promotes generated coverage/CRAP to the required gate.
- The alternative changes the required boundary. Adopting it as a required
  metric (thresholds, requiredness, CRAP, baselines) is a reviewed change
  requiring Core's agreement and an accepted measurement migration. This
  candidate exposes certified owners; it does not set the gate.

`tools/quality/rust-stable-collector/validate_generated_owners.py` is the
repository-only regression (11 checks): tests and raw coverage in both
configurations, doctor/prepare/collect, the certified-owner export, the recompute
`verify` path, and three tamper cases. It is not shipped in the plugin payload.

