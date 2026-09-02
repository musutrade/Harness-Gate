# Harness-Gate

[![CI](https://github.com/musutrade/Harness-Gate/actions/workflows/ci.yml/badge.svg)](https://github.com/musutrade/Harness-Gate/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/harness-gate.svg)](https://crates.io/crates/harness-gate)
[![Documentation](https://docs.rs/harness-gate/badge.svg)](https://docs.rs/harness-gate)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

[English](https://github.com/musutrade/Harness-Gate/blob/main/README.md) | [简体中文](https://github.com/musutrade/Harness-Gate/blob/main/README.zh-CN.md)

`Harness-Gate` is a reusable Rust development workflow and architecture guard CLI. This is an independent tool providing complete quality gate and workflow management capabilities.

It handles changed paths, secret scanning, architecture auditing, environment validation, external command orchestration, test result counting, timeout and interrupt handling, and temporary service lifecycle. Git hooks only keep launchers, and flow decisions do not depend on Shell scripts.

## Navigation

- Quick Start: see [Installation](#installation) and [Quick Start](#installation-and-quick-start)
- New Project Integration: see [Installation and Quick Start](#installation-and-quick-start) and [Built-in Presets](#built-in-presets)
- Add Commands, Components or CI Profiles: see [Command Overview](#command-overview) and [schema v2 configuration reference](https://github.com/musutrade/Harness-Gate/blob/main/docs/configuration.md)
- Handle Failures: see [Reports and Notifications](#reports-and-notifications) and [Common Repair Paths](#common-repair-paths)
- Extend Rust Engine: see [No Code Change Scope](#no-code-change-scope) and [Rust Change Boundary](#rust-change-boundary)

## Working Model

`harness-gate` breaks project flow into four types of data:

1. **scope rule**: Maps Git changed paths to components
2. **profile**: Selects different intensity steps from the same component, such as `hook`, `full`, `ci`
3. **step**: Declares a `program + args[]` external command, timeout, log, parser and service dependencies
4. **gate**: Fixed to run secret scan and audit first, only allowing external steps to execute after success

Data flow when running `verify`:

```text
Git changed files
  -> scope.rules
  -> components
  -> steps matched by component + profile
  -> secret scan
  -> architecture audit
  -> execute steps in configured order
  -> JSON / Markdown / optional HTML/JUnit reports
  -> optional HTTP(S) Webhook notification
```

Components, profiles, commands, paths, parsers and services all come from TOML. Regular project migration does not need to add enums or modify match branches in Rust.

## Installation

### Install from Crates.io (Recommended)

```bash
cargo install harness-gate
```

### Install from GitHub Release (Pre-built Binaries)

Download the binary for your platform from an immutable [GitHub Release
tag](https://github.com/musutrade/Harness-Gate/releases/tag/v0.3.5):

- **Linux (x86_64)**: `harness-gate-linux-amd64`
- **macOS (Intel)**: `harness-gate-macos-amd64`
- **macOS (Apple Silicon)**: `harness-gate-macos-arm64`
- **Windows (x86_64)**: `harness-gate-windows-amd64.exe`

The checked installer verifies the checksum manifest and Sigstore certificate
before changing the installation directory. Download the script from the same
immutable tag, then pass that tag explicitly:

```bash
curl --fail --show-error --location --proto '=https' --tlsv1.2 \
  -o /tmp/harness-gate-install.sh \
  https://raw.githubusercontent.com/musutrade/Harness-Gate/v0.3.5/install.sh
bash /tmp/harness-gate-install.sh --version v0.3.5
harness-gate --version
```

The script installs to `~/.local/bin` by default. Set `--install-dir` to an
existing private directory when a different location is required. It never
uses the mutable `releases/latest` API or a `raw/main` installation command.
Binary installation requires the `cosign` CLI so the keyless Sigstore
certificate can be checked locally; source installation additionally requires
`git` and Rust `cargo`.

Release assets include `SHA256SUMS`, a CycloneDX SBOM, and Sigstore bundles. For
an offline integrity check, download the binary, `SHA256SUMS`, and the matching
`.sig`/`.crt` files, then run:

```bash
sha256sum --check SHA256SUMS
cosign verify-blob --signature harness-gate-linux-amd64.sig \
  --certificate harness-gate-linux-amd64.crt \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  --certificate-identity-regexp '^https://github.com/musutrade/Harness-Gate/.github/workflows/release\.yml@refs/tags/v0\.3\.5$' \
  harness-gate-linux-amd64
```

Verify `harness-gate.sbom.cdx.json` with its corresponding signature as well;
the SBOM records the source commit, Cargo.lock digest, and Rust toolchain used
for the build. The release workflow first writes one explicit
`release-inventory.json`; checksum, Sigstore signature/certificate, GitHub
provenance, verification, and upload subjects are all derived from that same
inventory, including the SBOM. Missing, extra, unsigned, or unattested assets
block release creation. Published release assets are immutable: a replacement
requires a new version tag and a new attestation.

### Install from Source

```bash
git clone https://github.com/musutrade/Harness-Gate.git
cd Harness-Gate
cargo install --locked --path tools/harness-gate
```

## Development Commands

The repository uses `cargo-nextest` for fast, isolated test execution:

```bash
cargo nextest run --manifest-path tools/harness-gate/Cargo.toml
cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check
```

`release` keeps the default panic unwinding behavior for diagnostics. Use
`release-small` for the distributed binary when size matters:

```bash
cargo build --manifest-path tools/harness-gate/Cargo.toml --release
cargo build --manifest-path tools/harness-gate/Cargo.toml --profile release-small
```

## Installation and Quick Start

Verify installation and explore presets:

```bash
harness-gate --version
harness-gate presets
```

Initialize in target project:

```bash
harness-gate --project-root /path/to/new-project init --preset rust-api
harness-gate --project-root /path/to/new-project config check
harness-gate --project-root /path/to/new-project doctor
harness-gate --project-root /path/to/new-project cleanup --dry-run
harness-gate --project-root /path/to/new-project verify --all
```

Recommended integration order:

1. Select the preset closest to the tech stack
2. Modify paths, components, scope and steps in `.harness-gate/flow.toml`
3. Add project-specific architecture rules in `.harness-gate/audit.toml`
4. Add business or vendor-specific credential rules in `.harness-gate/secrets.toml`
5. Run `config check` to resolve reference and schema errors first
6. Run `doctor` to fill in local tools, dependencies, images or environment variables
7. Run `verify --all` in clean repository to confirm all components can run
8. Use the same command in CI, and optionally install thin hooks that only call `harness-gate hook`

`init` creates the target directory, but `scope` and `verify` require the target directory to be a Git worktree. Refuses to overwrite when the project already has configuration; only use `--force` when confirming the target content is replaceable.

## Built-in Presets

| Preset                    | Purpose                     | Initial Steps                           |
| ------------------------- | --------------------------- | --------------------------------------- |
| `generic`                 | Any Git project             | working tree and staged whitespace check |
| `rust-api`                | Single Rust crate           | fmt, Clippy, check, test                |
| `angular-only`            | Angular/npm                 | lint, format check, test, build         |
| `angular-rust-postgres`   | Angular + Rust + PostgreSQL | Dual-end check, test, build and temporary DB |

`init` writes `.harness-gate/flow.toml`, `.harness-gate/audit.toml`, `.harness-gate/secrets.toml` and `.harness-gate/.gitignore` with same-directory temporary files and atomic rename, will not leave half-written files, and will not overwrite existing configuration unless explicitly passing `--force`. `config migrate` uses the same atomic write strategy for target configuration, and generates Secret Scan v2 default rules when missing. Newly created audit v2 files preset lexical configuration for Rust, TypeScript, JavaScript, SQL, TOML and YAML, and can directly append the first rule.

Presets are starting points, not runtime branches. After initialization is complete, all behavior is determined by the project's internal TOML: you can rename components, add `ci` profiles, switch to MySQL/Redis, adjust directories or replace any steps without retaining preset original names. Audit configuration is project-local: the selected project's `flow.toml` determines a literal repository-relative `paths.audit_config`; use explicit `--project-root` and `--config` options when selecting a different project or workflow file.

## Command Overview

| Command                                                      | Purpose                                   |
| ------------------------------------------------------------ | ----------------------------------------- |
| `harness-gate presets`                                       | List built-in project presets             |
| `harness-gate init --preset <name>`                          | Generate schema v2 configuration          |
| `harness-gate doctor [--strict] [--json]`                    | Execute environment checks declared in config |
| `harness-gate cleanup [--dry-run] [--json]`                 | Inspect or reclaim stale resource leases  |
| `harness-gate scope [--staged\|--base REF\|--all] [--json]`  | List changes and selected components      |
| `harness-gate secrets [--staged] [--json]`                   | Scan high-confidence credential patterns, only output matched filenames |
| `harness-gate audit [--json]`                                | Execute regex architecture rules and generate audit report |
| `harness-gate verify`                                        | Execute default profile based on workspace changes |
| `harness-gate verify --profile ci --all`                     | Execute specified profile for all components |
| `harness-gate hook`                                          | Execute hook profile on a private staged snapshot |
| `harness-gate step <id>`                                     | Run a single step after passing secrets/audit |
| `harness-gate config check`                                  | Validate schema, references, paths, environment overrides, and resource safety |
| `harness-gate config check --format json`                    | Emit stable field-path diagnostics for editors and CI          |
| `harness-gate config print --resolved`                       | Output final effective configuration      |
| `harness-gate config migrate`                                | Convert schema v1 to v2                   |
| `harness-gate schema export`                                 | Generate JSON Schema for flow.toml         |
| `harness-gate adapter run --request <PATH> --trusted-key <PATH>` | Validate and execute one signed out-of-process adapter request |
| `harness-gate parse-logs`                                    | Extract JSON Lines ERROR trace context    |

All commands support global `--project-root <PATH>` and `--config <PATH>`. Commands return 0 on success; configuration errors, gate failures, step failures, timeouts or interrupts return non-zero, suitable for direct use in CI.

`cleanup --dry-run` is safe to run before a retry or on a shared CI host. It
only lists lease records carrying the `harness-gate` owner marker and writes
the structured observation to `<reports>/cleanup.json`. Lease schema `2`
records the canonical project, logical resource, invocation, complete runtime
labels, container name, and immutable runtime object ID. Without `--dry-run`,
expiry only makes a lease eligible for reclaim: the runtime object is inspected
again and its filename, labels, project, resource, invocation, and immutable ID
must all match before removal. Unknown, malformed, renamed, active, or
ambiguous leases are retained, and a failed stop remains a blocking cleanup
failure for a later retry.

Interactive `verify` renders a progress bar and colored pass/warning/failure markers. Output stays plain when redirected or in CI. Use `--color auto` (default), `--color always`, or `--color never`; `NO_COLOR` disables automatic color.

## Typical Workflow

### Daily Development

```bash
# Confirm scope before coding
harness-gate scope

# Only verify hit components after coding
harness-gate verify

# Directedly re-run a single step; still goes through secrets and audit first
harness-gate step frontend.tests
```

When there are no changes in the workspace, the default scope does not select components, and `verify` only runs fixed gates. `--all` must be explicitly passed when confirmation of the entire repository is needed.

### Before Commit

```bash
git add <explicit file list>
harness-gate scope --staged
harness-gate hook
git commit -m "..."
```

The repository's pre-commit automatically executes `harness-gate hook`. The command materializes the complete Git index into a private staged snapshot before loading configuration, selecting scope, running built-in gates, or executing ordinary steps. It therefore does not mix staged and unstaged file contents. The hook profile only retains fast deterministic checks, not a replacement for complete testing.

Configured steps use `input = "snapshot"` by default, so `{root}`, path aliases, and arguments resolve against the invocation input root. A step that genuinely needs the original checkout or direct Git metadata may opt into `input = "repository"`; this is an explicit compatibility capability and does not change scope, secret-scan, or architecture-audit input. Invocation reports record the input mode, source identity, execution root, and configuration digest.

### Before PR or Release

```bash
harness-gate verify --all
```

This command ignores changed paths, selects all components in the configuration, and executes the default `full` profile.

### Compare Against a Baseline

```bash
harness-gate scope --base origin/main
harness-gate verify --base origin/main
```

`--base REF` uses committed changes from `REF...HEAD`, does not include uncommitted workspace content, suitable for CI or PR branch verification.

### Manually Specify Scope

```bash
harness-gate verify --components backend
harness-gate verify --components backend,frontend --profile full
```

Explicit components override automatic scope, and cannot be used with `--staged`, `--base`, `--all` at the same time. Unknown components or profiles fail immediately.

## Error Codes

Operational failures are printed as `ERROR [E####]: message`. The code is stable
enough for CI logs and support requests; the message keeps the file path or Git
command context needed to resolve the issue.

| Code | Category |
| ---- | -------- |
| `E1000` | General command, project, or configuration failure |
| `E1101`-`E1103` | Audit configuration, execution, or log parsing |
| `E1201`-`E1202` | Secret-scan configuration or execution |
| `E1301`-`E1304` | Git scope, scope configuration, unmatched paths, or reports |
| `E1401`-`E1404` | Verification selection, cancellation, execution, or reports |

### Common Repair Paths

For a new repository, use the shortest repeatable setup:

```bash
harness-gate init --preset generic
harness-gate config check
harness-gate doctor
harness-gate verify --all
```

When `config check` fails, the human output includes the stable error code,
field path, a repair hint, and a minimal schema v2 `flow.toml` shape. Apply the
hint (or regenerate with `harness-gate init --preset generic`), then run
`harness-gate config check` again. Use `--format json` when an editor or CI
needs machine-readable diagnostics.

For daily work, inspect `harness-gate scope`, then run `harness-gate verify`.
Before a commit, stage the intended files and run `harness-gate scope --staged`
followed by `harness-gate hook`. Before a PR or release, run
`harness-gate verify --all`; upload the reports directory when a failure needs
review.

## Features

### Multi-component Workflow Management
- Intelligent component selection based on Git changes
- Configurable profiles (hook, full, ci)
- Parallel step execution
- Complete timeout and interrupt handling

### Security Gates
- **Secret Scan**: High-confidence credential detection
- **Architecture Audit**: Regex architecture rules
- Enforced before all external steps
- Scans reject individual files larger than 16 MiB to keep memory use bounded

### Environment Check (Doctor)
- Tool version validation
- Environment variable checking
- Docker service availability
- Git configuration validation

### Test Service Management
- Docker or Podman temporary containers
- Random port allocation
- Health checks
- Automatic cleanup
- Selected services are prewarmed while the mandatory secret and architecture gates run

### Flexible Configuration
- Schema v2 TOML configuration
- Path aliases and placeholders
- Environment variable overrides
- Custom test parsers
- Optional HTML/JUnit reports and HTTP(S) Webhook notifications

## Git Hook Integration

Set up Git hooks:

```bash
git config core.hooksPath hooks
```

The `pre-commit` hook executes `harness-gate hook`. The hook profile does not run database integration tests or production builds; use `harness-gate verify --all` before delivery.

For standalone installation in new projects, you can create a similar thin hook:

```sh
#!/bin/sh
set -eu
root="$(git rev-parse --show-toplevel)"
cd "$root"
exec harness-gate hook
```

The hook is only responsible for locating the root directory and launching the binary, all selection, gates and steps remain in Rust and TOML.

## CI Integration

CI is recommended to execute the full profile rather than relying on workspace diff on the runner:

```bash
harness-gate config check
harness-gate doctor --strict
harness-gate verify --all
```

To reuse external test services, inject the service configuration's `external_env` into the job; otherwise, pre-pull configured images and allow runner access to Docker daemon. Caching Cargo, npm and build directories only affects performance, should not skip `verify --all`.

Whether successful or failed, it is recommended to upload the `[paths].reports` directory as an artifact. This preserves audit line numbers, step timings and complete logs.

## Reports and Notifications

Every verification writes canonical evidence below `reports/invocations/<invocation_id>/`, including
`invocation.json`, `scope.json`, step logs, and the machine/human reports. The compatible
`test_result.json` and `test_result.md` files are mirrored at the report root for existing consumers. A
repository-contained HTML template can additionally produce `test_result.html`, and `report_templates.junit`
can write JUnit XML under the invocation directory. HTML templates use Tera and support `include`, `extends`,
and `block`; the full serialized report is available as `report` alongside the legacy direct fields. Report
output paths are containment-checked, including existing symlinks, and published files use temporary-file plus
atomic-rename semantics.

`test_result.json` follows the versioned [machine-result schema](https://github.com/musutrade/Harness-Gate/blob/main/schema/machine-result.schema.json). It keeps
the legacy `passed` field while exposing stable `status` values, per-step `attempts`, structured `failures`,
invocation-relative `artifacts`, and `evidence_complete`; consumers should use `status` and these structured
fields instead of parsing Markdown or log text.

Runner results also record parser mode/version and completeness. JUnit, TRX, and
JSON parsers are preferred over regex compatibility mode; malformed, zero, and
partial result files map to `RESULT_PARSE_FAILURE`, `RESULT_ZERO`, and
`RESULT_PARTIAL`. JUnit accepts one `testsuite` or `testsuites` root and TRX one
`TestRun` root (namespace prefixes are supported); missing/multiple roots and
non-whitespace content outside the root fail closed. Bounded retries expose `retry_count` and `flaky`, while shard
results expose a merge identity and reject missing or duplicate test identities.
An approved expiring waiver is machine-distinct as `WAIVED` and includes its
approval and compensating-control evidence.

Each invocation also publishes `artifact-registry.json` and `manifest.json`, described by the
[artifact registry schema](https://github.com/musutrade/Harness-Gate/blob/main/schema/artifact-registry.schema.json)
and [artifact manifest schema](https://github.com/musutrade/Harness-Gate/blob/main/schema/artifact-manifest.schema.json).
The registry and manifest record every invocation-local evidence file (excluding control files) with its relative
path, kind, byte size, and SHA-256 digest; step artifacts also carry invocation and step bindings. Result
declarations, registry entries, manifest entries, and publishable files on disk must match as one closed set.
Missing, undeclared, escaped, symlinked, replaced, or digest-mismatched evidence fails publication and leaves
`evidence_complete` false. The final machine result is written only after the manifest has passed validation.
Text evidence is redacted before publication: authorization and cookie headers, bearer/basic credentials, API keys,
passwords, private-key blocks, and common database connection strings are replaced with `[REDACTED]`. The default
retention policy keeps the newest 50 invocation directories and only removes older directories after the 15-minute
lease window; active or recently modified invocations are retained.

Optional `[[notifications.webhooks]]` entries send the serialized report to HTTP(S) endpoints after all report
files are written. `on_failure` defaults to `true`, `on_success` defaults to `false`; a non-2xx response or
connection error returns `E1404` while preserving the generated reports. Entries are sent in configuration order;
the first failure stops notification delivery to later endpoints.

Service cleanup is part of verification success. If a started service cannot be stopped, verification remains
failed and the cleanup error is included in `test_result.json`/`test_result.md` so a leaked container is not hidden.

For a DevRail migration, replay a serial request and compare normalized results:

```bash
harness-gate compat run --input request.json --output result.json --old-result frozen.json
harness-gate compat compare --old frozen.json --new result.json --output comparison.json
harness-gate compat canary --state migration-canary.json --slice team-a
harness-gate compat rollback --state migration-canary.json
```

These commands retain raw result digests and never delete invocation evidence.
The P2 signed adapter boundary is specified separately in
[ADR-0033](docs/adr/0033-signed-out-of-process-adapter-protocol.md). A host can
execute one signed adapter request without loading code into the CLI process:

```bash
harness-gate adapter run \
  --request adapter-request.json \
  --trusted-key adapter-key.json \
  --allow-resource test-database
```

The v2 host verifies an Ed25519 signature over the canonical complete request
(adapter identity, invocation/step, arguments, input, environment,
capabilities, timeout, configuration digest, artifact root, nonce, and validity
window) and the executable SHA-256 digest before starting the adapter. It
clears inherited environment variables, applies the declared capability
allowlist, enforces bounded stdout/stderr and artifact budgets, attempts bounded
process-tree cleanup on timeout or cancellation, and rejects malformed results
or artifacts outside the invocation root. Adapter failures are reported as
`ADAPTER_PROTOCOL_FAILURE`. Nonces are single-use within the host policy; a
long-lived orchestrator should persist its replay ledger across host restarts.

The request is a single JSON document; it is not read from `flow.toml` and does
not enable adapters in built-in steps. Repeat `--trusted-key` for every trusted
Ed25519 key and pass only the capabilities that the invocation is allowed to
use (`--allow-network`, `--allow-resource`, or `--allow-environment`). Keep the
request, executable, and artifact root inside the project or another explicitly
managed deployment directory, and review the signer before execution. The
capability allowlist is a protocol-level declaration check, not an
operating-system network, filesystem, resource, or process sandbox; process
cleanup is best effort and is not proof of complete descendant containment.
The host retains at most 16 MiB per output stream by default and returns a
structured truncation failure when a limit is exceeded.

`parse-logs` reads JSON Lines in bounded streaming passes. It keeps at most 20 records before the first matching
error and 30 records in the output; when no trace ID is available it emits only the last 30 raw lines. This avoids
loading an unbounded application log into memory.

## No Code Change Scope

The following changes only require edits to `.harness-gate/flow.toml` or the
project audit TOML:

- add or rename components, profiles, and external steps;
- connect Go, Java, Python, Node, or other CLI tools;
- adjust monorepo scope rules and path aliases;
- add Doctor checks expressible by the existing check kinds;
- add regex result parsers, Docker/environment services, or audit rules;
- provide CI-specific timeout, image, or directory overrides.

## Rust Change Boundary

Source changes are required for a new behavior category, such as a non-Docker
service lifecycle provider, a report format that cannot be parsed by regex, a
new Doctor protocol, a new credential algorithm, or a different process
cancellation policy. These changes require tests, preset validation, and a
versioned compatibility review.

## License

This project is licensed under the MIT License - see the [LICENSE](https://github.com/musutrade/Harness-Gate/blob/main/LICENSE) file for details.

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](https://github.com/musutrade/Harness-Gate/blob/main/CONTRIBUTING.md) for details on our code of conduct and the process for submitting pull requests.

## Acknowledgments

Thanks to all contributors of the arc-admin project, Harness-Gate evolved from that excellent foundation.

## Links

- **Documentation**: [docs/](https://github.com/musutrade/Harness-Gate/tree/main/docs)
- **Issue Tracker**: https://github.com/musutrade/Harness-Gate/issues
- **Original Project**: https://github.com/musutrade/arc-admin
