use super::report::DoctorReport;
use crate::config::{DoctorCheck, DoctorCheckKind, PathType};
use crate::project::Project;
use anyhow::{bail, Context, Result};
use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub fn run(project: &Project) -> Result<super::DoctorReport> {
    let mut report = DoctorReport::new(project);
    for check in &project.config.doctor.checks {
        match run_check(project, check) {
            Ok(detail) => report.record_pass(&check.label, detail),
            Err(error) => {
                let detail = match &check.help {
                    Some(help) => format!("{error:#}; {help}"),
                    None => format!("{error:#}"),
                };
                report.record_failure(check.required, &check.label, detail);
            }
        }
    }
    Ok(report)
}

fn run_check(project: &Project, check: &DoctorCheck) -> Result<String> {
    let timeout = Duration::from_secs(check.timeout_secs);
    match &check.kind {
        DoctorCheckKind::Command { program, args } => {
            let args = args
                .iter()
                .map(|arg| project.expand(arg))
                .collect::<Vec<_>>();
            command_output(project, program, &args, timeout)
        }
        DoctorCheckKind::Path { path, path_type } => {
            let path = resolve_check_path(project, path);
            let exists = match path_type {
                PathType::Any => path.exists(),
                PathType::File => path.is_file(),
                PathType::Directory => path.is_dir(),
            };
            if !exists {
                bail!("{} is missing", path.display());
            }
            Ok(path.display().to_string())
        }
        DoctorCheckKind::Glob { pattern } => check_glob(project, pattern),
        DoctorCheckKind::Env { name } => {
            std::env::var_os(name).ok_or_else(|| anyhow::anyhow!("{name} is not configured"))?;
            Ok(format!("{name} is configured"))
        }
        DoctorCheckKind::EnvOrFile {
            env,
            path,
            contains,
        } => {
            if std::env::var_os(env).is_some() {
                return Ok(format!("{env} is configured"));
            }
            let path = resolve_check_path(project, path);
            let found = fs::read_to_string(&path)
                .map(|content| {
                    content
                        .lines()
                        .any(|line| line.trim_start().starts_with(contains))
                })
                .unwrap_or(false);
            if !found {
                bail!(
                    "{env} is absent and {} does not define {contains}",
                    path.display()
                );
            }
            Ok(format!("{contains} found in {}", path.display()))
        }
        DoctorCheckKind::GitConfig { key, expected } => {
            let args = vec!["config".into(), "--get".into(), key.clone()];
            let output = crate::process::capture("git", &args, &project.root, timeout)
                .with_context(|| format!("read Git config {key}"))?;
            if !output.status.success() {
                bail!("Git config {key} is not set");
            }
            let actual = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if actual != *expected {
                bail!("Git config {key} is {actual:?}, expected {expected:?}");
            }
            Ok(format!("{key}={expected}"))
        }
        DoctorCheckKind::GitRemotes => check_remotes(&project.root, timeout),
        DoctorCheckKind::Version {
            program,
            args,
            path,
            trim_prefix,
        } => {
            let args = args
                .iter()
                .map(|arg| project.expand(arg))
                .collect::<Vec<_>>();
            let actual = command_output(project, program, &args, timeout)?
                .trim_start_matches(trim_prefix)
                .to_string();
            let path = resolve_check_path(project, path);
            let expected = fs::read_to_string(&path)
                .with_context(|| format!("read version file {}", path.display()))?
                .trim()
                .to_string();
            if actual != expected {
                bail!("found {actual}, expected {expected}");
            }
            Ok(expected)
        }
        DoctorCheckKind::Service { service } => {
            crate::service::check_available(project, service, timeout)
        }
    }
}

fn resolve_check_path(project: &Project, value: &str) -> PathBuf {
    let expanded = project.expand(value);
    let path = Path::new(&expanded);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project.root.join(path)
    }
}

fn command_output(
    project: &Project,
    program: &str,
    args: &[String],
    timeout: Duration,
) -> Result<String> {
    let output = crate::process::capture(program, args, &project.root, timeout)
        .with_context(|| format!("command {program} is not available or did not finish"))?;
    if !output.status.success() {
        bail!(
            "command {program} exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::to_string)
        .filter(|line| !line.is_empty())
        .ok_or_else(|| anyhow::anyhow!("command {program} produced no output"))
}

fn check_glob(project: &Project, pattern: &str) -> Result<String> {
    let pattern = project.expand(pattern);
    let mut builder = GlobSetBuilder::new();
    builder.add(Glob::new(&pattern)?);
    let matcher = builder.build()?;
    let found = WalkBuilder::new(&project.root)
        .hidden(false)
        .build()
        .filter_map(Result::ok)
        .any(|entry| {
            entry.file_type().is_some_and(|kind| kind.is_file()) && matcher.is_match(entry.path())
        });
    if !found {
        bail!("no files match {pattern}");
    }
    Ok(format!("matched {pattern}"))
}

pub(crate) fn check_remotes(project_root: &Path, timeout: Duration) -> Result<String> {
    let deadline = Instant::now() + timeout;
    let remotes = crate::process::capture(
        "git",
        &["remote".to_string()],
        project_root,
        remaining(deadline)?,
    )
    .context("list Git remotes")?;
    if !remotes.status.success() {
        bail!(
            "project root is not a Git worktree: {}",
            String::from_utf8_lossy(&remotes.stderr).trim()
        );
    }
    for remote in String::from_utf8_lossy(&remotes.stdout).lines() {
        let args = vec![
            "remote".into(),
            "get-url".into(),
            "--all".into(),
            remote.into(),
        ];
        let urls = crate::process::capture("git", &args, project_root, remaining(deadline)?)?;
        if !urls.status.success() {
            bail!("cannot read URLs for Git remote {remote:?}");
        }
        let unsafe_url = String::from_utf8_lossy(&urls.stdout).lines().any(|url| {
            (url.starts_with("https://") || url.starts_with("http://"))
                && url.split_once("//").is_some_and(|(_, tail)| {
                    tail.split_once('@')
                        .is_some_and(|(credentials, _)| !credentials.is_empty())
                })
        });
        if unsafe_url {
            bail!("remote {remote:?} contains embedded HTTPS credentials");
        }
    }
    Ok("no embedded credentials".into())
}

fn remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or_else(|| anyhow::anyhow!("doctor check timed out"))
}
