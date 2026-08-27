use super::{remaining, RunningService};
use crate::project::Project;
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(super) struct DockerStartOptions {
    pub(super) image: String,
    pub(super) inject_env: String,
    pub(super) startup_timeout_secs: u64,
    pub(super) container_port: u16,
    pub(super) environment: BTreeMap<String, String>,
    pub(super) healthcheck: Vec<String>,
    pub(super) connection: String,
    pub(super) deadline: Instant,
}
pub(super) fn start_docker(
    project: &Project,
    id: &str,
    options: DockerStartOptions,
) -> Result<RunningService> {
    let DockerStartOptions {
        image,
        inject_env,
        startup_timeout_secs,
        container_port,
        environment,
        healthcheck,
        connection,
        deadline,
    } = options;
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let name = format!(
        "arc-flow-{}-{id}-{}-{unique}",
        project.config.project.name,
        std::process::id()
    );
    let publish = format!("127.0.0.1::{container_port}");
    let mut args = vec![
        "run".to_string(),
        "--rm".into(),
        "--detach".into(),
        "--pull=never".into(),
        "--name".into(),
        name.clone(),
    ];
    for (key, value) in environment {
        args.extend(["--env".into(), format!("{key}={value}")]);
    }
    args.extend(["--publish".into(), publish, image.clone()]);
    let output = crate::process::capture("docker", &args, &project.root, remaining(deadline)?)
        .with_context(|| format!("start Docker service {id:?}"))?;
    if !output.status.success() {
        bail!(
            "failed to start Docker service {id:?} with image {image}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let mut running = RunningService {
        inject_env,
        value: String::new(),
        container: Some(name),
        project_root: project.root.clone(),
    };
    while Instant::now() < deadline {
        if crate::process::cancelled() {
            bail!("verification cancelled while waiting for service {id:?}");
        }
        if let Some(port) = running.port(container_port, remaining(deadline)?)? {
            let mut health_args = vec![
                "exec".to_string(),
                running.container.as_deref().unwrap_or_default().to_string(),
            ];
            health_args.extend(healthcheck.iter().cloned());
            let ready = crate::process::capture(
                "docker",
                &health_args,
                &project.root,
                remaining(deadline)?,
            )?
            .status
            .success();
            if ready {
                running.value = connection.replace("{host_port}", &port);
                return Ok(running);
            }
        }
        thread::sleep(Duration::from_secs(1));
    }
    bail!("service {id:?} did not become ready within {startup_timeout_secs} seconds")
}

pub(super) fn ensure_docker(project: &Project, id: &str, timeout: Duration) -> Result<()> {
    let info = crate::process::capture("docker", &["info".to_string()], &project.root, timeout)
        .with_context(|| format!("Docker is required by service {id:?}"))?;
    if !info.status.success() {
        bail!("Docker daemon is unavailable for service {id:?}");
    }
    Ok(())
}

impl RunningService {
    fn port(&self, container_port: u16, timeout: Duration) -> Result<Option<String>> {
        let Some(container) = &self.container else {
            return Ok(None);
        };
        let args = vec![
            "port".into(),
            container.clone(),
            format!("{container_port}/tcp"),
        ];
        let output = crate::process::capture("docker", &args, &self.project_root, timeout)?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .and_then(|line| line.rsplit(':').next())
            .map(str::to_string))
    }
}

impl Drop for RunningService {
    fn drop(&mut self) {
        if let Some(container) = &self.container {
            let args = vec!["rm".into(), "--force".into(), container.clone()];
            let _ = crate::process::capture_cleanup(
                "docker",
                &args,
                &self.project_root,
                Duration::from_secs(5),
            );
        }
    }
}
