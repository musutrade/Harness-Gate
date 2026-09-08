# GH-151: Rust authority-transfer candidate

OpenSpec tasks 6.1–6.3 are **not yet accepted**. The opt-in shadow workflow and
released-package integration are implemented for review. Authority transfer is
pending hosted shadow and required CI acceptance of the exact candidate SHA.
Do not merge this candidate or use its results to replace existing required CI.

The predecessor is complete: GH-150 PR [#157](https://github.com/musutrade/Harness-Gate/pull/157)
merged as `9a73ae0f7c61d804fbfc2fe35eac8377eb1fc22e` after required CI succeeded
on `4bc5d5bd6486e47458666ca3c516a1d909268c89`. Its
[Required Quality Aggregate](https://github.com/musutrade/Harness-Gate/actions/runs/34231377223/job/102081158898)
and [Generic Quality Shadow](https://github.com/musutrade/Harness-Gate/actions/runs/34231377223/job/102081158943)
are green. That existing shadow job projects Python reference results; it is not
evidence that the new Rust shadow workflow has run.

## Implemented candidate

The `harness-gate` package now includes the generic Rust library and its embedded
schemas. The unpublished `harness-gate-quality-core` workspace package becomes a
compatibility facade for the migration comparator, which uses the same library
as the executable. Publishing the CLI does not require publishing that facade.

The candidate exposes `quality shadow`; the transfer commit will enable
`quality evaluate` only after hosted acceptance. The entry point consumes existing contracts with trusted
context supplied separately from collector records:

```bash
harness-gate quality shadow \
  --project project.json --policy policy.json --evidence evidence.json \
  --expected expected.json --source-root source --artifact-root artifacts \
  --now 2026-09-08T00:00:00Z --output project-report.json
```

`--selection`, `--mappings`, and `--exceptions` accept the existing selection,
lineage and exception documents. Baseline comparison requires all of
`--base-project`, `--base-evidence`, `--base-expected`, `--base-source-root`, and
`--base-artifact-root`. An omitted clock uses UTC now. The command does not
discover a project manifest or manufacture expected collector/source identity.

The output is the existing `harness-project-report/v1` document. Its legacy
`mode: "shadow"` field is preserved byte-for-byte in the compatibility contract;
the field is not a release-approval signal. Exit 0 requires aggregate `pass`;
policy failures and evaluation errors exit 1, and invalid CLI arguments exit 2.
An evaluation error removes a prior report rather than leaving a stale pass.
Input/output document aliases are refused. No collector, Python interpreter or
Python C-class module is invoked by evaluation or reporting.

External collectors retain protocol v1 and measure only. The Rust library owns
validation, policy requiredness, aggregation, debt/trend, relationships and report
indexes. The frozen oracle, external schemas, collectors and goldens are unchanged.
Task 7's final Python dispositions remain outside this issue.

## Opt-in shadow evidence

The separate `Rust Generic Shadow` workflow runs on manual dispatch, or on PRs
and main pushes when repository variable `RUST_GENERIC_SHADOW` equals `true`.
Same-repository PRs from `symphony/GH-151` also opt in, as authorized by the
operator; no repository-variable write is required.
It builds the product binary and migration comparator, then runs:

```bash
python3 tools/quality/fixtures/generic-core/authority.py \
  --harness-gate tools/harness-gate/target/debug/harness-gate \
  --rust-replay tools/harness-gate/target/debug/examples/differential_replay \
  --output target/quality/rust-generic-shadow
```

Use fresh output directories. With `CARGO_TARGET_DIR`, substitute that directory's
binary paths. The driver verifies the frozen corpus, saves complete Python and
Rust outputs, retains every field difference, and checks the installed command's
report and exit status with an empty executable search path. Both ecosystems and
the consolidated negative matrix run without recollecting native evidence.
The only classified diagnostic variation is GH-150's missing-file OS wording;
original strings and logical paths remain retained. Any unexplained difference
fails the job. Acceptance artifacts explicitly declare `authoritative: false`.

An `always()` artifact upload retains both engines, CLI reports, stderr, field
differences and materialized evidence for 30 days. Download evidence before that
retention window expires if it is needed for a later review. The existing
`.github/workflows/ci.yml` is unchanged, including the `Required Quality Aggregate`
name, dependencies, thresholds and release authority. Branch protection and repository variables remain unchanged. Each artifact
includes the exact checked-out PR head SHA and hosted run identity.

## Explicit transfer decision: pending hosted acceptance

The operator authorized versioned measurement extensions on 2026-09-08.
`production-source-2` includes the linked core as an eleventh blocking production
boundary, including production outside `src/`. Wire declarations without LLVM
counters are source-hash pinned; no counters are invented. The unpublished
comparator facade is migration tooling, while its linked library remains production.
`mccabe-rust-3` / analyzer 0.3.0 measures all 23 declared production source paths,
with explicit absent-source records for predecessor files. Both revisions use the
same analyzer, instrumentation and thresholds. Every measured production symbol
must join real LLVM counters. Historical debt retains syntax-based lineage.

Local validation and exact candidate/transfer commit identities will be recorded
alongside hosted evidence. Transfer is not accepted from local or predecessor
results. Acceptance requires both ecosystem corpora, the negative matrix, hosted
Rust shadow and every existing required check to pass with no unresolved mismatch.
The existing required aggregate retains its name, dependencies and authority.

Earlier failed boundary and read-only Git diagnostics in `validation.json`,
`validation.tar.gz` and `validation-retry.tar.gz` are historical records. Current
operator authorization permits these measurement extensions and writing this
workspace's Git metadata; staging has been verified. They are not current blockers.

## Rollback

Before transfer, disable `RUST_GENERIC_SHADOW`, remove the GH-151 branch opt-in,
and stop manual dispatches. Existing
required CI remains authoritative; preserve downloaded differential artifacts.

After a future accepted transfer, revert the transfer commit(s) and redeploy the
previous accepted release, restoring its recorded authority path. Keep the
frozen Python reference, corpus, original evidence, baselines and source/series
identities. Re-run required CI before releasing the rollback. Do not interpret a
Rust error as permission to fall back automatically to favorable Python output.
Any later re-transfer repeats corpus, negative-matrix and hosted acceptance.

This checkout has no `.harness-gate/flow.toml` declaring a `ci` profile.
`harness-gate config check` and `harness-gate verify --profile ci --all` are not
applicable and were not run. Task 7 and full-proposal acceptance remain outside
this issue. The controller owns merge and issue closure.
