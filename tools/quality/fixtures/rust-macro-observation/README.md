# Stable macro observation experiment

GH-259 now consumes the ordinary generator library from the candidate Rust
collector's `observe-macro-template` command. Both callers use the same input
parser. See `docs/quality/stable-rust-macro-observation.md` for the bound observation
schema and real coverage gap. This does not certify general expansion or CRAP.

This unpublished Rust-only fixture demonstrates a normal generator library shared
by a procedural macro and an analysis caller. Analysis visits the same generated
AST instead of maintaining a second expansion implementation. It does not use
nightly, compiler-private APIs, expansion-to-text tools, or a runtime interpreter.

Run from this directory:

```sh
cargo +stable test --workspace --locked
cargo +stable test --workspace --locked --features gate-observation-consumer/branching
```

The two deliberately bounded templates have cyclomatic complexity 1 (identity)
and 2 (one if). These are prototype expectations, not the production measurement
series. Tests compile and execute actual macro output, vary invocation and feature
selection, and retain one unexecuted generated function for subsequent coverage
mapping acceptance. No coverage percentage or CRAP is inferred by these tests.

Before adoption: bind observations to exact dependency/source/input/config/target
identities; distinguish module-qualified and repeated owners; handle or explicitly
reject nested expansions; verify real LLVM coverage mapping, including unexecuted
owners and deliberately corrupted mappings. Token text alone loses hygiene/span
information and must never be used as sufficient identity evidence.

This fixture is not a shipped plugin, general macro expander, upstream library fix,
or completed coverage integration. No production dependencies on vacro or debtmap
are introduced. See issue #259 and the stable-pure-rust-collector design.
