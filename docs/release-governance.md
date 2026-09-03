# Release Governance

Harness-Gate releases are authorized by the tagged source state, the exact
protected-main CI run, repository tag rules, and the `release` environment.
The release workflow must not be treated as an alternate path around `main`.

## Required Repository Settings

The repository must retain all of these controls:

| Control | Required state |
| --- | --- |
| Protected `main` | Active ruleset requiring pull requests and `Required Quality Aggregate` |
| Version tags | Active tag ruleset for `refs/tags/v*` with update and deletion prohibited and no bypass actor |
| Publication environment | `release`, administrator bypass disabled, explicit reviewer required |
| Deployment source | Custom tag policy `v*`; branches and other tags are rejected |

The environment currently permits self-review because the repository has only
one maintainer. Disable self-review when an independent release reviewer or
team is available.

Current configuration evidence (2026-09-01):

- repository tag ruleset
  [21989651](https://github.com/musutrade/Harness-Gate/rules/21989651) is
  active for `refs/tags/v*`, has update/deletion rules, no bypass actors, and
  reports `current_user_can_bypass: never`;
- environment `release` has ID `20983060444`, administrator bypass is disabled,
  and required-reviewer rule `64256604` names `higoalespn`; and
- deployment policy `58783819` is type `tag` with pattern `v*`.

Latest release evidence (2026-09-03):

- PR [#75](https://github.com/musutrade/Harness-Gate/pull/75) merged as
  `6a9066b7f5dba241a3190a5508727cd21ba2c9b0`; the PR CI run
  [33701746324](https://github.com/musutrade/Harness-Gate/actions/runs/33701746324)
  passed all 21 required checks.
- Protected `main` CI run
  [33702793530](https://github.com/musutrade/Harness-Gate/actions/runs/33702793530)
  passed for that exact commit, including `Required Quality Aggregate`.
- Immutable tag and published release [`v0.3.6`](https://github.com/musutrade/Harness-Gate/releases/tag/v0.3.6)
  point to the same commit. Release workflow
  [33704014615](https://github.com/musutrade/Harness-Gate/actions/runs/33704014615)
  passed eligibility, multi-platform builds, quality gates, signing,
  provenance, GitHub Release creation, and crates.io publication.
- The release inventory and checksum manifest were downloaded and verified;
  the public crate is [harness-gate 0.3.6](https://crates.io/crates/harness-gate/0.3.6).
  Binary and source installer paths were exercised in isolated temporary
  directories. These checks close the v0.3.6 publication record only; they do
  not constitute DevRail staging, shadow/canary, rollback-authority, or R-07
  OS-sandbox acceptance.

## Release Procedure

1. Merge the version, changelog, and release-record change through a pull
   request.
2. Wait for the resulting `main` push CI, including
   `Required Quality Aggregate`, to complete successfully.
3. Resolve the exact `main` commit and create the matching version tag without
   moving an existing tag.
4. Push the tag and confirm `Verify release eligibility` records the expected
   `main` CI run and aggregate job.
5. Review the pending `release` environment deployment, then approve
   publication.
6. Verify the GitHub Release asset set, crate, signatures, attestations, and
   clean-environment consumer procedure before closing the release record.

Example tag creation after the version change is on verified `main`:

```bash
git fetch origin main
git switch main
git pull --ff-only origin main
git tag -a v0.3.7 -m "Release v0.3.7" origin/main
git push origin v0.3.7
```

The literal version is an example. It must match the package version and must
not already exist locally or remotely.

## Automated Eligibility Evidence

`tools/release/release_policy.py` writes `release-policy.json`. A passing file
binds all of the following:

- repository, tag, package version, and tag commit;
- protected branch and fetched main ref;
- exact successful `.github/workflows/ci.yml` push run; and
- the successful `Required Quality Aggregate` job in that run.

The release workflow retains this file as an artifact for 90 days. Missing or
ambiguous Git history, GitHub API failures, a non-main tag, a mismatched
version, a failed CI run, or a missing aggregate job blocks every downstream
release job.

## Recovery

Do not move or delete a published version tag. Correct source or workflow
problems through a new pull request and publish a new version. A failed
environment approval or external signing outage may be retried against the
same immutable tag only when no release or crate was partially published and
the retained run evidence proves the retry uses the same source commit.
