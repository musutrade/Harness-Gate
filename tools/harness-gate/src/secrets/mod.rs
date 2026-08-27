mod config;
mod matcher;

#[cfg(test)]
mod tests;

use crate::error::CodedError;
use crate::project::Project;
use crate::utils::{fs as output_fs, git};
use anyhow::{Context, Result};
use config::SecretScanner;
use serde::Serialize;
use std::fs;

/// Errors emitted by the secret-scan boundary.
#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    #[error("secret scan configuration failed: {message}")]
    Configuration { message: String },
    #[error("secret scan failed: {message}")]
    Scan { message: String },
}

impl SecretsError {
    fn configuration(error: anyhow::Error) -> Self {
        Self::Configuration {
            message: format!("{error:#}"),
        }
    }

    fn scan(error: anyhow::Error) -> Self {
        Self::Scan {
            message: format!("{error:#}"),
        }
    }
}

impl CodedError for SecretsError {
    fn code(&self) -> &'static str {
        match self {
            Self::Configuration { .. } => "E1201",
            Self::Scan { .. } => "E1202",
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum SecretMode {
    WorkingTree,
    Staged,
}

#[derive(Debug, Serialize)]
struct SecretReport<'a> {
    timestamp: String,
    mode: &'a str,
    findings: &'a [String],
}

fn scanner_for_mode(project: &Project, mode: SecretMode) -> Result<SecretScanner> {
    match mode {
        SecretMode::WorkingTree => SecretScanner::load(&project.secrets_config),
        SecretMode::Staged => {
            let relative = project
                .secrets_config
                .strip_prefix(&project.root)
                .context("secret scan configuration must stay inside the project")?;
            let relative = relative
                .to_str()
                .context("secret scan configuration path must be UTF-8")?;
            let bytes = git::staged_file(&project.root, relative)?.ok_or_else(|| {
                anyhow::anyhow!("staged secret scan configuration is missing: {relative}")
            })?;
            let source = std::str::from_utf8(&bytes)
                .context("staged secret scan configuration must be UTF-8")?;
            SecretScanner::from_source(source)
                .with_context(|| format!("parse staged secret scan configuration {relative}"))
        }
    }
}

pub fn scan(project: &Project, mode: SecretMode) -> std::result::Result<Vec<String>, SecretsError> {
    let files = match mode {
        SecretMode::WorkingTree => git::null_terminated_paths(
            &project.root,
            [
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
                "-z",
            ],
        )
        .map_err(SecretsError::scan)?,
        SecretMode::Staged => git::null_terminated_paths(
            &project.root,
            [
                "diff",
                "--cached",
                "--diff-filter=ACMR",
                "--name-only",
                "-z",
            ],
        )
        .map_err(SecretsError::scan)?,
    };
    let patterns = scanner_for_mode(project, mode).map_err(SecretsError::configuration)?;
    let mut findings = Vec::new();

    for file in files {
        let bytes = match mode {
            SecretMode::WorkingTree => match fs::read(project.root.join(&file)) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(SecretsError::scan(
                        anyhow::Error::from(error).context(format!("read {file}")),
                    ));
                }
            },
            SecretMode::Staged => {
                match git::staged_file(&project.root, &file).map_err(SecretsError::scan)? {
                    Some(bytes) => bytes,
                    None => continue,
                }
            }
        };
        if patterns.is_match(&bytes) {
            findings.push(file);
        }
    }

    let mode_label = match mode {
        SecretMode::WorkingTree => "working-tree",
        SecretMode::Staged => "staged",
    };
    output_fs::write_json(
        &project.reports.join("secret_scan.json"),
        &SecretReport {
            timestamp: chrono::Utc::now().to_rfc3339(),
            mode: mode_label,
            findings: &findings,
        },
    )
    .map_err(SecretsError::scan)?;
    Ok(findings)
}
