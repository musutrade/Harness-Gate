use super::plan::{NodeResult, NodeStatus, PlanNode};
use super::steps::run_configured_step;
use crate::process::TaskResult;
use crate::project::Project;
use crate::service::ServiceManager;
use anyhow::{bail, Result};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Duration;

/// Result returned by a worker. Workers never publish output directly.
pub(super) struct ScheduledResult {
    pub(super) node_id: String,
    pub(super) node_result: NodeResult,
    pub(super) task_result: TaskResult,
}

pub(super) struct SchedulerOutcome {
    pub(super) results: Vec<ScheduledResult>,
    pub(super) cancelled: bool,
}

/// Execute external plan nodes in stable ready-queue batches.
pub(super) fn run_external<'a>(
    project: &'a Project,
    nodes: &[PlanNode<'a>],
    initial_statuses: &HashMap<String, NodeStatus>,
    services: &'a Mutex<ServiceManager<'a>>,
    max_parallel: usize,
) -> Result<SchedulerOutcome> {
    let limit = max_parallel.max(1);
    let mut statuses = initial_statuses.clone();
    let mut pending = nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<HashSet<_>>();
    let mut results = Vec::new();

    loop {
        if pending.is_empty() {
            return Ok(SchedulerOutcome {
                results,
                cancelled: false,
            });
        }

        if crate::process::cancelled() {
            mark_pending(
                nodes,
                &mut pending,
                &mut statuses,
                &mut results,
                NodeStatus::Cancelled,
                "verification cancelled before dispatch",
            );
            return Ok(SchedulerOutcome {
                results,
                cancelled: true,
            });
        }

        let mut ready = Vec::new();
        for node in nodes {
            if !pending.contains(&node.id) {
                continue;
            }
            if node.depends_on.iter().any(|dependency| {
                statuses
                    .get(dependency)
                    .is_some_and(|status| *status != NodeStatus::Passed)
            }) {
                pending.remove(&node.id);
                statuses.insert(node.id.clone(), NodeStatus::Skipped);
                results.push(skipped(node, "blocked by a failed prerequisite"));
            } else if node.depends_on.iter().all(|dependency| {
                statuses
                    .get(dependency)
                    .is_some_and(|status| *status == NodeStatus::Passed)
            }) {
                ready.push(node);
            }
        }

        if pending.is_empty() {
            return Ok(SchedulerOutcome {
                results,
                cancelled: false,
            });
        }

        if ready.is_empty() {
            bail!("verification scheduler could not make progress; plan dependencies are inconsistent");
        }

        let batch = ready.into_iter().take(limit).collect::<Vec<_>>();
        let worker_results = std::thread::scope(|scope| {
            let handles = batch
                .iter()
                .map(|node| {
                    scope.spawn(move || {
                        let task_result = run_configured_step(
                            project,
                            node.step.ok_or_else(|| {
                                anyhow::anyhow!("external node {:?} has no step", node.id)
                            })?,
                            services,
                        )?;
                        Ok::<_, anyhow::Error>((node.id.clone(), task_result))
                    })
                })
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .map(|handle| {
                    handle
                        .join()
                        .map_err(|_| anyhow::anyhow!("verification worker panicked"))?
                })
                .collect::<Result<Vec<_>>>()
        })?;

        for (node_id, task_result) in worker_results {
            let node = nodes
                .iter()
                .find(|candidate| candidate.id == node_id)
                .expect("worker node remains in plan");
            let status = if task_result.cancelled {
                NodeStatus::Cancelled
            } else if task_result.passed {
                NodeStatus::Passed
            } else {
                NodeStatus::Failed
            };
            pending.remove(&node_id);
            statuses.insert(node_id.clone(), status);
            let cancelled = status == NodeStatus::Cancelled;
            results.push(ScheduledResult {
                node_id,
                node_result: NodeResult {
                    id: node.id.clone(),
                    label: node.label.clone(),
                    kind: node.kind,
                    status,
                    duration: Duration::from_millis(task_result.duration_ms as u64),
                    detail: task_result.detail.clone(),
                    artifact: (!task_result.log.is_empty()).then(|| task_result.log.clone()),
                    reason: task_result.detail.clone(),
                },
                task_result,
            });
            if cancelled || crate::process::cancelled() {
                mark_pending(
                    nodes,
                    &mut pending,
                    &mut statuses,
                    &mut results,
                    NodeStatus::Cancelled,
                    "verification cancelled before dispatch",
                );
                return Ok(SchedulerOutcome {
                    results,
                    cancelled: true,
                });
            }
        }
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
            detail: Some(reason.to_string()),
            artifact: None,
            reason: Some(reason.to_string()),
        },
        task_result: TaskResult {
            label: node.label.clone(),
            passed: false,
            timed_out: false,
            cancelled: false,
            duration_ms: 0,
            log: node.step.map(|step| step.log.clone()).unwrap_or_default(),
            detail: Some(reason.to_string()),
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
            result.task_result.detail = Some(reason.to_string());
            results.push(result);
        }
    }
}
