mod parser;
mod plan;
mod steps;

#[cfg(test)]
mod tests;

use crate::audit;
use crate::error::CodedError;
use crate::process::TaskResult;
use crate::project::Project;
use crate::scope::ScopeResult;
use crate::secrets::{self, SecretMode};
use crate::ui::{self, Progress};
use crate::utils::fs as output_fs;
use anyhow::Result;
use serde::Serialize;
use std::collections::BTreeSet;
use std::time::Instant;

use plan::selected_steps;
use steps::{print_result, run_configured_steps};

/// Errors emitted by the verification workflow boundary.
#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("unknown or empty verification profile {profile:?}")]
    UnknownProfile { profile: String },
    #[error("unknown verification step {id:?}")]
    UnknownStep { id: String },
    #[error("verification was cancelled")]
    Cancelled,
    #[error("verification execution failed: {message}")]
    Execution { message: String },
    #[error("verification report failed: {message}")]
    Report { message: String },
    #[error(transparent)]
    Scope(#[from] crate::scope::ScopeError),
    #[error(transparent)]
    Secrets(#[from] crate::secrets::SecretsError),
    #[error(transparent)]
    Audit(#[from] crate::audit::AuditError),
}

impl VerifyError {
    fn execution(error: anyhow::Error) -> Self {
        Self::Execution {
            message: format!("{error:#}"),
        }
    }

    fn report(error: anyhow::Error) -> Self {
        Self::Report {
            message: format!("{error:#}"),
        }
    }
}

impl CodedError for VerifyError {
    fn code(&self) -> &'static str {
        match self {
            Self::UnknownProfile { .. } | Self::UnknownStep { .. } => "E1401",
            Self::Cancelled => "E1402",
            Self::Execution { .. } => "E1403",
            Self::Report { .. } => "E1404",
            Self::Scope(error) => error.code(),
            Self::Secrets(error) => error.code(),
            Self::Audit(error) => error.code(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct VerificationReport {
    pub timestamp: String,
    pub profile: String,
    pub scope: ScopeResult,
    pub steps: Vec<TaskResult>,
    pub passed: bool,
}

impl VerificationReport {
    fn write(&self, project: &Project) -> Result<()> {
        output_fs::write_json(&project.reports.join("test_result.json"), self)?;

        let mut markdown = String::from("=== Verification report ===\n");
        markdown.push_str(&format!("Timestamp: {}\n", self.timestamp));
        markdown.push_str(&format!("Profile: {}\n", self.profile));
        markdown.push_str(&format!(
            "Components: {}\n\n",
            self.scope
                .components
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(",")
        ));
        for step in &self.steps {
            let status = if step.passed { "PASS" } else { "FAIL" };
            markdown.push_str(&format!(
                "- {status}: {} ({} ms)",
                step.label, step.duration_ms
            ));
            if let Some(detail) = &step.detail {
                markdown.push_str(&format!(" - {detail}"));
            }
            if !step.passed {
                markdown.push_str(&format!("; log: {}", step.log));
            }
            markdown.push('\n');
        }
        markdown.push_str(&format!(
            "\nTEST_SUMMARY: {}\n",
            if self.passed { "PASS" } else { "FAIL" }
        ));
        output_fs::write(&project.reports.join("test_result.md"), markdown)?;
        Ok(())
    }
}

pub fn run(
    project: &Project,
    scope: ScopeResult,
    profile: &str,
    staged: bool,
) -> std::result::Result<VerificationReport, VerifyError> {
    if !project
        .config
        .steps
        .iter()
        .any(|step| step.profiles.contains(profile))
    {
        return Err(VerifyError::UnknownProfile {
            profile: profile.to_string(),
        });
    }
    run_selected(project, scope, profile, staged, None)
}

pub fn run_step(
    project: &Project,
    id: &str,
) -> std::result::Result<VerificationReport, VerifyError> {
    let step = project
        .config
        .step(id)
        .ok_or_else(|| VerifyError::UnknownStep { id: id.to_string() })?;
    let scope = explicit_scope(std::slice::from_ref(&step.component));
    run_selected(
        project,
        scope,
        &project.config.project.default_profile,
        false,
        Some(id),
    )
}

fn run_selected(
    project: &Project,
    scope: ScopeResult,
    profile: &str,
    staged: bool,
    only_step: Option<&str>,
) -> std::result::Result<VerificationReport, VerifyError> {
    println!("{}", ui::heading("arc-flow verify"));
    println!("Scope: {}", scope.mode);
    println!(
        "Components: {}\n",
        if scope.components.is_empty() {
            "none".to_string()
        } else {
            scope
                .components
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        }
    );

    let selected_steps = selected_steps(project, &scope, profile, only_step);
    let mut progress = Progress::new(2 + selected_steps.len());
    scope.write_reports(project)?;
    let mut steps = Vec::new();
    let secret_mode = if staged {
        SecretMode::Staged
    } else {
        SecretMode::WorkingTree
    };
    progress.begin("secret scan");
    let secret_started = Instant::now();
    let findings = secrets::scan(project, secret_mode)?;
    let secret_passed = findings.is_empty();
    let secret_result = TaskResult {
        label: "secret scan".to_string(),
        passed: secret_passed,
        timed_out: false,
        cancelled: false,
        duration_ms: secret_started.elapsed().as_millis(),
        log: project
            .reports
            .join("secret_scan.json")
            .to_string_lossy()
            .to_string(),
        detail: (!secret_passed).then(|| format!("{} file(s) require review", findings.len())),
    };
    progress.clear();
    print_result(&secret_result);
    progress.complete();
    steps.push(secret_result);

    if secret_passed {
        progress.begin("architecture audit");
        let audit_started = Instant::now();
        let outcome = audit::run(
            &project.root,
            &project.audit_config,
            &project.reports,
            false,
        )?;
        let audit_passed = outcome.total_violations == 0;
        let audit_result = TaskResult {
            label: "architecture audit".to_string(),
            passed: audit_passed,
            timed_out: false,
            cancelled: false,
            duration_ms: audit_started.elapsed().as_millis(),
            log: outcome.report_file.to_string_lossy().to_string(),
            detail: Some(format!(
                "{} violation(s), {} blocker(s), {} error(s), {} warning(s)",
                outcome.total_violations,
                outcome.blocker_count,
                outcome.error_count,
                outcome.warning_count
            )),
        };
        progress.clear();
        print_result(&audit_result);
        progress.complete();
        steps.push(audit_result);
    }

    if crate::process::cancelled() {
        return Err(VerifyError::Cancelled);
    }
    if steps.iter().all(|step| step.passed) {
        run_configured_steps(project, selected_steps, &mut steps, &mut progress).map_err(
            |error| {
                if error.to_string().contains("verification cancelled") {
                    VerifyError::Cancelled
                } else {
                    VerifyError::execution(error)
                }
            },
        )?;
    }

    let passed = steps.iter().all(|step| step.passed);
    let report = VerificationReport {
        timestamp: chrono::Utc::now().to_rfc3339(),
        profile: only_step
            .map(|id| format!("step:{id}"))
            .unwrap_or_else(|| profile.to_string()),
        scope,
        steps,
        passed,
    };
    report.write(project).map_err(VerifyError::report)?;
    progress.finish();
    println!(
        "\nVerification report: {}",
        project.reports.join("test_result.md").display()
    );
    println!(
        "TEST_SUMMARY: {}",
        if report.passed { "PASS" } else { "FAIL" }
    );
    Ok(report)
}
pub fn explicit_scope(components: &[String]) -> ScopeResult {
    ScopeResult {
        mode: "components".to_string(),
        changed_files: Vec::new(),
        components: components.iter().cloned().collect::<BTreeSet<_>>(),
        unmatched_files: Vec::new(),
    }
}
