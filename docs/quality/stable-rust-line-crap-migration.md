# Stable line coverage and Core CRAP migration preview

This change preserves policy semantics: thresholds, requiredness, baseline/debt
and ratchet rules are unchanged. It adds a candidate line measurement and a Core
migration preview. The preview is not evidence, a gate result, a series approval
or a baseline adoption. Required `risk.crap` remains blocked. GH-259 and the
legacy publication hold remain open; PR #261 remains draft.

The current release contract uses SHA-256 plus one exact-tag Sigstore signature;
see [the lifecycle delta](stable-rust-collector-lifecycle.md#release-simplification-2026-09-14).
RSA references in the recorded acceptance below describe the previous candidate,
and are not current requirements or acceptance of the rebuilt executable.

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

The current lifecycle suites use SHA-256 with explicitly mocked Sigstore. The
separate pinned cosign 3.1.3 upstream positive demonstrates actual Sigstore
signature/inclusion verification under its own identity; it does not sign or
certify this collector. Candidate production acceptance needs one Sigstore
signature over its exact inventory, the Harness-Gate workflow identity bound to
its exact version tag, authenticated trust and license/release review. This change
cannot remove the hold or publish a production version.

## Retained 2026-09-14 acceptance

The [compact acceptance record](stable-rust-candidate-evidence/line-migration-linux.json)
binds the original raw paths and hashes for both complete userspace runs. The
same 3,587,880-byte executable has SHA-256
`9b70b49135da5febfafb2100c78a7dadb7dfefa89b7bf097d2fb9d5f28097ad2`.
Each system passes 135 capture checks, 64 generated-source checks, 26 macro
checks, nine authenticated Core cases, the current-stable historical comparison,
47 lifecycle checks and 29 HTTPS checks. Both systems also pass five actual
cosign checks, including the upstream positive, wrong identity, corrupted
signature, missing transparency evidence and the candidate's rejection of a
valid signature for another payload. The installed candidate remains selected.

The unsigned review package is 6,134,708 bytes: a 3,587,880-byte program,
2,545,341-byte conservative license notices, support metadata and inventory.
The test-signed initial download is 6,135,177 response-body bytes; plugin cache is
zero. Independently built test-version upgrade downloads are 6,135,203 bytes on
Ubuntu 26.04 and 6,134,507 on Ubuntu 24.04. Those upgrade programs were built
separately on their respective systems; the main candidate is identical. The
record includes retained interrupted staging and installation bytes. External
Rust/LLVM/cosign and repository automation are outside the package.

The record retains the earlier failed test expectation, rejected mismatched
package binary, missing Python standard library in the test container and the
external-verifier size check correction. None of those earlier attempts is
relabeled as final-binary acceptance. Protected candidate signing remains open.
