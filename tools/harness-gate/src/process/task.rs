use super::command::{isolate_process_tree, terminate};
use super::isolation;
use super::signal::cancelled;
use crate::config::{RunnerResultFormat, TestIsolation};
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct Task {
    pub label: String,
    program: OsString,
    args: Vec<OsString>,
    cwd: PathBuf,
    env: Vec<(OsString, OsString)>,
    env_remove: Vec<OsString>,
    timeout: Duration,
    log: PathBuf,
    runner: Option<RunnerExecution>,
    isolation_state: Option<PathBuf>,
}

/// Records the declared runner contract and the effective inputs used for a task.
/// Environment values are limited to runner-owned declarations; service values
/// are intentionally not copied into this report field.
#[derive(Debug, Clone, Serialize)]
pub struct RunnerExecution {
    pub version: u32,
    pub kind: String,
    pub program: String,
    pub effective_args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub environment: BTreeMap<String, String>,
    pub result_format: RunnerResultFormat,
    pub isolation: TestIsolation,
    pub threads: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isolation_id: Option<String>,
    #[serde(default)]
    pub worker_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isolation_root: Option<String>,
    pub migration_decision: String,
    pub lock_decision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shard_index: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shard_total: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_identity: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invocation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    pub label: String,
    pub passed: bool,
    pub timed_out: bool,
    pub cancelled: bool,
    pub duration_ms: u128,
    pub log: String,
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_code: Option<String>,
    #[serde(default)]
    pub attempts: Vec<TaskAttempt>,
    #[serde(default)]
    pub flaky: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parser: Option<ParserEvidence>,
    #[serde(default)]
    pub waived: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiver: Option<WaiverEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runner: Option<RunnerExecution>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParserEvidence {
    pub mode: String,
    pub version: u32,
    pub observed: usize,
    pub minimum: usize,
    pub complete: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskAttempt {
    pub attempt: u32,
    pub status: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub duration_ms: u128,
    pub timed_out: bool,
    pub cancelled: bool,
    pub log: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WaiverEvidence {
    pub id: String,
    pub risk: String,
    pub owner: String,
    pub approved_by: String,
    pub created_at: String,
    pub expires_at: String,
    pub compensating_control: String,
}

impl Task {
    pub fn new(
        label: impl Into<String>,
        program: impl AsRef<OsStr>,
        cwd: &Path,
        log: PathBuf,
    ) -> Self {
        Self {
            label: label.into(),
            program: program.as_ref().to_os_string(),
            args: Vec::new(),
            cwd: cwd.to_path_buf(),
            env: Vec::new(),
            env_remove: Vec::new(),
            timeout: Duration::from_secs(180),
            log,
            runner: None,
            isolation_state: None,
        }
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args
            .extend(args.into_iter().map(|arg| arg.as_ref().to_os_string()));
        self
    }

    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.env
            .push((key.as_ref().to_os_string(), value.as_ref().to_os_string()));
        self
    }

    pub fn env_remove(mut self, key: impl AsRef<OsStr>) -> Self {
        self.env_remove.push(key.as_ref().to_os_string());
        self
    }

    pub fn timeout(mut self, seconds: u64) -> Self {
        self.timeout = Duration::from_secs(seconds);
        self
    }

    pub fn runner(mut self, execution: RunnerExecution) -> Self {
        self.runner = Some(execution);
        self
    }

    pub fn isolation_state(mut self, state_file: PathBuf) -> Self {
        self.isolation_state = Some(state_file);
        self
    }

    pub fn run(self) -> Result<TaskResult> {
        let _state_guard = self
            .isolation_state
            .as_deref()
            .map(IsolationStateGuard::new);
        let log_file = crate::utils::fs::create_atomic_output(&self.log, true)
            .with_context(|| format!("create log {}", self.log.display()))?;
        let stdout = log_file.try_clone()?;
        let stderr = log_file.try_clone()?;
        let started = Instant::now();
        let started_at = chrono::Utc::now().to_rfc3339();
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .current_dir(&self.cwd)
            .envs(self.env);
        for name in self.env_remove {
            command.env_remove(name);
        }
        command
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        isolate_process_tree(&mut command);
        let mut child = command
            .spawn()
            .with_context(|| format!("start {}", self.program.to_string_lossy()))?;

        let (status, timed_out, was_cancelled) = loop {
            if let Some(status) = child.try_wait()? {
                break (status, false, false);
            }
            if cancelled() {
                break (terminate(&mut child)?, false, true);
            }
            if started.elapsed() >= self.timeout {
                break (terminate(&mut child)?, true, false);
            }
            std::thread::sleep(Duration::from_millis(100));
        };

        log_file
            .publish()
            .with_context(|| format!("publish log {}", self.log.display()))?;

        Ok(TaskResult {
            step_id: None,
            invocation_id: None,
            attempt: None,
            started_at: Some(started_at),
            finished_at: Some(chrono::Utc::now().to_rfc3339()),
            label: self.label,
            passed: status.success() && !timed_out && !was_cancelled,
            timed_out,
            cancelled: was_cancelled,
            duration_ms: started.elapsed().as_millis(),
            log: self.log.to_string_lossy().to_string(),
            detail: if was_cancelled {
                Some("cancelled".to_string())
            } else if timed_out {
                Some("timed out".to_string())
            } else if status.success() {
                None
            } else {
                status.code().map(|code| format!("exit code {code}"))
            },
            failure_code: None,
            attempts: Vec::new(),
            flaky: false,
            retry_class: None,
            parser: None,
            waived: false,
            waiver: None,
            runner: self.runner,
        })
    }
}

struct IsolationStateGuard<'a> {
    path: &'a Path,
}

impl<'a> IsolationStateGuard<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }
}

impl Drop for IsolationStateGuard<'_> {
    fn drop(&mut self) {
        let _ = isolation::mark_terminal(self.path, "worker exited");
        let _ = isolation::remove(self.path);
    }
}
