use crate::config::ContainerRuntimeKind;
use crate::project::Project;
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

/// Small runtime boundary shared by Docker-compatible container engines.
/// Keeping command execution behind this trait makes service orchestration
/// independent of the selected CLI while preserving the existing adapter.
pub(crate) trait ContainerRuntime {
    fn executable(&self) -> &'static str;

    fn start_container(
        &self,
        project: &Project,
        options: ContainerStartOptions<'_>,
        timeout: Duration,
    ) -> Result<()> {
        let name = options.name;
        let image = options.image;
        let args = start_container_args(
            name,
            image,
            options.environment,
            options.labels,
            options.container_port,
        );
        let output = crate::process::capture(self.executable(), &args, &project.root, timeout)
            .with_context(|| format!("start {} container {name:?}", self.executable()))?;
        if !output.status.success() {
            bail!(
                "failed to start {} container {name:?} with image {image}: {}",
                self.executable(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(())
    }

    fn mapped_port(
        &self,
        project: &Project,
        name: &str,
        container_port: u16,
        timeout: Duration,
    ) -> Result<Option<String>> {
        let args = mapped_port_args(name, container_port);
        let output = crate::process::capture(self.executable(), &args, &project.root, timeout)?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .and_then(|line| line.rsplit(':').next())
            .map(str::to_string))
    }

    fn run_healthcheck(
        &self,
        project: &Project,
        name: &str,
        command: &[String],
        timeout: Duration,
    ) -> Result<bool> {
        let args = healthcheck_args(name, command);
        Ok(
            crate::process::capture(self.executable(), &args, &project.root, timeout)?
                .status
                .success(),
        )
    }

    fn stop_container(&self, cwd: &Path, name: &str, timeout: Duration) -> Result<()> {
        let args = stop_container_args(name);
        let output = crate::process::capture_cleanup(self.executable(), &args, cwd, timeout)?;
        if !output.status.success() {
            bail!(
                "failed to stop {} container {name:?}: {}",
                self.executable(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(())
    }

    fn check_available(&self, project: &Project, timeout: Duration) -> Result<()> {
        let output = crate::process::capture(
            self.executable(),
            &runtime_info_args(),
            &project.root,
            timeout,
        )
        .with_context(|| format!("{} is required for managed services", self.executable()))?;
        if output.status.success() {
            Ok(())
        } else {
            anyhow::bail!("{} daemon is unavailable", self.executable())
        }
    }
}

pub(crate) struct ContainerStartOptions<'a> {
    pub(crate) name: &'a str,
    pub(crate) image: &'a str,
    pub(crate) environment: &'a BTreeMap<String, String>,
    pub(crate) labels: &'a BTreeMap<String, String>,
    pub(crate) container_port: u16,
}

pub(crate) fn stop_owned_container(
    runtime: ContainerRuntimeKind,
    cwd: &Path,
    name: &str,
    timeout: Duration,
) -> Result<()> {
    runtime.stop_container(cwd, name, timeout)
}

impl ContainerRuntime for ContainerRuntimeKind {
    fn executable(&self) -> &'static str {
        match self {
            ContainerRuntimeKind::Docker => "docker",
            ContainerRuntimeKind::Podman => "podman",
        }
    }
}

fn start_container_args(
    name: &str,
    image: &str,
    environment: &BTreeMap<String, String>,
    labels: &BTreeMap<String, String>,
    container_port: u16,
) -> Vec<String> {
    let mut args = vec![
        "run".to_string(),
        "--rm".into(),
        "--detach".into(),
        "--pull=never".into(),
        "--name".into(),
        name.to_string(),
    ];
    for (key, value) in environment {
        args.extend(["--env".into(), format!("{key}={value}")]);
    }
    for (key, value) in labels {
        args.extend(["--label".into(), format!("{key}={value}")]);
    }
    args.extend([
        "--publish".into(),
        format!("127.0.0.1::{container_port}"),
        image.to_string(),
    ]);
    args
}

fn mapped_port_args(name: &str, container_port: u16) -> Vec<String> {
    vec!["port".into(), name.into(), format!("{container_port}/tcp")]
}

fn healthcheck_args(name: &str, command: &[String]) -> Vec<String> {
    let mut args = vec!["exec".into(), name.into()];
    args.extend(command.iter().cloned());
    args
}

fn stop_container_args(name: &str) -> Vec<String> {
    vec!["rm".into(), "--force".into(), name.into()]
}

fn runtime_info_args() -> Vec<String> {
    vec!["info".into()]
}

#[cfg(test)]
mod tests {
    use super::{
        healthcheck_args, mapped_port_args, runtime_info_args, start_container_args,
        stop_container_args,
    };
    use crate::config::ContainerRuntimeKind;
    use std::collections::BTreeMap;

    #[test]
    fn selects_the_expected_docker_compatible_executable() {
        assert_eq!(ContainerRuntimeKind::Docker.executable(), "docker");
        assert_eq!(ContainerRuntimeKind::Podman.executable(), "podman");
    }

    #[test]
    fn builds_deterministic_start_arguments() {
        let environment = BTreeMap::from([
            ("ZED".to_string(), "last".to_string()),
            ("ALPHA".to_string(), "first".to_string()),
        ]);
        assert_eq!(
            start_container_args(
                "fixture",
                "postgres:16",
                &environment,
                &BTreeMap::new(),
                5432
            ),
            vec![
                "run",
                "--rm",
                "--detach",
                "--pull=never",
                "--name",
                "fixture",
                "--env",
                "ALPHA=first",
                "--env",
                "ZED=last",
                "--publish",
                "127.0.0.1::5432",
                "postgres:16",
            ]
        );
    }

    #[test]
    fn builds_port_healthcheck_cleanup_and_info_arguments() {
        assert_eq!(
            mapped_port_args("fixture", 5432),
            vec!["port", "fixture", "5432/tcp"]
        );
        assert_eq!(
            healthcheck_args(
                "fixture",
                &["pg_isready".into(), "-U".into(), "test".into()]
            ),
            vec!["exec", "fixture", "pg_isready", "-U", "test"]
        );
        assert_eq!(
            stop_container_args("fixture"),
            vec!["rm", "--force", "fixture"]
        );
        assert_eq!(runtime_info_args(), vec!["info"]);
    }
}
