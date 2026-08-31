mod commands;
mod output;

use crate::cli::{Cli, Commands, CompatAction, ConfigAction, ConfigFormat, SchemaAction};
use crate::error::CliError;
use crate::project::Project;
use anyhow::Context;
use clap::Parser;

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
        let path = if output.is_absolute() {
            output.clone()
        } else {
            root.join(output)
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create schema directory {}", parent.display()))?;
        }
        crate::utils::fs::atomic_write(&path, format!("{}\n", crate::config::schema_json()?), true)
            .with_context(|| format!("write workflow schema {}", path.display()))?;
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
