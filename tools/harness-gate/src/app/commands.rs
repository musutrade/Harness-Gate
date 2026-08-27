use super::output::print_scope;
use crate::cli::{Commands, ConfigAction};
use crate::error::CliError;
use crate::project::Project;
use crate::scope::ScopeMode;
use anyhow::Context;

pub(super) fn run(project: &Project, command: Commands) -> Result<bool, CliError> {
    match command {
        Commands::Doctor { json, strict } => {
            let report = crate::doctor::run(project)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(anyhow::Error::from)?
                );
            } else {
                report.print();
            }
            Ok(report.failures == 0 && (!strict || report.warnings == 0))
        }
        Commands::Scope { scope: args, json } => {
            let result = crate::scope::detect(project, &args.mode())?;
            result.write_reports(project)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).map_err(anyhow::Error::from)?
                );
            } else {
                print_scope(&result);
            }
            Ok(true)
        }
        Commands::Secrets { staged, json } => {
            let mode = if staged {
                crate::secrets::SecretMode::Staged
            } else {
                crate::secrets::SecretMode::WorkingTree
            };
            let findings = crate::secrets::scan(project, mode)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "passed": findings.is_empty(),
                        "findings": findings,
                    }))
                    .map_err(anyhow::Error::from)?
                );
            } else if findings.is_empty() {
                println!("Secret scan passed");
            } else {
                eprintln!("Secret scan failed in {} file(s):", findings.len());
                for file in &findings {
                    eprintln!("  {file}");
                }
                eprintln!("Remove and revoke each credential before continuing.");
            }
            Ok(findings.is_empty())
        }
        Commands::Audit { json } => {
            let outcome =
                crate::audit::run(&project.root, &project.audit_config, &project.reports, json)?;
            if !json {
                println!(
                    "Audit: {} violation(s), {} blocker(s), {} error(s), {} warning(s)",
                    outcome.total_violations,
                    outcome.blocker_count,
                    outcome.error_count,
                    outcome.warning_count
                );
                println!("Report: {}", outcome.report_file.display());
            }
            Ok(outcome.total_violations == 0)
        }
        Commands::Verify {
            scope: args,
            components,
            profile,
        } => {
            let selected = if components.is_empty() {
                crate::scope::detect(project, &args.mode())?
            } else {
                let known = project.config.components();
                for component in &components {
                    if !known.contains(component) {
                        return Err(CliError::from(anyhow::anyhow!(
                            "unknown component {component:?}"
                        )));
                    }
                }
                crate::verify::explicit_scope(&components)
            };
            let profile = profile.unwrap_or_else(|| project.config.project.default_profile.clone());
            Ok(crate::verify::run(project, selected, &profile, false)?.passed)
        }
        Commands::Hook => {
            let selected = crate::scope::detect(project, &ScopeMode::Staged)?;
            Ok(crate::verify::run(
                project,
                selected,
                &project.config.project.hook_profile,
                true,
            )?
            .passed)
        }
        Commands::ParseLogs { input, output } => {
            crate::audit::parse_logs(&input, &output)?;
            println!("Error context: {}", output.display());
            Ok(true)
        }
        Commands::Config { action } => run_config(project, action),
        Commands::Step { id } => Ok(crate::verify::run_step(project, &id)?.passed),
        Commands::Init { .. } | Commands::Presets => {
            unreachable!("handled before project discovery")
        }
    }
}

fn run_config(project: &Project, action: ConfigAction) -> Result<bool, CliError> {
    match action {
        ConfigAction::Check => {
            println!("Configuration valid: {}", project.config_path.display());
            println!("Schema version: {}", project.config.version);
            println!(
                "Components: {}",
                project
                    .config
                    .components()
                    .into_iter()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            println!(
                "Profiles: {}",
                project
                    .config
                    .steps
                    .iter()
                    .flat_map(|step| step.profiles.iter().cloned())
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            println!("Verification steps: {}", project.config.steps.len());
            Ok(true)
        }
        ConfigAction::Print { resolved } => {
            if resolved {
                println!(
                    "{}",
                    toml::to_string_pretty(&project.config).map_err(anyhow::Error::from)?
                );
            } else {
                print!(
                    "{}",
                    std::fs::read_to_string(&project.config_path).with_context(|| format!(
                        "read workflow config {}",
                        project.config_path.display()
                    ))?
                );
            }
            Ok(true)
        }
        ConfigAction::Migrate { .. } => unreachable!("handled before project discovery"),
    }
}
