# Design

## Evidence and ownership
Pin source revision and hash every input. Retain compiler identity, target, cfg,
features, compile arguments, expansion provenance, binary, raw profile and LLVM
export. Compiler expansion is required for task_local, macro_rules, derives and
conditional compilation: zero source AST functions does not imply zero code.
Explicit tracing/select/metrics/anyhow/try_join syntax traversal supplies source
provenance only, never a runtime completeness certificate.

Map each LLVM function instance and every region to an observed function, closure,
async body or generated owner. Preserve one-to-many origins and instance identities.
Merge only proven instances; reject duplicate regions, ambiguity and missing
functions. A parent counter cannot supply an unobserved closure's coverage.
Retain excluded test owners and raw regions so production selection is verifiable.

## Independent measurement
Production mapping, selection, complexity and normalization rules form a new
versioned native series with toolchain and build configuration. Never emit
llvm-file-summary-unfiltered/1 for filtered production metrics. Compute line and
region coverage from raw counts and CRAP as exact rationals. Compare only identical
semantic series; preserve historical evidence and baselines unchanged.

## Fail-closed boundaries
Unsupported expansion, cfg, unresolved ownership, tampering and incompatible tools
or series are measurement errors without fabricated metrics. A successful measure
may fail unchanged thresholds. Real minimal compiled fixtures establish only their
bounded mechanism; synthetic mutations test rejection, not native success.

## Rollout
No authoritative collector switch. N0–N6 evidence and remaining gaps are reviewed
in a PR. Controller owns CI, acceptance, merge and issue closure. GH-215's complete
workflow/API/frontend/baseline acceptance and GH-221 remain separate.

## Prototype boundary and unresolved acceptance
`tools/quality/rust_native.py` is an experimental, opt-in single-file rustc adapter,
not the requested complete backend collector. It pins rustc commit
`8bab26f4f68e0e26f0bb7960be334d5b520ea452` and LLVM export schema 3.1.0.
InstrumentCoverage MIR regions are joined by exact spans and demangled free-function
identity; hygiene contexts distinguish separate macro instantiations. Aggregate
MIR supplies an additional omission check. LLVM is re-exported from the retained
binary and raw profile before accepting counts. Archived programs are not executed.
The trusted manifest anchors source, tool, command and raw artifact bytes; this is
integrity evidence, not cryptographic attestation of an arbitrary external producer.

The experimental CRAP complexity is E−N+2 on the normal MIR CFG with a common exit;
unwind, cleanup, imaginary and coroutine-drop edges are excluded. It is a distinct
rule from the established source McCabe series. Per-owner line coverage treats a
line as covered when any of its owned code regions has a positive counter; per-owner
regions are summed across verified generic instances. Regions from different
owners remain distinct even when source spans overlap. Global line coverage unions
source lines. CRAP is CC²×(1−covered_lines/lines)³+CC as an exact rational.

This fixture uses a normal rustc binary build, not a Rust test harness. Its cfg(test)
module is absent from both expanded code and MIR. It does not establish how to
exclude test-owned regions from arbitrary Cargo integration-test binaries. The
saved hygiene dump gives context chains but lacks complete invocation-span edges.
Reports therefore explicitly set `source_provenance_complete=false` and
`backend_complete=false`, even for a successful bounded LLVM join.

The real backend audit finds uninstrumented generated functions and ambiguous MIR
display names. Resolving these requires stable compiler definition identities,
complete generated-code instrumentation and Cargo target/dependency ownership.
Historical debt/delta integration also remains unimplemented. None of these gaps
is excused by the fixture passing or by syntax inventory improvements.
