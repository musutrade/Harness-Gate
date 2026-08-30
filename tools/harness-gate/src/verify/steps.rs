use super::parser::parse_result_count;
use crate::config::{ParserConfig, StepConfig};
use crate::process::{RunnerExecution, Task, TaskResult};
use crate::project::Project;
use crate::service::{ServiceLease, ServiceManager};
use crate::ui;
use anyhow::{bail, Context, Result};
use std::fs;
use std::sync::{Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant};

const SERVICE_LOCK_POLL: Duration = Duration::from_millis(25);
const SERVICE_LOCK_WAIT: Duration = Duration::from_secs(30);

pub(super) fn run_configured_step<'a>(
    project: &'a Project,
    step: &StepConfig,
    services: &'a Mutex<ServiceManager<'a>>,
) -> Result<TaskResult> {
    let mut service_leases = Vec::<ServiceLease>::new();
    for service in &step.services {
        let lease = match lock_services(services) {
            Ok(mut manager) => manager.handle(service),
            Err(error) => {
                let cancelled = crate::process::cancelled();
                return Ok(TaskResult {
                    label: format!("{}: service {service} setup", step.label),
                    passed: false,
                    timed_out: false,
                    cancelled,
                    duration_ms: 0,
                    log: String::new(),
                    detail: Some(format!("{error:#}")),
                    runner: None,
                });
            }
        };
        let lease = match lease.and_then(|handle| handle.acquire()) {
            Ok(lease) => lease,
            Err(error) => {
                let cancelled = crate::process::cancelled();
                return Ok(TaskResult {
                    label: format!("{}: service {service} setup", step.label),
                    passed: false,
                    timed_out: false,
                    cancelled,
                    duration_ms: 0,
                    log: String::new(),
                    detail: Some(format!("{error:#}")),
                    runner: None,
                });
            }
        };
        service_leases.push(lease);
    }
    let parser = step
        .parser
        .as_deref()
        .and_then(|id| project.config.parser(id));
    let task = configured_task(project, step, &service_leases)?;
    execute(task, parser, service_leases)
}

fn lock_services<'a>(
    services: &'a Mutex<ServiceManager<'a>>,
) -> Result<MutexGuard<'a, ServiceManager<'a>>> {
    let started = Instant::now();
    loop {
        if crate::process::cancelled() {
            return Err(anyhow::anyhow!(
                "verification cancelled while waiting for service lock"
            ));
        }
        match services.try_lock() {
            Ok(guard) => {
                if crate::process::cancelled() {
                    drop(guard);
                    return Err(anyhow::anyhow!(
                        "verification cancelled while waiting for service lock"
                    ));
                }
                return Ok(guard);
            }
            Err(TryLockError::Poisoned(_)) => {
                return Err(anyhow::anyhow!("service manager lock was poisoned"));
            }
            Err(TryLockError::WouldBlock) => {
                if crate::process::cancelled() {
                    return Err(anyhow::anyhow!(
                        "verification cancelled while waiting for service lock"
                    ));
                }
                if started.elapsed() >= SERVICE_LOCK_WAIT {
                    return Err(anyhow::anyhow!(
                        "timed out waiting for service manager lock"
                    ));
                }
                std::thread::sleep(SERVICE_LOCK_POLL);
            }
        }
    }
}

fn configured_task(
    project: &Project,
    step: &StepConfig,
    service_leases: &[ServiceLease],
) -> Result<Task> {
    let cwd = std::path::PathBuf::from(project.expand(&step.cwd));
    let args = step
        .args
        .iter()
        .map(|argument| project.expand(argument))
        .collect::<Vec<_>>();
    let mut task = Task::new(&step.label, &step.program, &cwd, log(project, &step.log)?)
        .timeout(step.timeout_secs);
    if let Some(runner) = &step.runner {
        let (effective_args, execution) = runner_inputs(runner, &args)?;
        let runner_environment = execution.environment.clone();
        task = task.args(effective_args).runner(execution);
        for (name, value) in runner_environment {
            task = task.env(name, value);
        }
    } else {
        task = task.args(args);
    }
    for lease in service_leases {
        let (name, value) = lease.environment();
        task = task.env(name, value);
    }
    for name in &step.remove_env {
        task = task.env_remove(name);
    }
    Ok(task)
}

fn runner_inputs(
    runner: &crate::config::RunnerConfig,
    step_args: &[String],
) -> Result<(Vec<String>, RunnerExecution)> {
    let mut effective_args = step_args.to_vec();
    let insertion = runner.args_position.unwrap_or(effective_args.len());
    effective_args.splice(insertion..insertion, runner.args.iter().cloned());

    if runner.kind == "cargo-test" {
        if let Some(threads) = runner.threads {
            let insertion = effective_args
                .iter()
                .position(|argument| argument == "--")
                .map(|index| index + 1)
                .unwrap_or_else(|| {
                    effective_args.push("--".into());
                    effective_args.len()
                });
            effective_args.splice(
                insertion..insertion,
                ["--test-threads".into(), threads.to_string()],
            );
        }
    }

    let mut environment = std::collections::BTreeMap::new();
    if let (Some(name), Some(threads)) = (&runner.threads_env, runner.threads) {
        let value = threads.to_string();
        environment.insert(name.clone(), value);
    }
    let execution = RunnerExecution {
        version: runner.version,
        kind: runner.kind.clone(),
        effective_args: effective_args.clone(),
        environment,
        result_format: runner.result_format,
        isolation: runner.isolation,
        threads: runner.threads,
    };
    Ok((effective_args, execution))
}

fn execute(
    task: Task,
    parser: Option<&ParserConfig>,
    _service_leases: Vec<ServiceLease>,
) -> Result<TaskResult> {
    let mut result = task.run()?;
    if result.passed {
        if let Some(parser) = parser {
            let content = fs::read_to_string(&result.log)
                .with_context(|| format!("read parser log {}", result.log))?;
            let (count, minimum) = parse_result_count(&content, parser)?;
            if count < minimum {
                result.passed = false;
                result.detail = Some(format!(
                    "parsed {count} result(s), expected at least {minimum}"
                ));
            } else {
                result.detail = Some(format!("{count} result(s)"));
            }
        }
    }
    Ok(result)
}

pub(super) fn print_result(result: &TaskResult) {
    let marker = if result.passed {
        ui::pass("PASS")
    } else {
        ui::failure("FAIL")
    };
    println!("[{marker}] {} ({} ms)", result.label, result.duration_ms);
    if !result.passed && !result.log.is_empty() {
        println!("       log: {}", result.log);
    }
}

pub(super) fn print_external_result(result: &TaskResult) {
    print!("[RUN ] {} ... ", result.label);
    use std::io::Write;
    std::io::stdout().flush().ok();
    print_result_inline(result);
}

fn print_result_inline(result: &TaskResult) {
    let marker = if result.passed {
        ui::pass("PASS")
    } else {
        ui::failure("FAIL")
    };
    println!("{marker} ({} ms)", result.duration_ms);
    if !result.passed && !result.log.is_empty() {
        println!("       log: {}", result.log);
    }
}

fn log(project: &Project, name: &str) -> Result<std::path::PathBuf> {
    if !safe_log_name(name) {
        bail!("step log must be a single .log filename: {name:?}");
    }

    let repository = project
        .root
        .canonicalize()
        .with_context(|| format!("resolve project root {}", project.root.display()))?;
    let reports = project
        .reports
        .canonicalize()
        .with_context(|| format!("resolve report directory {}", project.reports.display()))?;
    if !reports.starts_with(&repository) {
        bail!("report directory escapes project root");
    }

    let logs = project.reports.join("logs");
    if std::fs::symlink_metadata(&logs).is_ok() {
        let resolved = logs
            .canonicalize()
            .with_context(|| format!("resolve log directory {}", logs.display()))?;
        if !resolved.starts_with(&reports) {
            bail!("log directory escapes report directory");
        }
    } else {
        std::fs::create_dir_all(&logs)
            .with_context(|| format!("create log directory {}", logs.display()))?;
        let resolved = logs
            .canonicalize()
            .with_context(|| format!("resolve log directory {}", logs.display()))?;
        if !resolved.starts_with(&reports) {
            bail!("log directory escapes report directory");
        }
    }

    let target = logs.join(name);
    if std::fs::symlink_metadata(&target).is_ok() {
        let resolved = target
            .canonicalize()
            .with_context(|| format!("resolve log file {}", target.display()))?;
        if !resolved.starts_with(&reports) {
            bail!("log file escapes report directory");
        }
    }
    Ok(target)
}

fn safe_log_name(value: &str) -> bool {
    let path = std::path::Path::new(value);
    !value.is_empty()
        && !value.contains('\0')
        && !value.contains('/')
        && !value.contains('\\')
        && !value.contains(':')
        && !value.starts_with("//")
        && !value.starts_with("\\\\")
        && !value.starts_with('\\')
        && !value
            .as_bytes()
            .get(1)
            .is_some_and(|character| *character == b':')
        && !path.is_absolute()
        && path.components().count() == 1
        && value.len() > ".log".len()
        && value.ends_with(".log")
}

#[cfg(test)]
mod tests {
    use super::runner_inputs;
    use crate::config::{RunnerConfig, RunnerResultFormat, TestIsolation};

    #[test]
    fn runner_arguments_and_environment_are_recorded() {
        let runner = RunnerConfig {
            version: 1,
            kind: "generic".into(),
            threads: Some(3),
            threads_env: Some("TEST_THREADS".into()),
            args: vec!["--runner-flag".into()],
            args_position: Some(1),
            result_format: RunnerResultFormat::Json,
            isolation: TestIsolation::DatabasePerWorker,
        };
        let (args, execution) =
            runner_inputs(&runner, &["check".into(), "target".into()]).expect("runner inputs");

        assert_eq!(args, ["check", "--runner-flag", "target"]);
        assert_eq!(execution.effective_args, args);
        assert_eq!(execution.environment["TEST_THREADS"], "3");
        assert_eq!(execution.threads, Some(3));
    }

    #[test]
    fn cargo_test_threads_are_inserted_after_the_test_separator() {
        let runner = RunnerConfig {
            version: 1,
            kind: "cargo-test".into(),
            threads: Some(4),
            threads_env: None,
            args: vec!["--nocapture".into()],
            args_position: None,
            result_format: RunnerResultFormat::Junit,
            isolation: TestIsolation::SchemaPerWorker,
        };
        let (args, _) =
            runner_inputs(&runner, &["test".into(), "--".into()]).expect("runner inputs");

        assert_eq!(args, ["test", "--", "--test-threads", "4", "--nocapture"]);
    }
}
