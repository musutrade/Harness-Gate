mod commands;
mod output;

use crate::cli::{Cli, Commands, ConfigAction};
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
    let project = Project::discover(cli.project_root, cli.config)?;
    project.prepare()?;
    commands::run(&project, cli.command)
}
