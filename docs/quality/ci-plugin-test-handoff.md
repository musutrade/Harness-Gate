# Core CI plugin test handoff

This change transfers only tests whose execution is already demonstrated in
GateProbes. Engineering Policy semantics, required check names, Core platform
coverage, measurement identities, thresholds and release authority are unchanged.

## Accepted handoff

The [receipt](ci-plugin-test-handoff.json) records the exact GateProbes commit,
successful CI run, test paths, execution commands and retained source digests.

| Core execution | GateProbes execution | Evidence |
| --- | --- | --- |
| `test_rust_collector_project` | Native plugin helper tests through `tests/zero_skip.py` | Same eight test methods and bytes; any skip fails |
| `test_rust_native_driver` | Native `accept.py` loads `NativeDriverTests` with the real driver, sysroot and pinned Core | Same ten test methods and bytes; four platforms; any skip fails |
| Standalone `rust-source-risk` AST build/tests and Python discovery | Registry-selected Python plugin job builds its inventory, runs Cargo tests and Python discovery with `RUST_SOURCE_INVENTORY` | Same six Python test files and both AST Rust source files; no platform coverage removed |

The source-risk discovery includes the same static toolchain compatibility
contract as before. The separately invoked expensive dual-toolchain acceptance
was not part of either discovery command and is not newly claimed here.

`Quality Script Tests` uses
[`run_core.py`](../../tools/quality/tests/run_core.py). It first checks the
handoff and retained source digests, then runs every discovered `test_*.py`
module except the two accepted native modules. New modules run by default.
Missing or modified delegated tests or guarded plugin inputs fail selection;
restore local execution or renew the reviewed parity evidence before changing
those retained implementations. The check uses committed evidence, without a
GateProbes checkout or a new network dependency in Core CI.

The original test files remain available: legacy runtime tests still import
fixture helpers from `test_rust_collector_project`. This change transfers
execution and does not delete those dependencies.

## Retained coverage

The fifteen exclusions in `core_isolation_check.py` serve a different purpose:
they define the manually tested Core-only snapshot. They are not evidence that
all fifteen modules have been accepted by GateProbes. Ordinary CI retains the
other thirteen modules, with their individual reasons recorded in the receipt:

- Core preset and generic policy/reference semantics.
- Historical TypeScript/Angular/V8 adapters, archives, normalization and advisory
  behavior. The new TypeScript plugin does not support the same mapped reports.
- Legacy native delivery, bundled-runtime, artifact-path and classification
  contracts without complete replacement coverage in GateProbes.

Core reference equivalence and its `rust-reference` fixture directory remain
Core-owned. All release governance tests remain in Core while its release
workflow still packages four plugins. The remaining release and compatibility
dependencies require their own reviewed migration; this change does not claim
that ordinary Core CI or Core publication is fully independent of plugin source.

## Verification and rollback

Focused tests cover stale source/test digests, missing inputs, malformed or
unpinned handoffs, missing execution evidence and conservative selection of new
tests. Existing CI topology tests continue to enforce the unchanged required
job and platform contract. The fixture ownership test guards the historical
Core Python equivalence inputs as well as Rust fixture references.

Reverting the workflow change restores the original discovery command and
standalone source-risk execution. No historical release, signature or baseline
is changed. Hosted checks on the PR supply the required acceptance; no numerical
CI speed improvement is claimed without a measured comparison.
