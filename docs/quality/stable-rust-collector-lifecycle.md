# Stable candidate offline lifecycle

This is partial T5 implementation, not an approved distribution or installation
procedure for a user project. The [validation record](stable-rust-collector-validation.md)
separates real RSA/transaction checks from mocked Sigstore behavior. The legacy
release hold remains. No candidate package or test key authorizes production use.

## Program and bundle contract

The same Rust executable performs `release-verify`, `install` (also upgrade) and
`rollback`. It does not invoke Python, OpenSSL, a shell installer or a compiler.
The only verification subprocess is the host-pinned external `cosign` executable.
Its user-owned dependency installation is separate from collector upgrades.

A local flat bundle contains exactly these regular files, with no symlinks or
extra archives:

| Asset | Purpose |
| --- | --- |
| `harness-gate-rust-stable-collector` | Our x86_64 Linux GNU ELF program |
| `LICENSE` | Authenticated collected notices; production license review pending |
| `support.json` | Typed `rust-stable-support/v1` candidate observations and capability limits |
| `release-inventory.json` | `rust-stable-release-inventory/v1`: version, target, and `files` mapping the three payload names to `{sha256, bytes}` |
| `release-inventory.sig` | `rust-collector-signatures/v2`: base64 `rsa_signature` and nonempty `sigstore_bundle` |

RSA signs the exact inventory bytes using SHA-256/PKCS#1 v1.5 with a 2048–8192-bit
SPKI PEM public key. Sigstore independently verifies those same bytes. JSON
duplicate keys, trailing data and unknown envelope fields are rejected. Payload
hash/length, exact inventory, target and ELF architecture are checked before
selection. The typed `rust-stable-support/v1` document must bind the release,
program and license identities and declare the exact candidate series, unsupported
function coverage/CRAP and `candidate-review-required` status. Unknown fields,
empty/duplicate acceptance anchors and overstated capabilities fail even when the
inventory has a valid signature. Observations identify actual acceptance records;
they do not authorize a measurement migration or imply untested platform support.

## External trust and invocation

Trust is supplied independently of both the bundle and installation root. The
caller provides the trusted SHA-256 of a `rust-stable-release-trust/v1` JSON file:

```json
{
  "schema": "rust-stable-release-trust/v1",
  "public_key": "/absolute/host-trust/release-public.pem",
  "public_key_sha256": "<approved SHA-256>",
  "cosign": "/absolute/host-tools/cosign",
  "cosign_sha256": "<approved SHA-256>",
  "trusted_root": "/absolute/host-trust/trusted-root.json",
  "trusted_root_sha256": "<approved SHA-256>"
}
```

These are placeholders, not provisioned trust. Release assets cannot choose the
key, verifier, trust root, workflow identity or OIDC issuer. All paths must be
absolute and canonical. The command rechecks host pins and the verified snapshot
after external verification. A missing verifier produces an error naming its path;
there is no automatic tool installation or unsigned fallback.

The existing repository release contract selects cosign 3.1.3. Follow the upstream
[installation instructions](https://docs.sigstore.dev/cosign/system_config/installation/)
and the independently reviewed host trust manifest when provisioning that dependency.
This workspace has no actual cosign binary, so no version is yet certified for the
new Rust path. Trust bootstrap and actionable production download commands remain
an explicit acceptance item; this document supplies no unauthenticated bootstrap.

The fixed invocation is `cosign verify-blob --bundle ... --trusted-root ...
--offline --certificate-identity
https://github.com/musutrade/Harness-Gate/.github/workflows/rust-collector-release.yml@refs/heads/main
--certificate-oidc-issuer https://token.actions.githubusercontent.com ...`.
Verification has a 60-second deadline and a fresh private HOME; nonzero exit,
timeout, changed inputs or changed tool identity abort the transaction. Actual
Sigstore signature/inclusion verification remains to be exercised with protected
release evidence. A successful mock subprocess only checks argument/exit handling.

Given independently approved inputs, the candidate command shapes are:

```text
harness-gate-rust-stable-collector release-verify BUNDLE TRUST TRUST_SHA256 NEW_LOG
harness-gate-rust-stable-collector install BUNDLE TRUST TRUST_SHA256 ROOT NEW_LOG
harness-gate-rust-stable-collector rollback ROOT INVENTORY_SHA256 TRUST TRUST_SHA256 NEW_LOG
```

`ROOT` must already exist, belong to the invoking user and disallow group/world
writes. `NEW_LOG` must be fresh. These commands operate only on explicitly supplied
paths. No system package, default toolchain, Core baseline or project binding is
modified.

## Transactions and accounting

A nonblocking exclusive lock serializes each installation root. The program copies
bounded regular assets into private staging, authenticates them, syncs each file
and directory, and moves the version to `versions/<inventory-sha256>`. Existing
versions must match the complete authenticated snapshot and safe permissions.
Only then does it atomically rename a relative symlink to `current`. A failure
before that selection preserves the previous version. If the final directory
sync fails after selection, the error explicitly reports that selection committed
and requires inspection; it does not pretend the old version is still selected.

Rollback uses an explicit inventory digest and re-verifies the stored release with
the supplied host trust. Missing, corrupt or nonexecutable targets fail without
changing selection. The previous versions are retained. A process killed during
verification can leave unselected staging directories; automatic cleanup is not
implemented. The test records their disk usage separately from retained versions.

Reports include actual package/install bytes and selection identities. The current
input is a local directory: network download bytes are zero for these operations,
not a measurement of a future downloader. No compiler archives or acceptance logs
are package assets. Protected signing/publication, production license review,
download/cache accounting, trusted bootstrap and full independent upgrade acceptance
remain open.

## Candidate package preparation

Repository maintainers can run `prepare_release.py` with an explicit supported
stable build toolchain, a workspace build directory and SHA-256-pinned acceptance
summaries for the exact resulting binary. It performs locked offline builds and
rejects acceptance for a different binary or failed checks. The preparation script
is repository automation; it is never included in or called by the Rust plugin.

Preparation verifies every cached crates.io archive against `Cargo.lock`, checks
all unpacked source bytes against that archive before and after the build, and
rejects untracked files, symlinks, unknown sources or missing license notices. It
preserves notices verbatim for the conservatively filtered Cargo dependency graph
(including build dependencies), plus the build toolchain's Rust library copyright
notice. It requires these existing maintainer inputs and downloads no environment.
This is an auditable license inventory; production license review remains required.

```bash
python3 tools/quality/rust-stable-collector/prepare_release.py \
  --output target/candidate-package \
  --target-dir "$PWD/target/stable-build" --toolchain 1.97.1 \
  --acceptance target/candidate-acceptance/summary.json '<reviewed SHA-256>'
```

The fresh output contains `unsigned-package/` with exactly the executable,
`LICENSE`, `support.json` and `release-inventory.json`. Build logs and
`preparation.json` stay outside this payload. The required signature envelope is
absent, so the installed verifier rejects this package. No unsigned fallback,
release workflow activation or production signing occurs. A reviewed protected
signing step must eventually supply both signatures over the exact inventory bytes.

The lifecycle suite consumes this actual prepared program/license/support payload.
It re-signs test metadata with a repository-generated RSA key and uses an explicitly
mocked Sigstore verifier. Its disk accounting includes the collected notices, but
its signature envelope and two metadata versions remain test inputs. It is not
production signing or a demonstrated upgrade between two different program builds.
