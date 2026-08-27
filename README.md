# Harness-Gate

[![CI](https://github.com/musutrade/Harness-Gate/actions/workflows/ci.yml/badge.svg)](https://github.com/musutrade/Harness-Gate/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/harness-gate.svg)](https://crates.io/crates/harness-gate)
[![Documentation](https://docs.rs/harness-gate/badge.svg)](https://docs.rs/harness-gate)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

[English](README.md) | [简体中文](README.zh-CN.md)

`Harness-Gate` is a reusable Rust development workflow and architecture guard CLI. This is an independent tool providing complete quality gate and workflow management capabilities.

It handles changed paths, secret scanning, architecture auditing, environment validation, external command orchestration, test result counting, timeout and interrupt handling, and temporary service lifecycle. Git hooks only keep launchers, and flow decisions do not depend on Shell scripts.

## Navigation

- Quick Start: see [Installation](#installation) and [Quick Start](#installation-and-quick-start)
- New Project Integration: see [Installation and New Project Integration](#installation-and-new-project-integration) and [Built-in Presets](#built-in-presets)
- Add Commands, Components or CI Profiles: see [Selection Model](#selection-model) and [schema v2 configuration reference](docs/configuration.md)
- Handle Failures: see [Reports and Logs](#reports-and-logs) and [Troubleshooting](#troubleshooting)
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
  -> JSON / Markdown / log reports
```

Components, profiles, commands, paths, parsers and services all come from TOML. Regular project migration does not need to add enums or modify match branches in Rust.

## Installation

### Install from Crates.io (Recommended)

```bash
cargo install harness-gate
```

### Install from GitHub Release (Pre-built Binaries)

Download the binary for your platform from [GitHub Releases](https://github.com/musutrade/Harness-Gate/releases/latest):

- **Linux (x86_64)**: `harness-gate-linux-amd64`
- **macOS (Intel)**: `harness-gate-macos-amd64`
- **macOS (Apple Silicon)**: `harness-gate-macos-arm64`
- **Windows (x86_64)**: `harness-gate-windows-amd64.exe`

Example installation on Linux/macOS:

```bash
# Download (replace with your platform)
wget https://github.com/musutrade/Harness-Gate/releases/download/v0.2.0/harness-gate-linux-amd64

# Make executable
chmod +x harness-gate-linux-amd64

# Move to PATH
sudo mv harness-gate-linux-amd64 /usr/local/bin/harness-gate

# Verify installation
harness-gate --version
```

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

Presets are starting points, not runtime branches. After initialization is complete, all behavior is determined by the project's internal TOML: you can rename components, add `ci` profiles, switch to MySQL/Redis, adjust directories or replace any steps without retaining preset original names.

## Command Overview

| Command                                                      | Purpose                                   |
| ------------------------------------------------------------ | ----------------------------------------- |
| `harness-gate presets`                                       | List built-in project presets             |
| `harness-gate init --preset <name>`                          | Generate schema v2 configuration          |
| `harness-gate doctor [--strict] [--json]`                    | Execute environment checks declared in config |
| `harness-gate scope [--staged\|--base REF\|--all] [--json]`  | List changes and selected components      |
| `harness-gate secrets [--staged] [--json]`                   | Scan high-confidence credential patterns, only output matched filenames |
| `harness-gate audit [--json]`                                | Execute regex architecture rules and generate audit report |
| `harness-gate verify`                                        | Execute default profile based on workspace changes |
| `harness-gate verify --profile ci --all`                     | Execute specified profile for all components |
| `harness-gate hook`                                          | Execute hook profile on staged snapshot   |
| `harness-gate step <id>`                                     | Run a single step after passing secrets/audit |
| `harness-gate config check`                                  | Validate schema, references, paths and environment overrides |
| `harness-gate config print --resolved`                       | Output final effective configuration      |
| `harness-gate config migrate`                                | Convert schema v1 to v2                   |
| `harness-gate parse-logs`                                    | Extract JSON Lines ERROR trace context    |

All commands support global `--project-root <PATH>` and `--config <PATH>`. Commands return 0 on success; configuration errors, gate failures, step failures, timeouts or interrupts return non-zero, suitable for direct use in CI.

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

The repository's pre-commit automatically executes `harness-gate hook`. The hook profile only retains fast deterministic checks, not a replacement for complete testing.

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

### Environment Check (Doctor)
- Tool version validation
- Environment variable checking
- Docker service availability
- Git configuration validation

### Test Service Management
- Docker temporary containers
- Random port allocation
- Health checks
- Automatic cleanup

### Flexible Configuration
- Schema v2 TOML configuration
- Path aliases and placeholders
- Environment variable overrides
- Custom test parsers

## Installation

### Method 1: Install Script

```bash
# Download and run install script
curl -sSL https://raw.githubusercontent.com/yourusername/Harness-Gate/main/install.sh | bash

# Or install from source
curl -sSL https://raw.githubusercontent.com/yourusername/Harness-Gate/main/install.sh | bash -s -- --from-source
```

### Method 2: Manual Installation from Source

```bash
# Clone repository
git clone https://github.com/yourusername/Harness-Gate.git
cd Harness-Gate

# Compile and install
cargo install --locked --path tools/harness-gate

# Verify installation
harness-gate --version
# Output: harness-gate 1.0.0
```

### Method 3: From crates.io (Coming Soon)

```bash
cargo install harness-gate
```

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

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of conduct and the process for submitting pull requests.

## Acknowledgments

Thanks to all contributors of the arc-admin project, Harness-Gate evolved from that excellent foundation.

## Links

- **Documentation**: [docs/](docs/)
- **Issue Tracker**: https://github.com/yourusername/Harness-Gate/issues
- **Original Project**: https://github.com/musutrade/arc-admin
