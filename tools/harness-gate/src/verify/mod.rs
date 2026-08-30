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
use std::path::Path;
use std::sync::Mutex;
use steps::{print_external_result, print_result};

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("unknown or empty verification profile {profile:?}")]
    UnknownProfile { profile: String },
    #[error("unknown verification step {id:?}")]
    UnknownStep { id: String },
    #[error("verification step {id:?} is not part of profile {profile:?}")]
    StepNotInProfile { id: String, profile: String },
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
            Self::UnknownProfile { .. }
            | Self::UnknownStep { .. }
            | Self::StepNotInProfile { .. } => "E1401",
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
    pub invocation_id: String,
    pub executor_version: String,
    pub report_directory: String,
    pub timestamp: String,
    pub profile: String,
    pub scope: ScopeResult,
    pub steps: Vec<TaskResult>,
    /// Steps that were not dispatched because a prerequisite failed.
    /// Successful reports omit this field to preserve the existing JSON shape.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped_steps: Vec<SkippedStep>,
    pub passed: bool,
}

#[derive(Debug, Serialize)]
pub struct SkippedStep {
    pub id: String,
    pub label: String,
    pub reason: String,
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
    let profile = project.config.project.default_profile.clone();
    if !step.profiles.contains(&profile) {
        return Err(VerifyError::StepNotInProfile {
            id: id.to_string(),
            profile,
        });
    }
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
    println!("{}", ui::heading("harness-gate verify"));
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
    let invocation = report::allocate_invocation(project).map_err(VerifyError::report)?;
    report::write_invocation_metadata(&invocation, profile, staged).map_err(VerifyError::report)?;
    let mut invocation_project = project.clone();
    invocation_project.reports = invocation.root.clone();
    let mut progress = Progress::new(plan.nodes.len());
    scope.write_reports(&invocation_project)?;
    let services = Mutex::new(ServiceManager::new(&invocation_project));
    let scheduler_result = scheduler::run_plan(
        &invocation_project,
        &plan,
        staged,
        &services,
        if project.config.execution.parallel {
            project.config.execution.effective_max_parallel()
        } else {
            1
        },
    );
    let cleanup_result = services
        .lock()
        .map_err(|_| anyhow::anyhow!("service manager lock was poisoned"))
        .and_then(|mut manager| manager.cleanup());
    let outcome = match scheduler_result {
        Ok(outcome) => outcome,
        Err(error) => {
            let cleanup = cleanup_result.err().map(|cleanup| format!("; {cleanup:#}"));
            let detail = match error {
                scheduler::SchedulerError::Secrets(error) => VerifyError::Secrets(error),
                scheduler::SchedulerError::Audit(error) => VerifyError::Audit(error),
                scheduler::SchedulerError::Execution(error) => VerifyError::execution(
                    anyhow::anyhow!("{error:#}{}", cleanup.unwrap_or_default()),
                ),
            };
            return Err(detail);
        }
    };
    let cleanup_error = cleanup_result.err();
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
    let mut skipped_steps = Vec::new();
    for result in ordered {
        progress.clear();
        if result.node_result.status == plan::NodeStatus::Skipped {
            skipped_steps.push(SkippedStep {
                id: result.node_result.id,
                label: result.node_result.label,
                reason: result
                    .node_result
                    .reason
                    .unwrap_or_else(|| "blocked by a failed prerequisite".into()),
            });
        } else {
            let mut task_result = result.task_result;
            task_result.step_id = Some(result.node_id.clone());
            task_result.invocation_id = Some(invocation.id.clone());
            task_result.attempt = Some(1);
            if !task_result.log.is_empty() && !Path::new(&task_result.log).is_absolute() {
                task_result.log = invocation_project
                    .reports
                    .join("logs")
                    .join(&task_result.log)
                    .to_string_lossy()
                    .into_owned();
            }
            match result.node_result.kind {
                plan::PlanNodeKind::Builtin(_) => print_result(&task_result),
                plan::PlanNodeKind::External => {
                    if progress.enabled() {
                        print_result(&task_result);
                    } else {
                        print_external_result(&task_result);
                    }
                }
            }
            steps.push(task_result);
        }
        progress.complete();
    }
    if let Some(error) = &cleanup_error {
        steps.push(TaskResult {
            step_id: Some("service.cleanup".into()),
            invocation_id: Some(invocation.id.clone()),
            attempt: Some(1),
            started_at: None,
            finished_at: None,
            label: "service cleanup".into(),
            passed: false,
            timed_out: false,
            cancelled: false,
            duration_ms: 0,
            log: String::new(),
            detail: Some(format!("{error:#}")),
            runner: None,
        });
    }
    let passed = cleanup_error.is_none()
        && steps.iter().all(|step| step.passed)
        && steps.len() == plan.nodes.len();
    let report = VerificationReport {
        invocation_id: invocation.id.clone(),
        executor_version: env!("CARGO_PKG_VERSION").into(),
        report_directory: invocation.root.to_string_lossy().into_owned(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        profile: only_step
            .map(|id| format!("step:{id}"))
            .unwrap_or_else(|| profile.to_string()),
        scope,
        steps,
        skipped_steps,
        passed,
    };
    report::write(&report, &invocation_project).map_err(VerifyError::report)?;
    report::mirror_legacy_outputs(&invocation_project, project).map_err(VerifyError::report)?;
    report::notify(&report, &invocation_project).map_err(VerifyError::report)?;
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
    if let Some(error) = cleanup_error {
        return Err(VerifyError::execution(error));
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
