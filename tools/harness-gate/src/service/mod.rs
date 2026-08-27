mod docker;
mod postgres;

#[cfg(test)]
mod tests;

use crate::config::ServiceConfig;
use crate::project::Project;
use anyhow::{bail, Context, Result};
use docker::{ensure_docker, start_docker, DockerStartOptions};
use postgres::validate_external_value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct ServiceManager<'a> {
    project: &'a Project,
    running: BTreeMap<String, RunningService>,
    failures: BTreeMap<String, String>,
}

impl<'a> ServiceManager<'a> {
    pub fn new(project: &'a Project) -> Self {
        Self {
            project,
            running: BTreeMap::new(),
            failures: BTreeMap::new(),
        }
    }

    pub fn environment(&mut self, id: &str) -> Result<(String, String)> {
        if let Some(error) = self.failures.get(id) {
            bail!("service {id:?} previously failed: {error}");
        }
        if !self.running.contains_key(id) {
            let config = self
                .project
                .config
                .service(id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("unknown service {id:?}"))?;
            let running = match RunningService::start(self.project, id, config) {
                Ok(running) => running,
                Err(error) => {
                    let detail = format!("{error:#}");
                    self.failures.insert(id.to_string(), detail.clone());
                    bail!("{detail}");
                }
            };
            self.running.insert(id.to_string(), running);
        }
        let service = self.running.get(id).expect("service inserted");
        Ok((service.inject_env.clone(), service.value.clone()))
    }
}

struct RunningService {
    inject_env: String,
    value: String,
    container: Option<String>,
    project_root: PathBuf,
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
                    inject_env,
                    value,
                    container: None,
                    project_root: project.root.clone(),
                })
            }
            ServiceConfig::Docker {
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
                        inject_env,
                        value,
                        container: None,
                        project_root: project.root.clone(),
                    });
                }
                let deadline = Instant::now() + Duration::from_secs(startup_timeout_secs);
                ensure_docker(project, id, remaining(deadline)?)?;
                start_docker(
                    project,
                    id,
                    DockerStartOptions {
                        image,
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
            ensure_docker(project, id, remaining(deadline)?)?;
            let args = vec!["image".into(), "inspect".into(), image.clone()];
            let image_ready =
                crate::process::capture("docker", &args, &project.root, remaining(deadline)?)?
                    .status
                    .success();
            if !image_ready {
                bail!("Docker image {image} is not available; run `docker pull {image}`");
            }
            Ok(format!("Docker and {image} ready"))
        }
    }
}

fn remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or_else(|| anyhow::anyhow!("service operation timed out"))
}
