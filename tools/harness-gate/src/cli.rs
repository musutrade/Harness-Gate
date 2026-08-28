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
    after_help = "Examples:\n  harness-gate presets\n  harness-gate init --preset rust-api\n  harness-gate doctor\n  harness-gate verify --all"
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
    /// Check local tools, configuration, Git, and test database access.
    Doctor {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
        /// Treat warnings as failures.
        #[arg(long)]
        strict: bool,
    },
    /// Show changed files and the verification components they select.
    Scope {
        #[command(flatten)]
        scope: ScopeArgs,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
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
