# Rust source-risk collector (candidate 0.1.0-rc.1)

This is an independent Harness-Gate collector plugin. It does not modify Core,
change a policy threshold, sign its own requests, or replace the MIR measurement
series. The original MIR collector remains useful for generated-code diagnostics.

## Measurement contract

`rust-explicit-decisions` starts each source callable at 1 and adds:

- 1 for each `if`/`if let`, `for`, `while`, `?`, `let ... else`, `&&`, and `||`;
- `max(match arms - 1, 0)` plus 1 per match guard;
- 1 for `matches!`, plus 1 for its optional guard.

An unconditional `loop` adds no decision. Functions, methods, default trait
methods, closures and explicit async blocks have separate owners. Nested owners
are not charged to their parent. Async state machines and generated derive/logging
control flow do not contribute to source complexity. This is a **new series**;
its numbers are not interchangeable with MIR CC or another tool's CC definition.

Supported expression macros are `matches!`, comma-separated expression arguments
of `tracing::{info,warn,error,debug,trace}!`, and `sqlx::migrate!`. User expressions
inside those arguments are visited (including `?`). Unsupported macro grammar,
other macros, item macros, conditional compilation attributes and unrecognized
attributes fail closed. The collector does not pretend to measure arbitrary Rust
macro expansions. Generated derive bodies are outside this source-only scope.

Original source positions are parsed with pinned `syn` 2.0.119. UTF-8 character
columns are converted to LLVM byte columns. LLVM 3.1.0 code-region exports join
only at exact AST entry anchors, never by a function name or basename fallback.
Every inventoried source callable needs a native mapping. Unknown region kinds,
cross-file expansions, duplicate native instances and ambiguous joins are rejected.
All `.rs` files under explicitly approved source roots are inventoried, including
files with only declarations. These declaration files have no function metrics.

Async factory counters do not cover the async body: only its execution/poll
mapping contributes to line, region and function coverage. Equal source ranges
from distinct native instantiations are merged by covered/not-covered status.
Innermost code ranges override enclosing ranges; nested callable ranges are
removed from the parent. A non-whitespace code interval is a line opportunity,
and any covered opportunity covers that line. Unknown gap/expansion region kinds
are currently rejected rather than guessed. This is source region coverage, not
branch coverage or a claim about implicit error/cleanup paths in MIR.

`CRAP = CC + CC² × (1 − covered_source_lines / source_lines)³`, represented as a
reduced exact rational. A missing denominator is not applicable, never zero risk.
Core alone evaluates the required **CRAP ≤ 10** policy. Full coverage therefore
passes CC=10 and still fails CC=11.

## Trust and transport

`capture.py` copies explicitly listed build/test inputs before executing locked
Cargo tests. It retains original binaries and `.profraw` counters, hashes source,
configuration, inputs and native tools, and records the exact toolchain. Capture
is a **local host utility**, not a production signing or isolation service.
A source commit field identifies the baseline; hashes bind actual dirty input
bytes. It is not an assertion that the capture is a clean checkout of that commit.

The collector revalidates those bytes and independently runs pinned
`llvm-profdata merge` and `llvm-cov export`. It does not trust a supplied coverage
summary. Raw native capture files remain in the receipt's retained `raw_root`;
archive this directory with the bundle for replay. Emitted evidence contains the
re-exported LLVM data and capture receipt. Production hosts must protect both
capture provenance and the signing key from repository-controlled tests.

The adapter-v2 bridge validates Core invocation context, signed binding digest,
complete subject/capability claims, and configuration identity. Core authenticates
signatures, rejects stale/replayed requests, validates artifacts and evaluates
policy. A transport `PASS` means collection succeeded, not that quality passed.

Artifacts use an exclusive `artifact_subdir` inside Core's shared output root so
other independent collectors can publish their own directories in the same run.
Existing files in this plugin's directory are rejected. Collectors never issue
quality overrides or contain signing keys.

## Build and use

```sh
cargo build --locked --release --manifest-path ast/Cargo.toml
# Copy the resulting harness-gate-rust-source-inventory binary to ./inventory.
python3 -m unittest discover -s . -p 'test_*.py' -v
python3 plugin.py discover --request /absolute/request.json
python3 plugin.py collect --request /absolute/request.json
```

`capture.py --help` documents snapshot arguments. `acceptance.py --bundle BUNDLE
--plugin PLUGIN_DIRECTORY` runs a backend-only, ephemeral-key Core rehearsal. It
refuses a real checkout and checks signature, stale context, expiration, replay
and artifact tampering. It is not the project's complete production gate.

The candidate currently targets Rust 1.97.1 / LLVM 22.1.6, cargo-llvm-cov 0.9.0,
Python 3.14 and Linux x86-64. New syntax/LLVM formats require compatibility tests
and a new identity before adoption.

References: [LLVM coverage mapping](https://llvm.org/docs/CoverageMappingFormat.html)
and [Rust instrument-coverage](https://doc.rust-lang.org/rustc/instrument-coverage.html).
