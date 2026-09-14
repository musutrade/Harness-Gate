# Stable candidate lifecycle

## Current line and migration checkpoint (2026-09-14)

The v4 candidate adds authenticated per-function line coverage, including real
generated files. Core computes an exact `crap-line-1` migration preview from
validated same-owner evidence. The preview has no gate or baseline authority;
required CRAP remains blocked. Source/region reconciliation, mutation checks,
original historical anchors and old/new series rejection remain mandatory.
See [the line/CRAP contract](stable-rust-line-crap-migration.md).


This is partial T5 implementation, not an approved distribution or installation
procedure for a user project. The [validation record](stable-rust-collector-validation.md)
separates real SHA-256/transaction checks from mocked Sigstore behavior. The legacy
release hold remains. No candidate package or test key authorizes production use.

## Release simplification (2026-09-14)

The user-approved delivery delta follows Core: SHA-256 plus one Sigstore signature,
with an exact version tag, verified installation and failure recovery. Versioned
trust v2 and signature v1 replace the unpublished dual-signature candidate contract;
old candidate trust/envelopes must be regenerated explicitly and are rejected by
this installer. Existing legacy releases and their historical verifier are retained
as legacy evidence under the publication hold.

Quality thresholds, CRAP, actual measurement, required gates, baseline/debt/ratchet
and failure blocking are unchanged. Measurement promotion still needs evidence and
review. PR #261 stays draft; this change neither signs nor publishes a release.

## Simplified-contract validation

The [fresh acceptance record](stable-rust-candidate-evidence/single-sigstore.json)
binds the rebuilt executable, inputs and original logs. Rust 1.98.1 / LLVM 22.1.8
passes 25 Rust unit tests, Clippy/format, 135 real capture checks, 51 lifecycle
checks, 29 HTTPS checks and the authenticated partial-coverage Core fixture.
Required CRAP remains blocked; its preview remains non-authoritative. The quality
script suite passes (455 run, 36 optional input-dependent skips); documentation
consistency and strict OpenSpec validation pass.

Five actual cosign checks include an upstream-release positive and collector
rejection of an unrelated signed blob with the selected version preserved. The
positive authenticates upstream cosign only. Lifecycle positive signatures remain
explicit mocks; no collector production signature is claimed. The binary is
3,475,320 bytes, the unsigned package is 5,737,434 bytes, and initial HTTPS response
bodies total 5,737,540 bytes. No package cache or compiler environment is added.

The initial default-toolchain capture used Rust 1.97.1, so packaging correctly
rejected it. The failed preparation log and first capture are retained; the final
package uses the fresh 1.98.1 capture. Prior two-userspace acceptance remains bound
to the previous executable. This validates the delivery simplification on one
system; T3–T8 and production acceptance remain open.

## Program and bundle contract

The same Rust executable performs `release-verify`, `install` (also upgrade),
`download-install` and `rollback`. It does not invoke Python, OpenSSL, a shell installer or a compiler.
Verification invokes the host-pinned external `cosign` executable and then the
authenticated staged program with `--version`. The latter must launch, exit zero
within 60 seconds, and return exactly the program name and signed release version
followed by a newline. The Sigstore signature and staged identities must pass before
this execution; payload and trust identities are checked again afterward. Install,
upgrade and rollback share this check before changing `current`. This establishes
launch/version compatibility on the installation host, not coverage certification.
Its user-owned dependency installation is separate from collector upgrades.

A local flat bundle contains exactly these regular files, with no symlinks or
extra archives:

| Asset | Purpose |
| --- | --- |
| `harness-gate-rust-stable-collector` | Our x86_64 Linux GNU ELF program |
| `LICENSE` | Authenticated collected notices; production license review pending |
| `support.json` | Typed `rust-stable-support/v1` candidate observations and capability limits |
| `release-inventory.json` | `rust-stable-release-inventory/v1`: version, target, and `files` mapping the three payload names to `{sha256, bytes}` |
| `release-inventory.sig` | `rust-stable-release-signature/v1`: nonempty `sigstore_bundle` |

One keyless Sigstore signature authenticates the exact SHA-256 inventory bytes.
The inventory binds each payload digest and size. JSON
duplicate keys, trailing data and unknown envelope fields are rejected. Payload
hash/length, exact inventory, target and ELF architecture are checked before
selection. The typed `rust-stable-support/v1` document must bind the release,
program and license identities and declare the exact candidate series,
`rust-llvm-exact-free-owner/v4-candidate` function execution, line and code-region coverage, unsupported
CRAP and `candidate-review-required` status. Unknown fields,
empty/duplicate acceptance anchors and overstated capabilities fail even when the
inventory has a valid signature. Observations identify actual acceptance records;
they do not authorize a measurement migration or imply untested platform support.

## External trust and invocation

Trust is supplied independently of both the bundle and installation root. The
caller provides the trusted SHA-256 of a `rust-stable-release-trust/v2` JSON file:

```json
{
  "schema": "rust-stable-release-trust/v2",
  "cosign": "/absolute/host-tools/cosign",
  "cosign_sha256": "<approved SHA-256>",
  "trusted_root": "/absolute/host-trust/trusted-root.json",
  "trusted_root_sha256": "<approved SHA-256>"
}
```

These are placeholders, not provisioned trust. Release assets cannot choose the
verifier, trust root, workflow identity or OIDC issuer. All paths must be
absolute and canonical. The command rechecks host pins and the verified snapshot
after external verification. A missing verifier produces an error naming its path;
there is no automatic tool installation or unsigned fallback.

The existing repository release contract selects cosign 3.1.3. Follow the upstream
[installation instructions](https://docs.sigstore.dev/cosign/system_config/installation/)
and the independently reviewed host trust manifest when provisioning that dependency.
The operator provisioned cosign 3.1.3 and its public trust root under
`target/operator-inputs/cosign-v3.1.3/`; this is not production signature acceptance.
Trust bootstrap and actionable production download commands remain
an explicit acceptance item; this document supplies no unauthenticated bootstrap.

The fixed invocation is `cosign verify-blob --bundle ... --trusted-root ...
--offline --certificate-identity
https://github.com/musutrade/Harness-Gate/.github/workflows/rust-collector-release.yml@refs/tags/rust-collector-v<VERSION>
--certificate-oidc-issuer https://token.actions.githubusercontent.com ...`.
Verification has a 60-second deadline and a fresh private HOME; nonzero exit,
timeout, changed inputs or changed tool identity abort the transaction. Actual
Sigstore signature/inclusion verification remains to be exercised with protected
release evidence. A successful mock subprocess only checks argument/exit handling.

Given authenticated pinned inputs, the candidate command shapes are:

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

New `versions` directories are created atomically with mode 0755 filtered by the
caller's umask. Thus umask 0002 cannot make the directory group writable, and
umask 0077 still restricts it to 0700. Existing directories are validated without
changing their permissions. The focused real install regression covers 0002,
0022 and 0077, repeated installation, and rejection of an existing 0775 directory
while retaining a runnable selected program. See
[the evidence](stable-rust-candidate-evidence/install-permissions.json). This fixes
the operator's concrete defect, not the remaining T5 signing/release acceptance.

Rollback uses an explicit inventory digest and re-verifies the stored release with
the supplied host trust. Missing, corrupt or nonexecutable targets fail without
changing selection. The previous versions are retained. A process killed during
verification can leave unselected staging directories; automatic cleanup is not
implemented. The test records their disk usage separately from retained versions.

Reports include actual package/install bytes and selection identities. Local bundle
operations report zero download bytes. The HTTPS operation below reports received
asset body bytes separately. No compiler archives or acceptance logs are package
assets. Protected signing/publication, production license review, trusted bootstrap
and full independent upgrade acceptance remain open.

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
  --target-dir "$PWD/target/stable-build" --toolchain 1.98.1 \
  --acceptance target/candidate-acceptance/summary.json '<reviewed SHA-256>'
```

The fresh output contains `unsigned-package/` with exactly the executable,
`LICENSE`, `support.json` and `release-inventory.json`. Build logs and
`preparation.json` stay outside this payload. The required signature envelope is
absent, so the installed verifier rejects this package. No unsigned fallback,
release workflow activation or production signing occurs. The release workflow
must supply one Sigstore signature over the exact inventory
bytes under that version tag. Signing, installation verification and publication
reuse the same immutable package; there is no separate private-candidate signing
round, RSA key provisioning or second collector-specific approval packet.

The lifecycle suite consumes this actual prepared program/license/support payload.
Repository automation also makes a separate locked, offline stable release build
with a test-only version suffix. Source identities record that only the crate and
lockfile version changed. The two executables have different hashes and report
their respective versions; the installed first executable upgrades and the installed
second executable rolls back. This tests executable replacement and lifecycle
compatibility within this implementation, not compatibility with a historical or
production release. No compiler is invoked by the installed lifecycle implementation.

Test bundles use an explicitly mocked Sigstore verifier and real SHA-256
inventories. Nonlaunching, wrong-version, nonzero, timed-out and stage-mutating
Rust/ELF fixtures must preserve the current version. Command records check that
failed signatures never reach program execution. Test fixtures, build sources,
verifiers and build caches remain outside the runtime packages.

## Explicit HTTPS download and installation

`download-install REQUEST REQUEST_SHA256 TRUST TRUST_SHA256 ROOT NEW_LOG` adds a
Rust transport to the same authenticated transaction. The caller supplies the exact
request digest and existing host trust; neither comes from a downloaded package.
There is no `latest` discovery, unsigned fallback, automatic trust installation,
package manager invocation or Rust/LLVM environment download. Obtain release
identities through an independently authenticated channel. Production trust
bootstrap and published asset URLs remain pending; test keys are not production
trust. The old release hold remains in force.

The strict `rust-stable-download-request/v1` JSON has `timeout_seconds` (1–120 per
asset), optional `tls_root` (`path`, `sha256`), and an `assets` object containing
exactly the five names in the bundle table. Each asset has its complete HTTPS
`url`, exact positive `bytes`, and lowercase `sha256`. This includes the inventory
and signature envelope, fixing the requested release rather than silently adopting
a different signed version. Unknown/duplicate fields fail. Program size is bounded
at 64 MiB; each other asset at 8 MiB. These are safety limits, not package sizes.

TLS uses ureq 3.4.0/rustls with bundled WebPKI roots by default; it does not require
an external OpenSSL command or library at runtime. A caller may explicitly pin one
host-owned PEM root for private distribution. Certificate and hostname verification
remain mandatory. The optional root must be outside the installation/download
stage. Existing proxy environment variables are honored; TLS and signature checks
still apply. Initial URLs and each of at most three absolute redirects require
HTTPS and reject userinfo/fragments. Relative redirects are currently rejected.
Only status 200 and unencoded bodies are accepted. A present Content-Length must
match; received length and SHA-256 are independently checked. Each asset's deadline
includes its redirects and body. Failed transfers have no automatic retries.

The installation lock covers download, verification and activation. Five assets
are streamed into a private bounded staging directory, then checked by the existing
SHA-256, pinned cosign, support-contract and launch/version validators. Request and
custom TLS-root pins are checked again after transfer; request/trust/payload checks
precede activation. Download success alone grants no installation authority.
Network errors, bad signatures and interrupted transfers preserve `current`.

`NEW_LOG/download.json` records per-asset bytes actually read by the plugin and
whether the transfer completed. `download_bytes` counts HTTP response-body bytes;
it excludes headers, TLS/proxy framing and unread transport buffers, and is not a
wire-traffic measurement. No cache is implemented (`cache_bytes: 0`). An upgrade
downloads these five new assets only, independently of user toolchains. SIGKILL can
prevent the final audit and leave an unselected `.download-*` directory; retries
use fresh staging and retained bytes must be accounted for separately. No automatic
cleanup deletes these directories or prior versions.

Repository-only `validate_download.py` uses a real local HTTPS server with a
separate test CA/server certificate and lifecycle test bundles. It exercises
installation, a separately compiled upgrade, rollback, TLS rejection, downgrade
and redirect limits, bad HTTP/length/encoding/hash, truncation, timeout, failed
signature checks, request/CA changes during transfer, and interruption. SHA-256
verification is real; Sigstore is explicitly mocked by the lifecycle fixture.
`--trace` requires the repository execution observer and is mandatory in CI.
This does not establish public hosting, production
signing or another supported operating system.
