use super::parser::parse_result_count;
use crate::config::{ParserConfig, StepConfig};
use crate::process::{Task, TaskResult};
use crate::project::Project;
use crate::service::ServiceManager;
use crate::ui;
use anyhow::Result;
use std::fs;
use std::sync::Mutex;

pub(super) fn run_configured_step(
    project: &Project,
    step: &StepConfig,
    services: &Mutex<ServiceManager<'_>>,
) -> Result<TaskResult> {
    let mut service_env = Vec::new();
    for service in &step.services {
        let environment = match services
            .lock()
            .map_err(|_| anyhow::anyhow!("service manager lock was poisoned"))?
            .environment(service)
        {
            Ok(environment) => environment,
            Err(error) => {
                return Ok(TaskResult {
                    label: format!("{}: service {service} setup", step.label),
                    passed: false,
                    timed_out: false,
                    cancelled: false,
                    duration_ms: 0,
                    log: String::new(),
                    detail: Some(format!("{error:#}")),
                });
            }
        };
        service_env.push(environment);
    }
    let parser = step
        .parser
        .as_deref()
        .and_then(|id| project.config.parser(id));
    execute(configured_task(project, step, service_env), parser)
}

fn configured_task(
    project: &Project,
    step: &StepConfig,
    service_env: Vec<(String, String)>,
) -> Task {
    let cwd = std::path::PathBuf::from(project.expand(&step.cwd));
    let args = step
        .args
        .iter()
        .map(|argument| project.expand(argument))
        .collect::<Vec<_>>();
    let mut task = Task::new(&step.label, &step.program, &cwd, log(project, &step.log))
        .args(args)
        .timeout(step.timeout_secs);
    for (name, value) in service_env {
        task = task.env(name, value);
    }
    for name in &step.remove_env {
        task = task.env_remove(name);
    }
    task
}

fn execute(task: Task, parser: Option<&ParserConfig>) -> Result<TaskResult> {
    let mut result = task.run()?;
    if result.passed {
        if let Some(parser) = parser {
            let content = fs::read_to_string(&result.log).unwrap_or_default();
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

fn log(project: &Project, name: &str) -> std::path::PathBuf {
    project.reports.join("logs").join(name)
}
