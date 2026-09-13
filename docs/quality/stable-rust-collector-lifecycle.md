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
| `LICENSE` | Required distribution license/notices; production inventory pending |
| `support.json` | Signed support/measurement metadata; production schema pending |
| `release-inventory.json` | `rust-stable-release-inventory/v1`: version, target, and `files` mapping the three payload names to `{sha256, bytes}` |
| `release-inventory.sig` | `rust-collector-signatures/v2`: base64 `rsa_signature` and nonempty `sigstore_bundle` |

RSA signs the exact inventory bytes using SHA-256/PKCS#1 v1.5 with a 2048–8192-bit
SPKI PEM public key. Sigstore independently verifies those same bytes. JSON
duplicate keys, trailing data and unknown envelope fields are rejected. Payload
hash/length, exact inventory, target and ELF architecture are checked before
selection. The production support schema and license inventory still need release
preparation; a signed arbitrary support document is not a compatibility claim.

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
are package assets. Protected packaging, license completeness, download/cache
accounting, trusted bootstrap and full independent upgrade acceptance remain open.
