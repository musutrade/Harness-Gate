mod docker;
mod postgres;
mod runtime;

#[cfg(test)]
mod tests;

use crate::config::ServiceConfig;
use crate::project::Project;
use anyhow::{bail, Context, Result};
use docker::{start_docker, DockerStartOptions};
use postgres::validate_external_value;
use runtime::ContainerRuntime;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

const RESOURCE_LOCK_POLL: Duration = Duration::from_millis(25);
const RESOURCE_LOCK_WAIT: Duration = Duration::from_secs(30);

pub struct ServiceManager<'a> {
    project: &'a Project,
    resources: BTreeMap<String, Arc<ServiceResource>>,
}

impl<'a> ServiceManager<'a> {
    pub fn new(project: &'a Project) -> Self {
        Self {
            project,
            resources: BTreeMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn environment(&mut self, id: &str) -> Result<(String, String)> {
        let lease = self.handle(id)?.acquire()?;
        Ok(lease.environment())
    }

    pub(super) fn handle(&mut self, id: &str) -> Result<ServiceHandle<'a>> {
        let config = self
            .project
            .config
            .service(id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unknown service {id:?}"))?;
        let resource = self
            .resources
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(ServiceResource::new(is_shareable(&config))))
            .clone();
        Ok(ServiceHandle {
            project: self.project,
            id: id.to_string(),
            config,
            resource,
        })
    }
}

/// A service resource is shareable only when its adapter can safely serve
/// multiple child processes without mutable teardown state. Environment values
/// are immutable; managed containers remain exclusive until a future adapter
/// explicitly provides a shared lifecycle contract.
fn is_shareable(config: &ServiceConfig) -> bool {
    matches!(config, ServiceConfig::Environment { .. })
}

struct ServiceResource {
    state: Mutex<ResourceState>,
    changed: Condvar,
    shareable: bool,
}

enum ResourceState {
    Empty,
    Starting,
    Ready {
        service: RunningService,
        users: usize,
    },
    /// A non-shareable service is being torn down after its final lease.
    /// Keeping this state visible prevents a new startup from racing the
    /// adapter's cleanup transition.
    Stopping,
    Failed(String),
}

impl ServiceResource {
    fn new(shareable: bool) -> Self {
        Self {
            state: Mutex::new(ResourceState::Empty),
            changed: Condvar::new(),
            shareable,
        }
    }

    fn acquire(
        self: &Arc<Self>,
        project: &Project,
        id: &str,
        config: ServiceConfig,
    ) -> Result<ServiceLease> {
        let started = Instant::now();
        loop {
            let mut state = self
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("service resource lock was poisoned"))?;
            match &mut *state {
                ResourceState::Ready { service, users } if self.shareable || *users == 0 => {
                    *users += 1;
                    return Ok(ServiceLease {
                        resource: Arc::clone(self),
                        inject_env: service.inject_env.clone(),
                        value: service.value.clone(),
                    });
                }
                ResourceState::Ready { .. } | ResourceState::Starting | ResourceState::Stopping => {
                    if crate::process::cancelled() {
                        bail!("verification cancelled while waiting for service {id:?} resource");
                    }
                    let remaining = RESOURCE_LOCK_WAIT.saturating_sub(started.elapsed());
                    if remaining.is_zero() {
                        bail!("timed out waiting for service {id:?} resource");
                    }
                    let wait = remaining.min(RESOURCE_LOCK_POLL);
                    let (next, _) = self
                        .changed
                        .wait_timeout(state, wait)
                        .map_err(|_| anyhow::anyhow!("service resource lock was poisoned"))?;
                    drop(next);
                }
                ResourceState::Failed(error) => bail!("service {id:?} previously failed: {error}"),
                ResourceState::Empty => {
                    if crate::process::cancelled() {
                        bail!("verification cancelled while acquiring service {id:?} resource");
                    }
                    *state = ResourceState::Starting;
                    drop(state);
                    let result = RunningService::start(project, id, config);
                    // A cancellation can arrive while an adapter is starting.
                    // Drop any partial service and return the resource to Empty
                    // instead of publishing it as ready after cancellation.
                    if crate::process::cancelled() {
                        drop(result);
                        let mut state = self
                            .state
                            .lock()
                            .map_err(|_| anyhow::anyhow!("service resource lock was poisoned"))?;
                        *state = ResourceState::Empty;
                        self.changed.notify_all();
                        bail!("verification cancelled while starting service {id:?}");
                    }
                    let mut state = self
                        .state
                        .lock()
                        .map_err(|_| anyhow::anyhow!("service resource lock was poisoned"))?;
                    match result {
                        Ok(service) => {
                            let inject_env = service.inject_env.clone();
                            let value = service.value.clone();
                            *state = ResourceState::Ready { service, users: 1 };
                            self.changed.notify_all();
                            return Ok(ServiceLease {
                                resource: Arc::clone(self),
                                inject_env,
                                value,
                            });
                        }
                        Err(error) => {
                            let detail = format!("{error:#}");
                            *state = ResourceState::Failed(detail.clone());
                            self.changed.notify_all();
                            bail!("{detail}");
                        }
                    }
                }
            }
        }
    }

    fn release(&self) {
        let service = {
            let Ok(mut state) = self.state.lock() else {
                return;
            };
            let ResourceState::Ready { users, .. } = &mut *state else {
                return;
            };
            *users = users.saturating_sub(1);
            if self.shareable || *users != 0 {
                self.changed.notify_all();
                return;
            }

            // Move the managed instance out before dropping the lock. The
            // Stopping marker makes waiters observe the teardown transition.
            let previous = std::mem::replace(&mut *state, ResourceState::Stopping);
            self.changed.notify_all();
            match previous {
                ResourceState::Ready { service, .. } => Some(service),
                _ => None,
            }
        };

        if let Some(service) = service {
            drop(service);
            if let Ok(mut state) = self.state.lock() {
                if matches!(*state, ResourceState::Stopping) {
                    *state = ResourceState::Empty;
                    self.changed.notify_all();
                }
            }
        }
    }
}

pub(super) struct ServiceLease {
    resource: Arc<ServiceResource>,
    inject_env: String,
    value: String,
}

pub(super) struct ServiceHandle<'a> {
    project: &'a Project,
    id: String,
    config: ServiceConfig,
    resource: Arc<ServiceResource>,
}

impl ServiceHandle<'_> {
    pub(super) fn acquire(self) -> Result<ServiceLease> {
        self.resource.acquire(self.project, &self.id, self.config)
    }
}

impl ServiceLease {
    pub(super) fn environment(&self) -> (String, String) {
        (self.inject_env.clone(), self.value.clone())
    }
}

impl Drop for ServiceLease {
    fn drop(&mut self) {
        self.resource.release();
    }
}

struct RunningService {
    runtime: crate::config::ContainerRuntimeKind,
    inject_env: String,
    value: String,
    container: Option<String>,
    project_root: PathBuf,
}

impl Drop for RunningService {
    fn drop(&mut self) {
        if let Some(container) = &self.container {
            let _ =
                self.runtime
                    .stop_container(&self.project_root, container, Duration::from_secs(5));
        }
    }
}

impl RunningService {
    fn start(project: &Project, id: &str, config: ServiceConfig) -> Result<Self> {
        match config {
            ServiceConfig::Environment {
                source_env,
                inject_env,
            } => {
                let value = std::env::var(&source_env)
                    .with_context(|| format!("service {id:?} requires {source_env}"))?;
                if value.trim().is_empty() {
                    bail!("service {id:?} requires non-empty {source_env}");
                }
                Ok(Self {
                    runtime: crate::config::ContainerRuntimeKind::Docker,
                    inject_env,
                    value,
                    container: None,
                    project_root: project.root.clone(),
                })
            }
            ServiceConfig::Docker {
                runtime,
                image,
                external_env,
                inject_env,
                external_value_policy,
                startup_timeout_secs,
                container_port,
                environment,
                healthcheck,
                connection,
                ..
            } => {
                if let Some((_, value)) = external_env
                    .as_ref()
                    .and_then(|name| std::env::var(name).ok().map(|value| (name, value)))
                    .filter(|(_, value)| !value.trim().is_empty())
                {
                    validate_external_value(external_value_policy, &value)?;
                    return Ok(Self {
                        runtime,
                        inject_env,
                        value,
                        container: None,
                        project_root: project.root.clone(),
                    });
                }
                let deadline = Instant::now() + Duration::from_secs(startup_timeout_secs);
                runtime
                    .check_available(project, remaining(deadline)?)
                    .with_context(|| format!("container runtime unavailable for service {id:?}"))?;
                start_docker(
                    project,
                    id,
                    DockerStartOptions {
                        image,
                        runtime,
                        inject_env,
                        startup_timeout_secs,
                        container_port,
                        environment,
                        healthcheck,
                        connection,
                        deadline,
                    },
                )
            }
        }
    }
}
pub fn check_available(project: &Project, id: &str, timeout: Duration) -> Result<String> {
    let service = project
        .config
        .service(id)
        .ok_or_else(|| anyhow::anyhow!("unknown service {id:?}"))?;
    match service {
        ServiceConfig::Environment { source_env, .. } => {
            let value = std::env::var(source_env)
                .with_context(|| format!("{source_env} is not configured"))?;
            if value.trim().is_empty() {
                bail!("{source_env} is empty");
            }
            Ok(format!("{source_env} is configured"))
        }
        ServiceConfig::Docker {
            runtime,
            image,
            external_env,
            external_value_policy,
            ..
        } => {
            if let Some(name) = external_env {
                if let Ok(value) = std::env::var(name) {
                    if !value.trim().is_empty() {
                        validate_external_value(*external_value_policy, &value)?;
                        return Ok(format!("{name} is configured"));
                    }
                }
            }
            let deadline = Instant::now() + timeout;
            runtime
                .check_available(project, remaining(deadline)?)
                .with_context(|| format!("container runtime unavailable for service {id:?}"))?;
            let args = vec!["image".into(), "inspect".into(), image.clone()];
            let image_ready = crate::process::capture(
                runtime.executable(),
                &args,
                &project.root,
                remaining(deadline)?,
            )?
            .status
            .success();
            if !image_ready {
                bail!(
                    "{} image {image} is not available; run `{} pull {image}`",
                    runtime.executable(),
                    runtime.executable()
                );
            }
            Ok(format!("{} and {image} ready", runtime.executable()))
        }
    }
}

fn remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or_else(|| anyhow::anyhow!("service operation timed out"))
}
