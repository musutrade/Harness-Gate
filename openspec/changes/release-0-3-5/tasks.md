# Tasks: Release Harness-Gate 0.3.5

**Parent:** [proposal.md](proposal.md)
**Status:** In progress; implementation is prepared and operational evidence
is pending.

- [x] Narrow the release artifact download glob and add the workflow regression
  contract.
- [x] Synchronize package metadata, lockfile, CLI version assertion, snapshot,
  changelog, project status, and documented release commands.
- [x] Run focused local release and package validation.
- [ ] Create the release pull request and pass every required PR check.
- [ ] Merge the pull request, wait for protected `main` CI, and create the
  immutable `v0.3.5` tag at that exact commit.
- [ ] Approve the protected publication environment and verify every release
  job completes successfully.
- [ ] Download and verify the published inventory, checksums, signatures,
  certificates, provenance, and clean-environment installer flow.
- [ ] Record the final tag, workflow, release URL, and verification evidence,
  then mark this release record implemented.
