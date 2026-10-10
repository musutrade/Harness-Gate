use super::command::{isolate_process_tree, terminate};
use super::isolation;
use super::reader::{
    collect_limited_reader, spawn_limited_reader, LimitedOutput, ReaderThread,
    DEFAULT_CAPTURE_BYTES, DEFAULT_READER_DEADLINE,
};
use super::signal::cancelled;
use crate::config::{RunnerResultFormat, TestIsolation};
use crate::failure::{FailureCode, RetryClass};
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
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
    pub failure_code: Option<FailureCode>,
    #[serde(default)]
    pub attempts: Vec<TaskAttempt>,
    #[serde(default)]
    pub flaky: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_class: Option<RetryClass>,
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
        let mut log_file = crate::utils::fs::create_atomic_output(&self.log, true)
            .with_context(|| format!("create log {}", self.log.display()))?;
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
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        isolate_process_tree(&mut command);
        let mut child = command
            .spawn()
            .with_context(|| format!("start {}", self.program.to_string_lossy()))?;

        let stdout = child
            .stdout
            .take()
            .context("task stdout was not captured")?;
        let stderr = child
            .stderr
            .take()
            .context("task stderr was not captured")?;
        let stdout_overflow = Arc::new(AtomicBool::new(false));
        let stderr_overflow = Arc::new(AtomicBool::new(false));
        let (stdout_handle, stdout_receiver) =
            spawn_limited_reader(stdout, DEFAULT_CAPTURE_BYTES, Arc::clone(&stdout_overflow));
        let (stderr_handle, stderr_receiver) =
            spawn_limited_reader(stderr, DEFAULT_CAPTURE_BYTES, Arc::clone(&stderr_overflow));

        let (status, timed_out, was_cancelled, output_limited) = wait_for_task(
            &mut child,
            started,
            self.timeout,
            &stdout_overflow,
            &stderr_overflow,
        )?;

        let reader_started = Instant::now();
        let (stdout, stdout_error) = collect_task_stream(
            stdout_handle,
            stdout_receiver,
            DEFAULT_READER_DEADLINE,
            "task stdout",
        );
        let remaining_reader_deadline =
            DEFAULT_READER_DEADLINE.saturating_sub(reader_started.elapsed());
        let (stderr, stderr_error) = collect_task_stream(
            stderr_handle,
            stderr_receiver,
            remaining_reader_deadline,
            "task stderr",
        );
        let reader_error = stdout_error.or(stderr_error);

        let (failure_code, limit_detail) =
            output_failure(output_limited, &stdout, &stderr, reader_error);

        log_file.write_all(&stdout.bytes)?;
        if !stdout.bytes.is_empty() && !stderr.bytes.is_empty() {
            log_file.write_all(b"\n")?;
        }
        log_file.write_all(&stderr.bytes)?;
        if let Some(detail) = &limit_detail {
            log_file
                .write_all(format!("\n[HARNESS_GATE_EVIDENCE_FAILURE] {detail}\n").as_bytes())?;
        }
        log_file
            .publish()
            .with_context(|| format!("publish log {}", self.log.display()))?;

        let detail = task_detail(limit_detail, was_cancelled, timed_out, &status);

        Ok(TaskResult {
            step_id: None,
            invocation_id: None,
            attempt: None,
            started_at: Some(started_at),
            finished_at: Some(chrono::Utc::now().to_rfc3339()),
            label: self.label,
            passed: status.success() && !timed_out && !was_cancelled && failure_code.is_none(),
            timed_out,
            cancelled: was_cancelled,
            duration_ms: started.elapsed().as_millis(),
            log: self.log.to_string_lossy().to_string(),
            detail,
            failure_code,
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

fn observed_overflow(
    stdout_overflow: &AtomicBool,
    stderr_overflow: &AtomicBool,
) -> Option<&'static str> {
    if stdout_overflow.load(Ordering::Acquire) {
        Some("stdout")
    } else if stderr_overflow.load(Ordering::Acquire) {
        Some("stderr")
    } else {
        None
    }
}

type TaskOutcome = (ExitStatus, bool, bool, Option<&'static str>);

fn poll_task(
    child: &mut Child,
    started: Instant,
    timeout: Duration,
    stdout_overflow: &AtomicBool,
    stderr_overflow: &AtomicBool,
) -> Result<Option<TaskOutcome>> {
    if let Some(status) = child.try_wait()? {
        return Ok(Some((
            status,
            false,
            false,
            observed_overflow(stdout_overflow, stderr_overflow),
        )));
    }
    if let Some(stream) = observed_overflow(stdout_overflow, stderr_overflow) {
        return Ok(Some((terminate(child)?, false, false, Some(stream))));
    }
    if cancelled() {
        return Ok(Some((terminate(child)?, false, true, None)));
    }
    if started.elapsed() >= timeout {
        return Ok(Some((terminate(child)?, true, false, None)));
    }
    Ok(None)
}

fn wait_for_task(
    child: &mut Child,
    started: Instant,
    timeout: Duration,
    stdout_overflow: &AtomicBool,
    stderr_overflow: &AtomicBool,
) -> Result<TaskOutcome> {
    // `Child::try_wait` is the portable API available on all supported
    // platforms. Keep the initial delay short for fast commands, then
    // back off to cap wakeups while preserving timeout/cancellation checks.
    let mut wait_round = 0_u32;
    loop {
        if let Some(outcome) = poll_task(child, started, timeout, stdout_overflow, stderr_overflow)?
        {
            return Ok(outcome);
        }
        std::thread::sleep(wait_backoff(wait_round));
        wait_round = wait_round.saturating_add(1);
    }
}

fn collect_task_stream(
    handle: ReaderThread,
    receiver: Receiver<std::io::Result<LimitedOutput>>,
    deadline: Duration,
    stream: &str,
) -> (LimitedOutput, Option<String>) {
    match collect_limited_reader(handle, receiver, deadline, stream) {
        Ok(output) => (output, None),
        Err(error) => (
            LimitedOutput {
                bytes: Vec::new(),
                truncated: false,
            },
            Some(format!("{stream} reader deadline/error: {error}")),
        ),
    }
}

fn output_failure(
    output_limited: Option<&'static str>,
    stdout: &LimitedOutput,
    stderr: &LimitedOutput,
    reader_error: Option<String>,
) -> (Option<FailureCode>, Option<String>) {
    let combined_bytes = stdout.bytes.len().saturating_add(stderr.bytes.len());
    let limited = output_limited
        .or_else(|| stdout.truncated.then_some("stdout"))
        .or_else(|| stderr.truncated.then_some("stderr"))
        .or_else(|| {
            (combined_bytes > DEFAULT_CAPTURE_BYTES.saturating_mul(2)).then_some("combined")
        });
    if let Some(stream) = limited {
        let captured = match stream {
            "stdout" => stdout.bytes.len(),
            "stderr" => stderr.bytes.len(),
            _ => combined_bytes,
        };
        let budget = if stream == "combined" {
            DEFAULT_CAPTURE_BYTES.saturating_mul(2)
        } else {
            DEFAULT_CAPTURE_BYTES
        };
        return (
            Some(FailureCode::OutputLimitExceeded),
            Some(format!(
                "{stream} output exceeded {budget} bytes (captured {captured} bytes; truncated=true)"
            )),
        );
    }
    if let Some(error) = reader_error {
        return (Some(FailureCode::ReaderDeadlineExceeded), Some(error));
    }
    (None, None)
}

fn task_detail(
    limit_detail: Option<String>,
    was_cancelled: bool,
    timed_out: bool,
    status: &ExitStatus,
) -> Option<String> {
    if let Some(detail) = limit_detail {
        return Some(detail);
    }
    if was_cancelled {
        return Some("cancelled".to_string());
    }
    if timed_out {
        return Some("timed out".to_string());
    }
    if status.success() {
        return None;
    }
    status.code().map(|code| format!("exit code {code}"))
}

fn wait_backoff(round: u32) -> Duration {
    let shift = round.min(4);
    Duration::from_millis((5_u64 << shift).min(80))
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

#[cfg(test)]
mod tests {
    use super::wait_backoff;
    use std::time::Duration;

    #[test]
    fn wait_backoff_is_bounded_and_starts_short() {
        assert_eq!(wait_backoff(0), Duration::from_millis(5));
        assert_eq!(wait_backoff(1), Duration::from_millis(10));
        assert_eq!(wait_backoff(4), Duration::from_millis(80));
        assert_eq!(wait_backoff(u32::MAX), Duration::from_millis(80));
    }
}
