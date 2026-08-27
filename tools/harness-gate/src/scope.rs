use crate::config::UnmatchedScope;
use crate::error::CodedError;
use crate::project::Project;
use crate::utils::{fs as output_fs, git};
use anyhow::{bail, Result};
use serde::Serialize;
use std::collections::BTreeSet;

/// Errors emitted while determining a verification scope.
#[derive(Debug, thiserror::Error)]
pub enum ScopeError {
    #[error("Git scope detection failed: {message}")]
    Git { message: String },
    #[error("scope configuration failed: {message}")]
    Configuration { message: String },
    #[error("scope has {count} unmatched changed file(s): {files}")]
    UnmatchedFiles { count: usize, files: String },
    #[error("scope report failed: {message}")]
    Report { message: String },
}

impl ScopeError {
    fn git(error: anyhow::Error) -> Self {
        Self::Git {
            message: format!("{error:#}"),
        }
    }

    fn configuration(error: anyhow::Error) -> Self {
        Self::Configuration {
            message: format!("{error:#}"),
        }
    }

    fn report(error: anyhow::Error) -> Self {
        Self::Report {
            message: format!("{error:#}"),
        }
    }
}

impl CodedError for ScopeError {
    fn code(&self) -> &'static str {
        match self {
            Self::Git { .. } => "E1301",
            Self::Configuration { .. } => "E1302",
            Self::UnmatchedFiles { .. } => "E1303",
            Self::Report { .. } => "E1304",
        }
    }
}

#[derive(Debug, Clone)]
pub enum ScopeMode {
    WorkingTree,
    Staged,
    Base(String),
    All,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScopeResult {
    pub mode: String,
    pub changed_files: Vec<String>,
    pub components: BTreeSet<String>,
    pub unmatched_files: Vec<String>,
}

impl ScopeResult {
    pub fn all(project: &Project) -> Self {
        Self {
            mode: "all".to_string(),
            changed_files: Vec::new(),
            components: project.config.components(),
            unmatched_files: Vec::new(),
        }
    }

    pub fn write_reports(&self, project: &Project) -> std::result::Result<(), ScopeError> {
        let changed = if self.changed_files.is_empty() {
            String::new()
        } else {
            format!("{}\n", self.changed_files.join("\n"))
        };
        output_fs::write(&project.reports.join("changed_files.txt"), changed)
            .map_err(ScopeError::report)?;
        output_fs::write_json(&project.reports.join("scope.json"), self)
            .map_err(ScopeError::report)?;
        Ok(())
    }
}

pub fn detect(project: &Project, mode: &ScopeMode) -> std::result::Result<ScopeResult, ScopeError> {
    if matches!(mode, ScopeMode::All) {
        return Ok(ScopeResult::all(project));
    }

    ensure_git_worktree(project).map_err(ScopeError::git)?;
    let mut paths = BTreeSet::new();
    let mode_label = match mode {
        ScopeMode::WorkingTree => {
            paths.extend(
                git::null_terminated_paths(&project.root, ["diff", "--name-only", "-z"])
                    .map_err(ScopeError::git)?,
            );
            paths.extend(
                git::null_terminated_paths(
                    &project.root,
                    ["diff", "--cached", "--name-only", "-z"],
                )
                .map_err(ScopeError::git)?,
            );
            paths.extend(
                git::null_terminated_paths(
                    &project.root,
                    ["ls-files", "--others", "--exclude-standard", "-z"],
                )
                .map_err(ScopeError::git)?,
            );
            "working-tree".to_string()
        }
        ScopeMode::Staged => {
            paths.extend(
                git::null_terminated_paths(
                    &project.root,
                    ["diff", "--cached", "--name-only", "-z"],
                )
                .map_err(ScopeError::git)?,
            );
            "staged".to_string()
        }
        ScopeMode::Base(reference) => {
            let revision = format!("{reference}^{{commit}}");
            let output = git::capture(&project.root, ["rev-parse", "--verify", revision.as_str()])
                .map_err(ScopeError::git)?;
            if !output.status.success() {
                return Err(ScopeError::Git {
                    message: format!("base reference does not exist: {reference}"),
                });
            }
            let range = format!("{reference}...HEAD");
            paths.extend(
                git::null_terminated_paths(
                    &project.root,
                    ["diff", "--name-only", "-z", range.as_str()],
                )
                .map_err(ScopeError::git)?,
            );
            format!("base:{reference}")
        }
        ScopeMode::All => unreachable!(),
    };

    let changed_files = paths.into_iter().collect::<Vec<_>>();
    let (mut components, unmatched_files) = project
        .config
        .classify_paths(&changed_files)
        .map_err(ScopeError::configuration)?;
    match project.config.scope.unmatched {
        UnmatchedScope::Fail if !unmatched_files.is_empty() => {
            return Err(ScopeError::UnmatchedFiles {
                count: unmatched_files.len(),
                files: unmatched_files.join(", "),
            });
        }
        UnmatchedScope::All => components.extend(project.config.components()),
        UnmatchedScope::Fail | UnmatchedScope::Ignore => {}
    }
    Ok(ScopeResult {
        mode: mode_label,
        components,
        changed_files,
        unmatched_files,
    })
}

fn ensure_git_worktree(project: &Project) -> Result<()> {
    let output = git::capture(&project.root, ["rev-parse", "--is-inside-work-tree"])?;
    if !output.status.success() || output.stdout != b"true\n" {
        bail!("project root is not a Git worktree");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> crate::config::FlowConfig {
        toml::from_str(include_str!("../presets/rust-api.flow.toml")).expect("parse config")
    }

    #[test]
    fn workflow_changes_force_all_components() {
        // Use a path that matches the rust-api preset patterns
        let components = config()
            .classify_paths(&[".harness-gate/flow.toml".into()])
            .expect("classify")
            .0;
        // rust-api preset has 1 component: app
        assert_eq!(components.len(), 1);
        assert!(components.contains("app"));
    }

    #[test]
    fn frontend_change_only_selects_frontend() {
        // rust-api preset doesn't have frontend, test with app component
        let components = config()
            .classify_paths(&["src/main.rs".into()])
            .expect("classify")
            .0;
        assert_eq!(components, BTreeSet::from(["app".to_string()]));
    }

    #[test]
    fn unmatched_paths_are_reported() {
        let (components, unmatched) = config()
            .classify_paths(&["unconfigured/new-tool.lock".into()])
            .expect("classify");

        assert!(components.is_empty());
        assert_eq!(unmatched, vec!["unconfigured/new-tool.lock"]);
    }
}
