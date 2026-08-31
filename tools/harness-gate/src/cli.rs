use crate::scope::ScopeMode;
use crate::ui::ColorMode;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "harness-gate",
    version,
    about = "Configurable development workflow and architecture guard",
    arg_required_else_help = true,
    after_help = "Examples:\n  harness-gate presets\n  harness-gate init --preset rust-api\n  harness-gate doctor\n  harness-gate cleanup --dry-run\n  harness-gate verify --all"
)]
pub(crate) struct Cli {
    /// Control colored terminal output.
    #[arg(long, global = true, value_enum, default_value_t = ColorMode::Auto)]
    pub(crate) color: ColorMode,

    /// Override automatic project root discovery.
    #[arg(long, global = true, value_name = "PATH")]
    pub(crate) project_root: Option<PathBuf>,

    /// Override the repository workflow configuration file.
    #[arg(long, global = true, value_name = "PATH")]
    pub(crate) config: Option<PathBuf>,

    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    /// Run the versioned serial compatibility launcher and migration tools.
    Compat {
        #[command(subcommand)]
        action: CompatAction,
    },
    /// Execute a signed out-of-process adapter request.
    Adapter {
        #[command(subcommand)]
        action: AdapterAction,
    },
    /// Check local tools, configuration, Git, and test database access.
    Doctor {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
        /// Treat warnings as failures.
        #[arg(long)]
        strict: bool,
    },
    /// Inspect and reclaim stale Harness-Gate resource leases.
    Cleanup {
        /// Only report stale marked resources without reclaiming them.
        #[arg(long)]
        dry_run: bool,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Show changed files and the verification components they select.
    Scope {
        #[command(flatten)]
        scope: ScopeArgs,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
        /// Repeat the in-process matcher benchmark (used by quality baselines).
        #[arg(long, hide = true, default_value_t = 0, value_name = "N")]
        benchmark_repeat: usize,
    },
    /// Scan file names for high-confidence credential patterns.
    Secrets {
        /// Scan the staged snapshot instead of the working tree.
        #[arg(long)]
        staged: bool,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Run deterministic architecture rules and write review_context reports.
    Audit {
        /// Emit the complete audit report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Run gates and configured steps for the selected profile and components.
    #[command(visible_alias = "check")]
    Verify {
        #[command(flatten)]
        scope: ScopeArgs,
        /// Override scope detection with a comma-separated component list.
        #[arg(
            long,
            value_delimiter = ',',
            num_args = 1..,
            conflicts_with_all = ["staged", "all", "base"]
        )]
        components: Vec<String>,
        /// Select any profile declared by configured steps.
        #[arg(long, value_name = "PROFILE")]
        profile: Option<String>,
    },
    /// Run the staged, fast verification profile used by pre-commit.
    Hook,
    /// Extract one trace's error context from JSON Lines logs.
    ParseLogs {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Validate or inspect the repository workflow configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Generate artifacts for the workflow configuration model.
    Schema {
        #[command(subcommand)]
        action: SchemaAction,
    },
    /// Run one configured full-profile step after secrets and audit gates.
    Step {
        /// Step id from flow.toml, for example api.clippy.
        id: String,
    },
    /// Initialize .harness-gate configuration from an embedded preset.
    Init {
        #[arg(long, default_value = "generic")]
        preset: String,
        /// Replace existing .harness-gate configuration files.
        #[arg(long)]
        force: bool,
    },
    /// List embedded project presets.
    Presets,
}

#[derive(Debug, Subcommand)]
pub(crate) enum CompatAction {
    /// Replay an existing DevRail request through the serial verifier.
    Run {
        #[arg(short, long, value_name = "PATH")]
        input: PathBuf,
        #[arg(short, long, value_name = "PATH")]
        output: PathBuf,
        /// Optional frozen/legacy result to compare in shadow mode.
        #[arg(long, value_name = "PATH")]
        old_result: Option<PathBuf>,
    },
    /// Compare two machine results after removing volatile execution fields.
    Compare {
        #[arg(long, value_name = "PATH")]
        old: PathBuf,
        #[arg(long, value_name = "PATH")]
        new: PathBuf,
        #[arg(short, long, value_name = "PATH")]
        output: PathBuf,
    },
    /// Enable a bounded migration canary slice.
    Canary {
        #[arg(long, value_name = "PATH")]
        state: PathBuf,
        #[arg(long)]
        slice: String,
    },
    /// Disable the launcher and record a rollback event.
    Rollback {
        #[arg(long, value_name = "PATH")]
        state: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum AdapterAction {
    /// Validate and execute one JSON adapter request.
    Run {
        #[arg(short, long, value_name = "PATH")]
        request: PathBuf,
        /// Trusted Ed25519 key JSON files ({"key_id": ..., "public_key": ...}).
        #[arg(long = "trusted-key", value_name = "PATH")]
        trusted_keys: Vec<PathBuf>,
        /// Capability values allowed by the host. Repeat for multiple values.
        #[arg(long = "allow-network", value_name = "CAPABILITY")]
        allow_network: Vec<String>,
        #[arg(long = "allow-resource", value_name = "CAPABILITY")]
        allow_resources: Vec<String>,
        #[arg(long = "allow-environment", value_name = "NAME")]
        allow_environment: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum SchemaAction {
    /// Export the JSON Schema for flow.toml.
    Export {
        #[arg(long, value_name = "PATH", default_value = "schema/flow.schema.json")]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ConfigAction {
    /// Validate configuration, environment overrides, and protected steps.
    Check {
        /// Render diagnostics as a machine-readable JSON envelope.
        #[arg(long, value_enum, default_value_t = ConfigFormat::Human)]
        format: ConfigFormat,
    },
    /// Print the source or effective configuration.
    Print {
        /// Include environment overrides in the rendered TOML.
        #[arg(long)]
        resolved: bool,
    },
    /// Convert a schema v1 flow.toml to .harness-gate/flow.toml schema v2.
    Migrate {
        #[arg(long, value_name = "PATH")]
        input: Option<PathBuf>,
        #[arg(long, value_name = "PATH")]
        output: Option<PathBuf>,
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub(crate) enum ConfigFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ScopeArgs {
    /// Inspect only staged files.
    #[arg(long, conflicts_with_all = ["all", "base"])]
    staged: bool,
    /// Select every verification component.
    #[arg(long, conflicts_with_all = ["staged", "base"])]
    all: bool,
    /// Inspect committed changes in REF...HEAD.
    #[arg(long, value_name = "REF", conflicts_with_all = ["staged", "all"])]
    base: Option<String>,
}

impl ScopeArgs {
    pub(crate) fn mode(&self) -> ScopeMode {
        if self.staged {
            ScopeMode::Staged
        } else if self.all {
            ScopeMode::All
        } else if let Some(reference) = &self.base {
            ScopeMode::Base(reference.clone())
        } else {
            ScopeMode::WorkingTree
        }
    }
}
