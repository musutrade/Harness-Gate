mod parser;
mod plan;
mod report;
mod scheduler;
mod steps;

#[cfg(test)]
mod tests;

use crate::error::CodedError;
use crate::process::TaskResult;
use crate::project::Project;
use crate::scope::ScopeResult;
use crate::service::ServiceManager;
use crate::ui::{self, Progress};
use plan::VerificationPlan;
use serde::Serialize;
use std::collections::BTreeSet;
use std::sync::Mutex;
use steps::{print_external_result, print_result};

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
    run_selected(
        project,
        explicit_scope(std::slice::from_ref(&step.component)),
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
    if project
        .config
        .execution
        .max_parallel
        .is_some_and(|limit| limit == 0 || limit > 64)
    {
        return Err(VerifyError::execution(anyhow::anyhow!(
            "execution.max_parallel must be between 1 and 64"
        )));
    }
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

    let plan = VerificationPlan::build(project, &scope, profile, only_step).map_err(|error| {
        if error.to_string().contains("unknown verification step") {
            VerifyError::UnknownStep {
                id: only_step.unwrap_or_default().to_string(),
            }
        } else {
            VerifyError::execution(error)
        }
    })?;
    let mut progress = Progress::new(plan.nodes.len());
    scope.write_reports(project)?;
    let services = Mutex::new(ServiceManager::new(project));
    let outcome = scheduler::run_plan(
        project,
        &plan,
        staged,
        &services,
        if project.config.execution.parallel {
            project.config.execution.effective_max_parallel()
        } else {
            1
        },
    )
    .map_err(|error| match error {
        scheduler::SchedulerError::Secrets(error) => VerifyError::Secrets(error),
        scheduler::SchedulerError::Audit(error) => VerifyError::Audit(error),
        scheduler::SchedulerError::Execution(error) => VerifyError::execution(error),
    })?;
    let scheduler::SchedulerOutcome {
        results,
        cancelled,
        failures,
    } = outcome;
    let mut ordered = results;
    ordered.sort_by_key(|result| {
        plan.nodes
            .iter()
            .position(|node| node.id == result.node_id)
            .unwrap_or(usize::MAX)
    });
    let mut steps = Vec::new();
    for result in ordered {
        progress.clear();
        if result.node_result.status != plan::NodeStatus::Skipped {
            match result.node_result.kind {
                plan::PlanNodeKind::Builtin(_) => print_result(&result.task_result),
                plan::PlanNodeKind::External => {
                    if progress.enabled() {
                        print_result(&result.task_result);
                    } else {
                        print_external_result(&result.task_result);
                    }
                }
            }
            steps.push(result.task_result);
        }
        progress.complete();
    }
    let passed = steps.iter().all(|step| step.passed) && steps.len() == plan.nodes.len();
    let report = VerificationReport {
        timestamp: chrono::Utc::now().to_rfc3339(),
        profile: only_step
            .map(|id| format!("step:{id}"))
            .unwrap_or_else(|| profile.to_string()),
        scope,
        steps,
        passed,
    };
    report::write(&report, project).map_err(VerifyError::report)?;
    report::notify(&report, project).map_err(VerifyError::report)?;
    progress.finish();
    println!(
        "\nVerification report: {}",
        project.reports.join("test_result.md").display()
    );
    println!(
        "TEST_SUMMARY: {}",
        if report.passed { "PASS" } else { "FAIL" }
    );

    // Publication happens before returning cancellation or adapter failures so
    // callers retain the same report and log evidence as a normal run.
    if cancelled {
        return Err(VerifyError::Cancelled);
    }
    if let Some(failure) = scheduler::primary_failure(&plan, failures) {
        return Err(match failure.error {
            scheduler::SchedulerError::Secrets(error) => VerifyError::Secrets(error),
            scheduler::SchedulerError::Audit(error) => VerifyError::Audit(error),
            scheduler::SchedulerError::Execution(error) => VerifyError::execution(error),
        });
    }
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
