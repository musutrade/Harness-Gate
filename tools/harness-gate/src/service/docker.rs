use super::{remaining, RunningService};
use crate::config::ContainerRuntimeKind;
use crate::project::Project;
use crate::service::runtime::{ContainerRuntime, ContainerStartOptions};
use crate::service::ResourceLease;
use anyhow::{bail, Result};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(super) struct DockerStartOptions {
    pub(super) runtime: ContainerRuntimeKind,
    pub(super) image: String,
    pub(super) inject_env: String,
    pub(super) startup_timeout_secs: u64,
    pub(super) container_port: u16,
    pub(super) environment: BTreeMap<String, String>,
    pub(super) healthcheck: Vec<String>,
    pub(super) connection: String,
    pub(super) deadline: Instant,
    pub(super) cleanup_errors: Arc<Mutex<Vec<String>>>,
    pub(super) invocation_id: String,
}
pub(super) fn start_docker(
    project: &Project,
    id: &str,
    options: DockerStartOptions,
) -> Result<RunningService> {
    let DockerStartOptions {
        runtime,
        image,
        inject_env,
        startup_timeout_secs,
        container_port,
        environment,
        healthcheck,
        connection,
        deadline,
        cleanup_errors,
        invocation_id,
    } = options;
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let name = format!(
        "harness-gate-{}-{id}-{}-{unique}",
        project.config.project.name,
        std::process::id()
    );
    let lease = ResourceLease::acquire(
        project,
        format!("service:{id}"),
        "container",
        invocation_id.clone(),
        Some(name.clone()),
        Some(runtime),
    )?;
    let executable = runtime.executable();
    runtime
        .start_container(
            project,
            ContainerStartOptions {
                name: &name,
                image: &image,
                environment: &environment,
                labels: &std::collections::BTreeMap::from([
                    ("harness-gate.owner".to_string(), "harness-gate".to_string()),
                    ("harness-gate.resource".to_string(), format!("service:{id}")),
                    ("harness-gate.invocation".to_string(), invocation_id),
                ]),
                container_port,
            },
            remaining(deadline)?,
        )
        .map_err(|error| anyhow::anyhow!("start {executable} service {id:?}: {error:#}"))?;

    let mut running = RunningService {
        runtime,
        inject_env,
        value: String::new(),
        container: Some(name),
        project_root: project.root.clone(),
        cleanup_errors,
        lease: Some(lease),
    };
    while Instant::now() < deadline {
        if let Some(lease) = &running.lease {
            lease.renew()?;
        }
        if crate::process::cancelled() {
            bail!("verification cancelled while waiting for service {id:?}");
        }
        if let Some(container) = running.container.as_deref() {
            if let Some(port) =
                runtime.mapped_port(project, container, container_port, remaining(deadline)?)?
            {
                let ready = runtime.run_healthcheck(
                    project,
                    container,
                    &healthcheck,
                    remaining(deadline)?,
                )?;
                if ready {
                    running.value = connection.replace("{host_port}", &port);
                    return Ok(running);
                }
            }
        }
        thread::sleep(Duration::from_secs(1));
    }
    bail!("service {id:?} did not become ready within {startup_timeout_secs} seconds")
}
