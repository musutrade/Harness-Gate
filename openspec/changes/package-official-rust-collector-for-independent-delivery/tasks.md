# Implementation tasks

Parent: [proposal](proposal.md), [design](design.md), [delivery requirements](specs/rust-collector-delivery/spec.md).
All tasks remain unchecked. Planning approval is not implementation or publication authorization.
Each item is estimated 1–3 focused hours (M); split before execution if larger.
Acceptance is the concrete output/test named in each item; logs must retain failures.

## P0 Contracts

- [ ] P0.1 [priority:P1] [effort:M, <=3h] Inventory existing protocol, native exits and authority boundaries; retain source references.
- [ ] P0.2 [priority:P1] [effort:M, <=3h] Define typed measurement success/error and legacy-exit compatibility tests.

## P1 Identity

- [ ] P1.1 [priority:P1] [effort:M, <=3h] Specify manifest fields and exact compatibility matrix; unknown combinations reject.
- [ ] P1.2 [priority:P1] [effort:M, <=3h] Specify relocation negatives and reviewed series transition requirements.

## P2 Standalone runtime

- [ ] P2.1 [priority:P1] [effort:M, <=3h] Inventory Python imports, frozen helpers and runtime/license closure.
- [ ] P2.2 [priority:P1] [effort:M, <=3h] Implement private-runtime launcher and command contracts with no source-tree dependency.
- [ ] P2.3 [priority:P1] [effort:M, <=3h] Separate measurement-only results from legacy policy-related exits; retain regression parity.

## P3 Build

- [ ] P3.1 [priority:P1] [effort:M, <=3h] Pin compiler/runtime/dependency inputs and document verified host ABI.
- [ ] P3.2 [priority:P1] [effort:M, <=3h] Build inventoried Linux bundle; compare two builds and document any nondeterminism.

## P4 Lifecycle

- [ ] P4.1 [priority:P1] [effort:M, <=3h] Implement verified staging/atomic activation and interrupted-install tests.
- [ ] P4.2 [priority:P1] [effort:M, <=3h] Implement isolated version selection, rollback and manifest-owned uninstall tests.

## P5 Supply chain

- [ ] P5.1 [priority:P1] [effort:M, <=3h] Define protected independent-tag eligibility, SBOM and provenance subjects.
- [ ] P5.2 [priority:P1] [effort:M, <=3h] Implement exact inventory verification and tamper/missing/extra asset rejection.

## P6 Integration

- [ ] P6.1 [priority:P1] [effort:M, <=3h] Bind installed collector through existing generic configuration and trusted requests.
- [ ] P6.2 [priority:P1] [effort:M, <=3h] Test Core evaluation, unsupported capabilities and no duplicate producer/fallback launches.

## P7 Acceptance

- [ ] P7.1 [priority:P1] [effort:M, <=3h] Run real clean-host fixture capture and full retained-binary re-export.
- [ ] P7.2 [priority:P1] [effort:M, <=3h] Run relocation, malformed output, stale context, wrong tools and missing owner negatives.
- [ ] P7.3 [priority:P1] [effort:M, <=3h] Measure cold/warm costs and durable artifact completeness; retain actual logs.

## P8 Handoff

- [ ] P8.1 [priority:P1] [effort:M, <=3h] Run applicable local checks, strict OpenSpec, docs and required CI; retain results.
- [ ] P8.2 [priority:P1] [effort:M, <=3h] Obtain controller release approval and publish/verify exact RC assets.
- [ ] P8.3 [priority:P1] [effort:M, <=3h] Deliver immutable RC receipt to Arc-Admin owner; record separate GH-215 acceptance/stable-promotion condition.

## Planning acceptance

- [ ] [priority:P0] [effort:S, <1h] Strict OpenSpec validation and applicable repository documentation checks pass with retained commands/results.
- [ ] [priority:P0] [effort:S, <1h] Controller reviews planning PR; no automatic merge or symphony-ready label.
