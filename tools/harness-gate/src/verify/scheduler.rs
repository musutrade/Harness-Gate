use super::plan::{BuiltinGate, NodeResult, NodeStatus, PlanNode, PlanNodeKind, VerificationPlan};
use super::steps::run_configured_step;
use crate::audit;
use crate::process::TaskResult;
use crate::project::Project;
use crate::secrets::{self, SecretMode};
use crate::service::ServiceManager;
use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, thiserror::Error)]
pub(super) enum SchedulerError {
    #[error(transparent)]
    Secrets(#[from] crate::secrets::SecretsError),
    #[error(transparent)]
    Audit(#[from] crate::audit::AuditError),
    #[error("verification scheduler failed: {0:#}")]
    Execution(#[from] anyhow::Error),
}

pub(super) struct ScheduledResult {
    pub(super) node_id: String,
    pub(super) node_result: NodeResult,
    pub(super) task_result: TaskResult,
}

pub(super) struct SchedulerOutcome {
    pub(super) results: Vec<ScheduledResult>,
    pub(super) cancelled: bool,
    /// Adapter failures are retained until the ordered publisher has written
    /// the report. This keeps failure evidence available even when a gate
    /// cannot produce its normal artifact.
    pub(super) failures: Vec<SchedulerFailure>,
}

pub(super) struct SchedulerFailure {
    pub(super) node_id: String,
    pub(super) error: SchedulerError,
}

/// Select the public adapter failure by stable plan position, independent of
/// worker completion order. All failures remain in the outcome for report
/// publication before this selector is applied.
pub(super) fn primary_failure<'a>(
    plan: &VerificationPlan<'a>,
    failures: Vec<SchedulerFailure>,
) -> Option<SchedulerFailure> {
    failures.into_iter().min_by_key(|failure| {
        plan.nodes
            .iter()
            .position(|node| node.id == failure.node_id)
            .unwrap_or(usize::MAX)
    })
}

enum WorkerResult {
    Completed(TaskResult),
    Failed(SchedulerError),
}

pub(super) fn run_plan<'a>(
    project: &'a Project,
    plan: &'a VerificationPlan<'a>,
    staged: bool,
    services: &'a Mutex<ServiceManager<'a>>,
    max_parallel: usize,
) -> std::result::Result<SchedulerOutcome, SchedulerError> {
    let limit = max_parallel.max(1);
    let nodes = &plan.nodes;
    let mut statuses = HashMap::<String, NodeStatus>::new();
    let mut pending = nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<HashSet<_>>();
    let mut running = HashSet::<String>::new();
    let mut results = Vec::with_capacity(nodes.len());
    let mut failures = Vec::new();
    let (sender, receiver) = mpsc::channel::<(String, WorkerResult)>();
    let mut cancellation_observed = false;

    std::thread::scope(|scope| {
        loop {
            if !cancellation_observed && crate::process::cancelled() {
                cancellation_observed = true;
                mark_pending(
                    nodes,
                    &mut pending,
                    &mut statuses,
                    &mut results,
                    NodeStatus::Cancelled,
                    "verification cancelled before dispatch",
                );
            }
            mark_blocked(nodes, &mut pending, &mut statuses, &mut results);

            if !cancellation_observed {
                let ready = ready_nodes(nodes, &pending, &statuses, running.len(), limit);
                for node in ready {
                    if crate::process::cancelled() {
                        break;
                    }
                    pending.remove(&node.id);
                    running.insert(node.id.clone());
                    let sender = sender.clone();
                    scope.spawn(move || {
                        let result = execute_node(project, node, staged, services);
                        let _ = sender.send((node.id.clone(), result));
                    });
                }
            }

            if pending.is_empty() && running.is_empty() {
                break;
            }
            if running.is_empty() {
                if cancellation_observed {
                    continue;
                }
                return Err(SchedulerError::Execution(anyhow::anyhow!("verification scheduler could not make progress; plan dependencies are inconsistent")));
            }

            let (node_id, worker_result) = receiver.recv().map_err(|_| {
                SchedulerError::Execution(anyhow::anyhow!(
                    "verification worker exited without returning a result"
                ))
            })?;
            running.remove(&node_id);
            let node = nodes
                .iter()
                .find(|candidate| candidate.id == node_id)
                .expect("worker node remains in plan");
            let task_result = match worker_result {
                WorkerResult::Completed(result) => result,
                WorkerResult::Failed(error) => {
                    let task_result = failed_task_result(node, &error, project);
                    failures.push(SchedulerFailure {
                        node_id: node_id.clone(),
                        error,
                    });
                    task_result
                }
            };
            let status = if task_result.cancelled {
                NodeStatus::Cancelled
            } else if task_result.passed {
                NodeStatus::Passed
            } else {
                NodeStatus::Failed
            };
            statuses.insert(node_id.clone(), status);
            let node_cancelled = status == NodeStatus::Cancelled;
            results.push(ScheduledResult {
                node_id,
                node_result: node_result(node, &task_result, status),
                task_result,
            });
            if node_cancelled || crate::process::cancelled() {
                cancellation_observed = true;
                mark_pending(
                    nodes,
                    &mut pending,
                    &mut statuses,
                    &mut results,
                    NodeStatus::Cancelled,
                    "verification cancelled before dispatch",
                );
            }
        }
        Ok(SchedulerOutcome {
            results,
            cancelled: cancellation_observed,
            failures,
        })
    })
}

/// Return the earliest ready nodes that fit in the remaining worker slots.
/// `nodes` is already in stable plan order, so retaining that iteration order
/// makes dispatch deterministic regardless of worker completion timing.
fn ready_nodes<'a>(
    nodes: &'a [PlanNode<'a>],
    pending: &HashSet<String>,
    statuses: &HashMap<String, NodeStatus>,
    running: usize,
    limit: usize,
) -> Vec<&'a PlanNode<'a>> {
    let slots = limit.saturating_sub(running);
    if slots == 0 {
        return Vec::new();
    }
    nodes
        .iter()
        .filter(|node| pending.contains(&node.id))
        .filter(|node| {
            node.depends_on
                .iter()
                .all(|dependency| statuses.get(dependency) == Some(&NodeStatus::Passed))
        })
        .take(slots)
        .collect()
}

fn execute_node<'a>(
    project: &'a Project,
    node: &PlanNode<'_>,
    staged: bool,
    services: &'a Mutex<ServiceManager<'a>>,
) -> WorkerResult {
    let result = match node.kind {
        PlanNodeKind::Builtin(BuiltinGate::SecretScan) => {
            run_secret_scan(project, staged, &node.label)
        }
        PlanNodeKind::Builtin(BuiltinGate::ArchitectureAudit) => {
            run_architecture_audit(project, &node.label)
        }
        PlanNodeKind::External => run_external_step(project, node, services),
    };
    match result {
        Ok(task_result) => WorkerResult::Completed(task_result),
        Err(error) => WorkerResult::Failed(error),
    }
}

fn failed_task_result(
    node: &PlanNode<'_>,
    error: &SchedulerError,
    project: &Project,
) -> TaskResult {
    let log = match node.kind {
        PlanNodeKind::Builtin(BuiltinGate::SecretScan) => project
            .reports
            .join("secret_scan.json")
            .to_string_lossy()
            .into_owned(),
        PlanNodeKind::Builtin(BuiltinGate::ArchitectureAudit) => project
            .reports
            .join("review_context.json")
            .to_string_lossy()
            .into_owned(),
        PlanNodeKind::External => node.step.map(|step| step.log.clone()).unwrap_or_default(),
    };
    TaskResult {
        label: node.label.clone(),
        passed: false,
        timed_out: false,
        cancelled: crate::process::cancelled(),
        duration_ms: 0,
        log,
        detail: Some(format!("{error:#}")),
    }
}

fn run_secret_scan(
    project: &Project,
    staged: bool,
    label: &str,
) -> std::result::Result<TaskResult, SchedulerError> {
    let started = Instant::now();
    let findings = secrets::scan(
        project,
        if staged {
            SecretMode::Staged
        } else {
            SecretMode::WorkingTree
        },
    )?;
    let passed = findings.is_empty();
    Ok(TaskResult {
        label: label.to_string(),
        passed,
        timed_out: false,
        cancelled: false,
        duration_ms: started.elapsed().as_millis(),
        log: project
            .reports
            .join("secret_scan.json")
            .to_string_lossy()
            .into(),
        detail: (!passed).then(|| format!("{} file(s) require review", findings.len())),
    })
}

fn run_architecture_audit(
    project: &Project,
    label: &str,
) -> std::result::Result<TaskResult, SchedulerError> {
    let started = Instant::now();
    let outcome = audit::run(
        &project.root,
        &project.audit_config,
        &project.reports,
        false,
    )?;
    let passed = outcome.total_violations == 0;
    Ok(TaskResult {
        label: label.to_string(),
        passed,
        timed_out: false,
        cancelled: false,
        duration_ms: started.elapsed().as_millis(),
        log: outcome.report_file.to_string_lossy().into(),
        detail: Some(format!(
            "{} violation(s), {} blocker(s), {} error(s), {} warning(s)",
            outcome.total_violations,
            outcome.blocker_count,
            outcome.error_count,
            outcome.warning_count
        )),
    })
}

fn run_external_step<'a>(
    project: &'a Project,
    node: &PlanNode<'_>,
    services: &'a Mutex<ServiceManager<'a>>,
) -> std::result::Result<TaskResult, SchedulerError> {
    let step = node.step.ok_or_else(|| {
        SchedulerError::Execution(anyhow::anyhow!("external node {:?} has no step", node.id))
    })?;
    Ok(
        run_configured_step(project, step, services).unwrap_or_else(|error| TaskResult {
            label: node.label.clone(),
            passed: false,
            timed_out: false,
            cancelled: crate::process::cancelled(),
            duration_ms: 0,
            log: step.log.clone(),
            detail: Some(format!("{error:#}")),
        }),
    )
}

fn mark_blocked<'a>(
    nodes: &[PlanNode<'a>],
    pending: &mut HashSet<String>,
    statuses: &mut HashMap<String, NodeStatus>,
    results: &mut Vec<ScheduledResult>,
) {
    for node in nodes {
        if pending.contains(&node.id)
            && node.depends_on.iter().any(|dependency| {
                statuses
                    .get(dependency)
                    .is_some_and(|status| *status != NodeStatus::Passed)
            })
        {
            pending.remove(&node.id);
            statuses.insert(node.id.clone(), NodeStatus::Skipped);
            results.push(skipped(node, "blocked by a failed prerequisite"));
        }
    }
}

fn node_result(node: &PlanNode<'_>, result: &TaskResult, status: NodeStatus) -> NodeResult {
    NodeResult {
        id: node.id.clone(),
        label: node.label.clone(),
        kind: node.kind,
        status,
        duration: Duration::from_millis(result.duration_ms as u64),
        detail: result.detail.clone(),
        artifact: (!result.log.is_empty()).then(|| result.log.clone()),
        reason: result.detail.clone(),
        timed_out: result.timed_out,
        cancelled: result.cancelled,
    }
}

fn skipped<'a>(node: &PlanNode<'a>, reason: &str) -> ScheduledResult {
    ScheduledResult {
        node_id: node.id.clone(),
        node_result: NodeResult {
            id: node.id.clone(),
            label: node.label.clone(),
            kind: node.kind,
            status: NodeStatus::Skipped,
            duration: Duration::ZERO,
            detail: Some(reason.into()),
            artifact: None,
            reason: Some(reason.into()),
            timed_out: false,
            cancelled: false,
        },
        task_result: TaskResult {
            label: node.label.clone(),
            passed: false,
            timed_out: false,
            cancelled: false,
            duration_ms: 0,
            log: node.step.map(|step| step.log.clone()).unwrap_or_default(),
            detail: Some(reason.into()),
        },
    }
}

fn mark_pending<'a>(
    nodes: &[PlanNode<'a>],
    pending: &mut HashSet<String>,
    statuses: &mut HashMap<String, NodeStatus>,
    results: &mut Vec<ScheduledResult>,
    status: NodeStatus,
    reason: &str,
) {
    for node in nodes {
        if pending.remove(&node.id) {
            statuses.insert(node.id.clone(), status);
            let mut result = skipped(node, reason);
            result.node_result.status = status;
            result.task_result.cancelled = status == NodeStatus::Cancelled;
            result.node_result.cancelled = status == NodeStatus::Cancelled;
            results.push(result);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, depends_on: &[&str]) -> PlanNode<'static> {
        PlanNode {
            id: id.into(),
            label: id.into(),
            kind: PlanNodeKind::External,
            depends_on: depends_on
                .iter()
                .map(|dependency| (*dependency).into())
                .collect(),
            step: None,
        }
    }

    #[test]
    fn ready_queue_respects_stable_order_and_limit() {
        let nodes = vec![node("a", &[]), node("b", &[]), node("c", &[])];
        let pending = nodes.iter().map(|node| node.id.clone()).collect();
        let statuses = HashMap::new();

        let selected = ready_nodes(&nodes, &pending, &statuses, 0, 2);
        assert_eq!(
            selected
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert!(ready_nodes(&nodes, &pending, &statuses, 2, 2).is_empty());
    }

    #[test]
    fn ready_queue_requires_all_dependencies_to_pass() {
        let nodes = vec![
            node("a", &[]),
            node("b", &["a"]),
            node("c", &["a", "b"]),
            node("d", &[]),
        ];
        let pending = ["b", "c", "d"].into_iter().map(str::to_string).collect();
        let mut statuses = HashMap::new();
        statuses.insert("a".into(), NodeStatus::Passed);

        let selected = ready_nodes(&nodes, &pending, &statuses, 0, 4);
        assert_eq!(
            selected
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            ["b", "d"]
        );
    }

    #[test]
    fn cancellation_marks_queued_nodes_without_satisfying_dependencies() {
        let nodes = vec![node("a", &[]), node("b", &["a"]), node("c", &[])];
        let mut pending = nodes.iter().map(|node| node.id.clone()).collect();
        let mut statuses = HashMap::new();
        let mut results = Vec::new();
        mark_pending(
            &nodes,
            &mut pending,
            &mut statuses,
            &mut results,
            NodeStatus::Cancelled,
            "cancelled before dispatch",
        );
        assert!(pending.is_empty());
        assert!(statuses
            .values()
            .all(|status| *status == NodeStatus::Cancelled));
        assert!(ready_nodes(&nodes, &pending, &statuses, 0, 2).is_empty());
        assert!(results.iter().all(|result| result.node_result.cancelled));
    }

    #[test]
    fn failed_ancestor_marks_descendant_skipped() {
        let nodes = vec![node("a", &[]), node("b", &["a"]), node("c", &[])];
        let mut pending = nodes.iter().map(|node| node.id.clone()).collect();
        let mut statuses = HashMap::from([("a".into(), NodeStatus::Failed)]);
        let mut results = Vec::new();
        mark_blocked(&nodes, &mut pending, &mut statuses, &mut results);
        assert_eq!(statuses.get("b"), Some(&NodeStatus::Skipped));
        assert!(pending.contains("c"));
        assert_eq!(results.len(), 1);
    }
}
