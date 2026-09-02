mod commands;
mod output;

use crate::cli::{
    AdapterAction, Cli, Commands, CompatAction, ConfigAction, ConfigFormat, SchemaAction,
};
use crate::error::CliError;
use crate::project::Project;
use anyhow::Context;
use clap::Parser;
use std::collections::BTreeSet;
use std::fs;

pub(crate) fn run() -> Result<bool, CliError> {
    let cli = Cli::parse();
    crate::ui::configure(cli.color);
    if let Commands::Init {
        preset: preset_name,
        force,
    } = &cli.command
    {
        let target = cli
            .project_root
            .clone()
            .unwrap_or(std::env::current_dir().context("read current directory")?);
        crate::preset::init(&target, preset_name, *force)?;
        return Ok(true);
    }
    if matches!(cli.command, Commands::Presets) {
        crate::preset::print_presets();
        return Ok(true);
    }
    if let Commands::Adapter {
        action:
            AdapterAction::Run {
                request,
                trusted_keys,
                allow_network,
                allow_resources,
                allow_environment,
            },
    } = &cli.command
    {
        let request_path = request.clone();
        let request = crate::process::read_adapter_request(request)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let trusted_keys = trusted_keys
            .iter()
            .map(|path| {
                let bytes = fs::read(path)
                    .with_context(|| format!("read trusted adapter key {}", path.display()))?;
                serde_json::from_slice(&bytes)
                    .with_context(|| format!("parse trusted adapter key {}", path.display()))
            })
            .collect::<anyhow::Result<Vec<crate::process::TrustedKey>>>()?;
        let policy = crate::process::HostPolicy {
            trusted_keys,
            capabilities: crate::process::CapabilityPolicy {
                network: allow_network.iter().cloned().collect::<BTreeSet<_>>(),
                resources: allow_resources.iter().cloned().collect::<BTreeSet<_>>(),
                environment: allow_environment.iter().cloned().collect::<BTreeSet<_>>(),
            },
            replay_state_dir: request_path
                .parent()
                .map(|parent| parent.join(".harness-gate-adapter-replay")),
            ..crate::process::HostPolicy::default()
        };
        let outcome = crate::process::run_adapter(request, &policy)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        println!(
            "{}",
            serde_json::to_string_pretty(&outcome.response).map_err(anyhow::Error::from)?
        );
        return Ok(true);
    }
    if let Commands::Compat {
        action: CompatAction::Compare { old, new, output },
    } = &cli.command
    {
        let comparison = crate::compat::compare_files(old, new)?;
        crate::utils::fs::atomic_write(
            output,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&comparison).map_err(anyhow::Error::from)?
            ),
            true,
        )
        .with_context(|| format!("write comparison report {}", output.display()))?;
        println!("Comparison written: {}", output.display());
        return Ok(comparison.equivalent);
    }
    if let Commands::Compat {
        action: CompatAction::Canary { state, slice },
    } = &cli.command
    {
        let state = crate::compat::set_canary(state, slice)?;
        println!("Canary enabled for {}: {}", state.slice, state.updated_at);
        return Ok(true);
    }
    if let Commands::Compat {
        action: CompatAction::Rollback { state },
    } = &cli.command
    {
        let state = crate::compat::rollback(state)?;
        println!("Canary rolled back: {}", state.updated_at);
        return Ok(true);
    }
    if let Commands::Schema {
        action: SchemaAction::Export { output },
    } = &cli.command
    {
        let root = cli
            .project_root
            .clone()
            .unwrap_or(std::env::current_dir().context("read current directory")?);
        if output.is_absolute() {
            return Err(
                anyhow::anyhow!("schema output must be relative to the project root").into(),
            );
        }
        let path = crate::utils::fs::confined_atomic_write(
            &root,
            output,
            format!("{}\n", crate::config::schema_json()?),
            true,
        )
        .with_context(|| format!("write workflow schema {}", root.join(output).display()))?;
        println!("Schema written: {}", path.display());
        return Ok(true);
    }
    if let Commands::Config {
        action:
            ConfigAction::Migrate {
                input,
                output,
                force,
            },
    } = &cli.command
    {
        let root = cli
            .project_root
            .clone()
            .unwrap_or(std::env::current_dir().context("read current directory")?);
        crate::preset::migrate(
            &root,
            input.clone().or_else(|| cli.config.clone()),
            output.clone(),
            *force,
        )?;
        return Ok(true);
    }
    if let Commands::Config {
        action: ConfigAction::Check {
            format: ConfigFormat::Json,
        },
    } = &cli.command
    {
        let root = cli
            .project_root
            .clone()
            .unwrap_or(std::env::current_dir().context("read current directory")?);
        match Project::discover(Some(root), cli.config.clone()) {
            Ok(project) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&project.config.diagnostics_report())
                        .map_err(anyhow::Error::from)?
                );
                return Ok(true);
            }
            Err(error) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&crate::config::report_for_error(&error))
                        .map_err(anyhow::Error::from)?
                );
                return Ok(false);
            }
        }
    }
    let is_config_check = matches!(
        &cli.command,
        Commands::Config {
            action: ConfigAction::Check { .. }
        }
    );
    let project = match Project::discover(cli.project_root, cli.config) {
        Ok(project) => project,
        Err(error) if is_config_check => {
            print_human_config_error(&error);
            return Ok(false);
        }
        Err(error) => return Err(error.into()),
    };
    if !is_config_check {
        project.prepare()?;
    }
    commands::run(&project, cli.command)
}

fn print_human_config_error(error: &anyhow::Error) {
    if let Some(diagnostics) = error.downcast_ref::<crate::config::ConfigDiagnostics>() {
        eprintln!(
            "{}",
            crate::ui::error("ERROR [E1000]: configuration check failed")
        );
        eprintln!("{}", diagnostics);
    } else {
        eprintln!(
            "{}",
            crate::ui::error(format!(
                "ERROR [E1000]: configuration check failed: {error:#}"
            ))
        );
        let report = crate::config::report_for_error(error);
        for diagnostic in report.diagnostics {
            eprintln!("  help: {}", diagnostic.help);
        }
    }
    eprintln!("Next: harness-gate init --preset generic");
    eprintln!("Minimal flow.toml shape:");
    eprintln!("{}", crate::config::MINIMAL_CONFIG_SNIPPET);
    eprintln!("Then run: harness-gate config check");
}
