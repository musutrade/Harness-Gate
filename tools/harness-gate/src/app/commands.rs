use super::output::print_scope;
use crate::cli::{Commands, CompatAction, ConfigAction, ConfigFormat};
use crate::error::CliError;
use crate::project::Project;
use crate::scope::ScopeMode;
use crate::ui;
use anyhow::Context;

pub(super) fn run(project: &Project, command: Commands) -> Result<bool, CliError> {
    match command {
        Commands::Compat {
            action:
                CompatAction::Run {
                    input,
                    output,
                    old_result,
                },
        } => {
            let response = crate::compat::run(project, &input, &output, old_result.as_deref())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&response).map_err(anyhow::Error::from)?
            );
            Ok(response.status == "PASS"
                && response
                    .comparison
                    .as_ref()
                    .is_none_or(|result| result.equivalent))
        }
        Commands::Compat { .. } => {
            unreachable!("compatibility maintenance actions handled before project discovery")
        }
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
        Commands::Cleanup { dry_run, json } => {
            let report = crate::service::cleanup_resources(project, dry_run)?;
            crate::utils::fs::write_json(&project.reports.join("cleanup.json"), &report)
                .context("write cleanup evidence")?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(anyhow::Error::from)?
                );
            } else {
                println!(
                    "Cleanup {}: scanned {}, active {}, stale {}, reclaimed {}",
                    if dry_run { "(dry-run)" } else { "complete" },
                    report.scanned,
                    report.active,
                    report.stale,
                    report.reclaimed
                );
                for resource in &report.resources {
                    println!(
                        "  {:<12} {:<24} {}",
                        resource.action, resource.resource_id, resource.lease_file
                    );
                }
                for failure in &report.failures {
                    eprintln!("  cleanup failure: {failure}");
                }
            }
            Ok(report.failures.is_empty())
        }
        Commands::Scope {
            scope: args,
            json,
            benchmark_repeat,
        } => {
            if benchmark_repeat > 0 {
                let benchmark = crate::scope::benchmark(project, &args.mode(), benchmark_repeat)?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&benchmark).map_err(anyhow::Error::from)?
                );
                return Ok(true);
            }
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
                println!("{}", ui::pass("Secret scan passed"));
            } else {
                eprintln!(
                    "{}",
                    ui::error(format!("Secret scan failed in {} file(s):", findings.len()))
                );
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
                let summary = format!(
                    "Audit: {} violation(s), {} blocker(s), {} error(s), {} warning(s)",
                    outcome.total_violations,
                    outcome.blocker_count,
                    outcome.error_count,
                    outcome.warning_count
                );
                println!(
                    "{}",
                    if outcome.total_violations == 0 {
                        ui::pass(summary)
                    } else {
                        ui::failure(summary)
                    }
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
        Commands::Init { .. } | Commands::Presets | Commands::Schema { .. } => {
            unreachable!("handled before project discovery")
        }
    }
}

fn run_config(project: &Project, action: ConfigAction) -> Result<bool, CliError> {
    match action {
        ConfigAction::Check {
            format: ConfigFormat::Human,
        } => {
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
        ConfigAction::Check {
            format: ConfigFormat::Json,
        } => {
            let report = project.config.diagnostics_report();
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(anyhow::Error::from)?
            );
            Ok(report.valid)
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
