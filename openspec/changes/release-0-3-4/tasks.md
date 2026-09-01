# Tasks: Release Harness-Gate 0.3.4

**Parent:** [proposal.md](proposal.md)
**Status:** In progress; the version change is prepared and real tag-release
evidence is pending.

- [x] Synchronize package metadata, lockfile, CLI version assertion, CLI
  snapshot, changelog, project status, and documented release commands.
- [ ] Run package validation, formatting, linting, tests, OpenSpec validation,
  and documentation consistency checks locally.
- [ ] Create the release pull request and pass every required PR check.
- [ ] Merge the release pull request, wait for the protected `main` CI run,
  and create the immutable `v0.3.4` tag at that exact commit.
- [ ] Approve the protected publication environment after policy and quality
  gates pass; verify the release workflow completes without retries that alter
  the source commit.
- [ ] Download the published asset set and verify the exact inventory,
  checksums, Sigstore signatures/certificates, and GitHub provenance.
- [ ] Run the documented binary and source installer flows in a clean,
  network-isolated environment and retain their output as release evidence.
- [ ] Record the tag, workflow, release URL, asset verification, and clean
  environment results in ADR-0032 and the DevRail capability OpenSpec, then
  mark this release record implemented.
