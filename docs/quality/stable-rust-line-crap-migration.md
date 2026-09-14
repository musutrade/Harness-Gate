# Stable line coverage and Core CRAP migration preview

This change preserves policy semantics: thresholds, requiredness, baseline/debt
and ratchet rules are unchanged. It adds a candidate line measurement and a Core
migration preview. The preview is not evidence, a gate result, a series approval
or a baseline adoption. Required `risk.crap` remains blocked. GH-259 and the
legacy publication hold remain open; PR #261 remains draft.

## Candidate line contract

`rust-llvm-exact-free-owner/v4-candidate` extends the verified root/inline-module
and real generated-file owners with `coverage.line`. The collector authenticates
source bytes, requires one exact LLVM function owner, excludes explicit test
owners and retains the original region/function metrics. Every counted region
must be a nonempty, single-file code region with source coordinates inside those
bytes. Crossing or ambiguous regions fail measurement.

A sweep of each function's region endpoints reconstructs its own line counters.
Nested regions replace enclosing counters. The line counter is the maximum of
the count carried from the preceding line and actual region entries on this line,
following the bounded code-only semantics of LLVM 22.1.8
[`LineCoverageStats`](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/llvm/lib/ProfileData/Coverage/CoverageMapping.cpp#L1459).
Blank/unmapped brace lines do not acquire invented executable counts. Two
functions on one physical line retain independent counts, as do byte-identical
generated files at different paths. No parent or file total supplies an owner's
coverage.

When LLVM exports a file view, the union of all individual owners (including
excluded test owners for this reconciliation only) must reproduce both its
segment-derived line counts and its line summary. Generated files without an
exported file view retain their independent authenticated function-region proof.
Their serialized line ratios and per-line counters are recomputed during capture
verification and Core source export. This remains the existing bounded
real-`include!` alternative; it does not certify arbitrary macros, derive, async,
closures, generics or untracked build-script inputs.

## Core-owned review calculation

`quality::risk::preview_line_crap(records, context, "crap-line-1")` first performs
Core's ordinary evidence/source/artifact/series/context validation. Both supported
inputs must come from the same function record: source cyclomatic complexity and
executable line coverage. It computes the existing line formula exactly:

`CC² × ((total − covered) / total)³ + CC`

The result is an exact reduced rational. Invalid/zero denominators, out-of-domain
inputs and arithmetic overflow return errors; missing or unsupported inputs
produce an unavailable preview with no numeric value. Region substitution and
stale inputs fail. The result schema is `core-crap-migration-preview/v1` with
`authoritative: false`, `state: review_required`, and `baseline_adopted: false`.
It cannot be passed off as a `harness-evidence/v1` metric or required gate result.

| Real fixture owner | CC | Lines | Region coverage | Core line CRAP preview |
| --- | ---: | ---: | ---: | ---: |
| plain `classify` | 3 | 5/5 | 6/6 | 3 |
| partial `classify` | 3 | 4/5 | 5/6 | 384/125 = 3.072 |
| plain/partial `never_called` | 1 | 0/3 | 0/3 | 2 |
| generated `unexecuted` | 2 | 0/1 | 0/5 | 6 |

Generated fixtures have one-line function bodies. Their line fraction does not
measure every branch; the independent region metric remains necessary. The Core
acceptance driver checks authenticated transport, exact values, missing owners,
wrong model, stale evidence and incompatible old series, while still asserting
that required CRAP blocks. No collector decides the CRAP formula or threshold.

## Explicit migration review

The v4 normalization has a different executable digest and therefore a different
complete Core series ID. Core rejects direct comparison with v3 or archived MIR
series. No current project binding is changed. The original GH-220 archive,
member hashes, absolute workspace anchor and incomplete-provenance flags remain
untouched; the current-stable comparison executes only a new capture of identical
source bytes and records its different Cargo build boundary.

Review must approve the source decision model, exact function/line scope and
candidate series before promoting any required CRAP claim. The broad historical
fixture still contains uncertified owners and cannot provide an equivalent
required replacement. Built-in derive and proc-macro token-stream coverage need
stable compiler capability or an explicitly accepted source alternative. Keeping
those claims blocking is part of the proposed transition, not a baseline reset.

## Linux and signing acceptance

`validate_linux_suite.py` runs the current capture/negative suite, macro and
real generated-source cases, all nine signed Core fixtures, the historical probe,
installer/upgrade/rollback and HTTPS failure/interruption suite with the supplied
same-binary package. It records the OS, executable hashes, commands and original
raw-output identities. Containers establish another runnable userspace and share
the host kernel; they do not establish a second kernel or all Linux support.

The lifecycle suites use real test RSA with explicitly mocked Sigstore. The
separate pinned cosign 3.1.3 upstream positive demonstrates actual Sigstore
signature/inclusion verification under its own identity; it does not sign or
certify this collector. Candidate production acceptance still needs both
signatures over its exact inventory, the protected Harness-Gate workflow
identity, independently approved trust and license/release review. This change
cannot remove the hold or publish a production version.
