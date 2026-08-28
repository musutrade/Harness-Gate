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
use std::collections::{BTreeSet, HashSet};
use std::time::Instant;

use crate::service::ServiceManager;
use plan::{BuiltinGate, NodeResult, NodeStatus, PlanNodeKind, VerificationPlan};
use steps::{print_result, run_configured_step};

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
    let mut steps = Vec::new();
    let mut services = ServiceManager::new(project);
    let mut blocked = HashSet::new();
    let mut node_results = Vec::<NodeResult>::new();
    let secret_mode = if staged {
        SecretMode::Staged
    } else {
        SecretMode::WorkingTree
    };
    for node in &plan.nodes {
        if crate::process::cancelled() {
            return Err(VerifyError::Cancelled);
        }
        if node
            .depends_on
            .iter()
            .any(|dependency| blocked.contains(dependency))
        {
            blocked.insert(node.id.clone());
            node_results.push(NodeResult {
                id: node.id.clone(),
                label: node.label.clone(),
                kind: node.kind,
                status: NodeStatus::Skipped,
                duration: std::time::Duration::ZERO,
                detail: Some("blocked by a failed prerequisite".into()),
                artifact: None,
                reason: Some("prerequisite failed".into()),
            });
            continue;
        }
        match node.kind {
            PlanNodeKind::Builtin(BuiltinGate::SecretScan) => {
                progress.begin(&node.label);
                let started = Instant::now();
                let findings = secrets::scan(project, secret_mode)?;
                let passed = findings.is_empty();
                let result = TaskResult {
                    label: "secret scan".to_string(),
                    passed,
                    timed_out: false,
                    cancelled: false,
                    duration_ms: started.elapsed().as_millis(),
                    log: project
                        .reports
                        .join("secret_scan.json")
                        .to_string_lossy()
                        .to_string(),
                    detail: (!passed).then(|| format!("{} file(s) require review", findings.len())),
                };
                progress.clear();
                print_result(&result);
                progress.complete();
                node_results.push(node_result(
                    node,
                    &result,
                    NodeStatus::from_passed(result.passed),
                ));
                steps.push(result);
                if !passed {
                    blocked.insert(node.id.clone());
                }
            }
            PlanNodeKind::Builtin(BuiltinGate::ArchitectureAudit) => {
                progress.begin(&node.label);
                let started = Instant::now();
                let outcome = audit::run(
                    &project.root,
                    &project.audit_config,
                    &project.reports,
                    false,
                )?;
                let passed = outcome.total_violations == 0;
                let result = TaskResult {
                    label: "architecture audit".to_string(),
                    passed,
                    timed_out: false,
                    cancelled: false,
                    duration_ms: started.elapsed().as_millis(),
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
                print_result(&result);
                progress.complete();
                node_results.push(node_result(
                    node,
                    &result,
                    NodeStatus::from_passed(result.passed),
                ));
                steps.push(result);
                if !passed {
                    blocked.insert(node.id.clone());
                }
            }
            PlanNodeKind::External => {
                if let Some(step) = node.step {
                    run_configured_step(project, step, &mut services, &mut steps, &mut progress)
                        .map_err(|error| {
                            if error.to_string().contains("verification cancelled") {
                                VerifyError::Cancelled
                            } else {
                                VerifyError::execution(error)
                            }
                        })?;
                    if let Some(result) = steps.last() {
                        node_results.push(node_result(node, result, NodeStatus::from_task(result)));
                    }
                    if !steps.last().is_some_and(|step| step.passed) {
                        blocked.insert(node.id.clone());
                    }
                }
            }
        }
    }

    let passed = node_results
        .iter()
        .all(|result| result.status == NodeStatus::Passed);
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

fn node_result(node: &plan::PlanNode<'_>, result: &TaskResult, status: NodeStatus) -> NodeResult {
    NodeResult {
        id: node.id.clone(),
        label: node.label.clone(),
        kind: node.kind,
        status,
        duration: std::time::Duration::from_millis(result.duration_ms as u64),
        detail: result.detail.clone(),
        artifact: (!result.log.is_empty()).then(|| result.log.clone()),
        reason: result.detail.clone(),
    }
}
