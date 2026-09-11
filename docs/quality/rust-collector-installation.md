# Rust collector installation and independent release

GH-231 prepares a separately authenticated private-Python installer capsule and
a production RSA/Sigstore verifier for review. The six-asset publication workflow
is a non-active template. See the [preparation and approval packet](gh-231/preparation.md)
for actual unsigned RC bytes, trust provisioning, checks and remaining blockers.
No collector RC or clean-host production installation is certified by this work.
The v1 source-install instructions below describe the earlier fixture path;
production bootstrap requires v2 trust and both inventory signatures.

GH-229 implements OpenSpec P4.1–P5.2. The predecessor is accepted PR #234,
merged at `9bdc203c21a75cc769fdb2adbafb897e4b96a30e`. This is an offline lifecycle
and nonpublishing release rehearsal. [Validation](gh-229/validation.md) records
actual results. P6 integration, P7 native clean-host acceptance and P8 release
approval remain separate; the production compatibility matrix is still empty.

## Host trust bootstrap

The trusted computing base is the host OS, a host-managed Python 3.11+ runtime,
the reviewed installer source and its imported contract/schema files, and a
host-managed OpenSSL executable and its dynamic dependencies. Install those
through an independently authenticated administrator channel before inspecting
collector assets. Never run an installer, Python, OpenSSL or key downloaded from
the collector release as its own verifier. The collector's bundled Python is
usable only after verification and is not this bootstrap runtime.

The current source entry is `tools/release/install_collector.py`; its release
modules, imported `tools/quality` modules and schemas must all come from the same
authenticated reviewed source tree. Preserve their relative paths. This issue
did not ship an independently authenticated standalone installer
executable. GH-231 now builds the private runtime capsule, but its independent
distribution and clean-host acceptance remain pending. Before claiming
installation on a host without Python or a source
checkout, P7/P8 must provide and verify an OS-packaged bootstrap runtime or a
separately authenticated standalone installer. Downloading the collector's own
runtime first does not satisfy that prerequisite.

An administrator provisions a JSON trust file outside the asset directory:

```json
{
  "schema": "rust-collector-host-trust/v1",
  "openssl": "/absolute/host/openssl",
  "openssl_sha256": "<independently verified executable SHA-256>",
  "public_key": "/absolute/host/collector-release-public.pem",
  "public_key_sha256": "<independently verified public-key-file SHA-256>",
  "rsa_signature_bytes": 256,
  "host_libraries": {"<reviewed logical library identity>": "/absolute/host/library"}
}
```

The example is a template, not usable trust. Pin an RSA public key and its actual
signature width (256 bytes for RSA-2048). Only SHA-256 RSA signatures are used;
the installer rejects trailing signature bytes even when OpenSSL accepts them.
Pinning the OpenSSL executable does not authenticate its loader, shared libraries
or the host Python runtime: those remain administrator-managed host trust.
Key rotation requires an independently reviewed trust-file update, never a key
supplied by a new release. No production private key is created by this issue.

The administrator also supplies the reviewed logical-name-to-library-path map.
The installer hashes those host files and compares the resulting inventory
fingerprint, `Linux`/`x86_64`, glibc string and kernel release with the exact
manifest ABI. A different kernel, library identity or digest fails before
sampling; there is no inferred distro-wide support. This host map must come from
the eventual certified ABI receipt, not from untrusted release instructions.

## Exact release assets and eligibility

The release directory contains exactly these six regular, single-link files:

| File | Binding |
| --- | --- |
| `collector.tar` | Uncompressed private runtime archive |
| `manifest.json` | Existing strict delivery manifest, all payload hashes and identities |
| `sbom.spdx.json` | SPDX 2.3 file inventory of every manifest payload and its SHA-256 |
| `provenance.json` | in-toto statement covering archive, manifest and SBOM |
| `release-inventory.json` | Exact ordered names and SHA-256 subjects for all four files above |
| `release-inventory.sig` | Detached RSA/SHA-256 signature of the inventory's exact bytes |

The signature binds the complete inventory; the inventory binds the attestation
and other subjects. The attestation excludes itself and the inventory to avoid
a digest cycle. Missing, unsigned, duplicate, extra or tampered assets fail.
The manifest binds its runtime dependency and license inventories; their bytes
and all shipped license notices are also ordinary SBOM payload subjects. File
licenses are `NOASSERTION`, not an invented legal clearance. P8 must review the
retained notices and redistribution obligations before any publication.

`collector_release_policy.py` requires the canonical repository, an exact
`rust-collector-vX.Y.Z-rc.N` tag (N >= 1), the same manifest version and full source
commit, tag resolution to that commit, and reachability from fetched protected
`origin/main`. It reuses Core's exact-commit successful `ci.yml` push-on-main run
and successful `Required Quality Aggregate` checks. It does not compare the
collector version with Core's Cargo version. Missing/wrong branch, SHA, tag,
workflow or aggregate evidence blocks eligibility. Stable tags remain explicitly
blocked pending separately accepted GH-215 fresh integration.

Eligibility also requires the existing `rust-collector-release` environment to
have required reviewers, prevent self-review and disable administrator bypass.
The receipt records the CI run/job IDs and normalized protection requirements.
It is a signed workflow assertion, not an independently queried approval or an
OIDC certificate. The eventual production signer must run only after this
environment's approval and recheck eligibility immediately before signing. Its
RSA private key must exist only as a secret scoped to that protected environment;
protect workflow edits on main and restrict who can administer that environment.
An unprotected local invocation can construct a receipt but cannot authenticate
it without that separately provisioned production key.

The implemented workflow, `.github/workflows/rust-collector-release.yml`, has
only manual dispatch on canonical main, read permissions, and no production
key, tag trigger or publication command. A preflight checks the existing
environment before requesting approval, so a missing environment cannot silently
become an unprotected release job. Approved execution runs tests and uploads
synthetic rehearsal artifacts. This issue exercises that rehearsal locally,
without requesting hosted approval. P8 must separately authorize any publication,
provision production trust, build real assets, verify the exact inventory and
retain the approved workflow receipt. Existing Core release checks are unchanged.

## Lifecycle

Use absolute paths for a private, administrator-selected installation root and
trust file. The source entry requires the authenticated bootstrap described above:

```bash
python3 tools/release/install_collector.py --root "$PWD/target/collector-install" --trust /absolute/host/trust.json install --release /absolute/verified-download-directory --tag rust-collector-v0.1.0-rc.1
python3 tools/release/install_collector.py --root "$PWD/target/collector-install" --trust /absolute/host/trust.json select --version 0.1.0-rc.1
python3 tools/release/install_collector.py --root "$PWD/target/collector-install" --trust /absolute/host/trust.json rollback --version 0.1.0-rc.1
python3 tools/release/install_collector.py --root "$PWD/target/collector-install" --trust /absolute/host/trust.json recover
python3 tools/release/install_collector.py --root "$PWD/target/collector-install" --trust /absolute/host/trust.json uninstall --version 0.1.0-rc.1
```

Install and select print the exact version's launcher path. `current.json` is the
atomic selection record; it is not an executable shim or a change to global PATH.
P6 connects the selected path through existing generic configuration. No command
installs rustup, changes global tools, collects evidence, or accepts a baseline.

The installer locks a private root, journals owned paths, snapshots the fixed
assets into private staging, then verifies signature, inventory, provenance and
host ABI before manually extracting. It rejects traversal, links, devices,
sparse/PAX metadata, duplicate/missing/extra entries and unsafe modes. It checks
each extracted payload digest and the complete tree before renaming the version
directory and atomically replacing the selection record, with fsync at durable
boundaries. Selection/rollback reverify signatures, ABI, every installed byte and
file mode. An existing version is never overwritten. The full signed assets
remain under each version's `.delivery` directory for offline re-verification;
budget roughly archive plus extracted runtime storage per version, plus staging
for the next install. No size or performance claim is established by tiny fixtures.

Before version commit, interruption leaves the previous selection unchanged;
recovery removes only journal-owned staging. A crash after version rename can
leave a complete unselected version, which explicit selection can reverify and
activate. A crash after pointer replacement leaves the new complete version
selected. Recover is idempotent, including interrupted uninstall. Uninstall
removes only a verified version and its retained delivery files; if active, it
first clears selection. It does not automatically select a fallback version.

Unexpected files or links inside an owned tree stop cleanup without deleting
them. Project captures, original binaries, anchors and unrelated files are never
cleanup targets. Preserve the journal and unexpected bytes for operator review;
move unrelated evidence to a safe project location before retrying recovery.
The private root assumes trusted same-UID administrators and a local filesystem
with atomic rename/flock/fsync semantics; it does not defend against a malicious
root user or certify network filesystems. Tests include actual SIGKILL and
injected interruption boundaries; they are not hardware power-loss tests.

Distribution authentication is not runtime capture authentication. Version or
path relocation establishes no measurement-series equivalence. Released Rust
Core retains requiredness, thresholds, lineage, debt/ratchet and final outcomes;
all accepted coverage/CRAP rules and capability states remain unchanged.

## Nonpublishing local rehearsal

```bash
python3 -m unittest discover -s tools/release/tests -v
python3 tools/release/collector_dry_run.py --output target/gh-229/dry-run
```

The rehearsal creates a disposable RSA key, synthetic payloads, a signed asset
inventory and an explicit report, then installs, selects and uninstalls. Its
private key is discarded; output retains the public test trust and signed bytes.
The report labels production eligibility and protected approval as unevaluated,
native measurement as unperformed, and publication as unattempted. Those test
assets are never production release candidates.
