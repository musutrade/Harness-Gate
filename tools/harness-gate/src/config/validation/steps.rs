use super::super::model::{ServiceConfig, StepConfig};
use super::primitives::{validate_env_name, validate_id, validate_program};
use super::FlowConfig;
use anyhow::{bail, Result};
use std::collections::HashSet;
use std::path::Path;

pub(super) fn validate_step(config: &FlowConfig, step: &StepConfig) -> Result<()> {
    validate_id("verification step id", &step.id)?;
    if step.label.trim().is_empty() || step.profiles.is_empty() {
        bail!(
            "step {:?} requires a label and at least one profile",
            step.id
        );
    }
    for profile in &step.profiles {
        validate_id("step profile", profile)?;
    }

    match step.kind.as_deref().unwrap_or("external-step") {
        "builtin-gate" => return validate_builtin_gate(step),
        "external-step" => {
            if step.gate_type.is_some() {
                bail!("external step {:?} may not declare gate_type", step.id);
            }
            if matches!(
                step.id.as_str(),
                "builtin.secret-scan" | "builtin.architecture-audit"
            ) {
                bail!(
                    "external step {:?} uses a reserved built-in gate id",
                    step.id
                );
            }
        }
        other => bail!("step {:?} has unknown kind {other:?}", step.id),
    }

    validate_id("step component", &step.component)?;
    validate_program(&format!("step {} program", step.id), &step.program)?;
    if is_shell(&step.program)
        && step
            .args
            .iter()
            .any(|argument| is_shell_command_argument(argument))
    {
        bail!("step {:?} may not execute a shell command string", step.id);
    }
    validate_arguments(config, &step.id, &step.args)?;
    let Some(cwd_name) = exact_placeholder(&step.cwd) else {
        bail!("step {:?} cwd must be one path placeholder", step.id);
    };
    if cwd_name != "root" && !config.paths.aliases.contains_key(cwd_name) {
        bail!(
            "step {:?} cwd references unknown path {cwd_name:?}",
            step.id
        );
    }
    validate_log_name(&step.log)
        .map_err(|error| anyhow::anyhow!("step {:?} log {error}", step.id))?;
    if step.timeout_secs == 0 || step.timeout_secs > 3600 {
        bail!("step {:?} timeout_secs must be between 1 and 3600", step.id);
    }
    if let Some(name) = &step.timeout_env {
        validate_env_name("step timeout_env", name)?;
    }
    if let Some(parser) = &step.parser {
        if !config.parsers.contains_key(parser) {
            bail!("step {:?} references unknown parser {parser:?}", step.id);
        }
    }
    let mut step_services = HashSet::new();
    let mut service_envs = HashSet::new();
    for service in &step.services {
        if !step_services.insert(service) {
            bail!("step {:?} contains duplicate service {service:?}", step.id);
        }
        let service_config = config.services.get(service).ok_or_else(|| {
            anyhow::anyhow!("step {:?} references unknown service {service:?}", step.id)
        })?;
        let inject_env = match service_config {
            ServiceConfig::Docker { inject_env, .. }
            | ServiceConfig::Environment { inject_env, .. } => inject_env,
        };
        if !service_envs.insert(inject_env) {
            bail!(
                "step {:?} has multiple services injecting {inject_env}",
                step.id
            );
        }
        if step.remove_env.contains(inject_env) {
            bail!(
                "step {:?} may not remove service injection variable {inject_env}",
                step.id
            );
        }
    }
    for name in &step.remove_env {
        validate_env_name("step remove_env", name)?;
    }
    Ok(())
}

/// Validate a log as a filename rather than relying only on the host platform's
/// `Path` parser. Configuration can be checked on Unix and later executed on
/// Windows, so both separator styles and Windows prefixes are rejected here.
pub(super) fn validate_log_name(value: &str) -> Result<()> {
    let path = Path::new(value);
    let windows_prefix = value.starts_with("\\\\")
        || value.starts_with("//")
        || value.starts_with('\\')
        || value
            .as_bytes()
            .get(1)
            .is_some_and(|character| *character == b':');
    if value.is_empty()
        || value.contains('\0')
        || value.contains('/')
        || value.contains('\\')
        || value.contains(':')
        || windows_prefix
        || path.is_absolute()
        || value == "."
        || value == ".."
        || value.len() <= ".log".len()
        || !value.ends_with(".log")
    {
        bail!("must be a single .log file name");
    }
    if path.components().count() != 1 {
        bail!("must be a single .log file name");
    }
    Ok(())
}

/// Return the lexical identity used by duplicate-log preflight. Lowercasing
/// makes the check conservative on case-insensitive filesystems while keeping
/// the configured spelling unchanged in reports.
pub(super) fn normalize_log_identity(value: &str) -> String {
    value.to_ascii_lowercase()
}

fn validate_builtin_gate(step: &StepConfig) -> Result<()> {
    let gate_type = step
        .gate_type
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("built-in gate {:?} requires gate_type", step.id))?;
    let expected_id = format!("builtin.{gate_type}");
    if !matches!(gate_type, "secret-scan" | "architecture-audit") {
        bail!(
            "built-in gate {:?} has unknown gate_type {gate_type:?}",
            step.id
        );
    }
    if step.id != expected_id {
        bail!(
            "built-in gate {:?} must use reserved id {expected_id:?}",
            step.id
        );
    }
    if !step.depends_on.is_empty() {
        bail!(
            "built-in gate {:?} may not declare external dependencies",
            step.id
        );
    }
    if !step.component.is_empty()
        || !step.program.is_empty()
        || !step.args.is_empty()
        || !step.cwd.is_empty()
        || !step.log.is_empty()
        || step.timeout_secs != 0
        || step.timeout_env.is_some()
        || step.parser.is_some()
        || !step.services.is_empty()
        || !step.remove_env.is_empty()
    {
        bail!(
            "built-in gate {:?} may not declare external-step fields",
            step.id
        );
    }
    Ok(())
}

pub(super) fn validate_arguments(config: &FlowConfig, owner: &str, args: &[String]) -> Result<()> {
    for arg in args {
        validate_template(config, owner, arg)?;
    }
    Ok(())
}

pub(super) fn validate_template(config: &FlowConfig, owner: &str, value: &str) -> Result<()> {
    if value.contains('\0') {
        bail!("{owner:?} contains a NUL value");
    }
    let mut rest = value;
    while let Some(start) = rest.find('{') {
        let tail = &rest[start..];
        let Some(end) = tail.find('}') else {
            bail!("{owner:?} contains an unterminated placeholder in {value:?}");
        };
        let name = &tail[1..end];
        if !config.allowed_placeholder(name) {
            bail!("{owner:?} contains unsupported placeholder {{{name}}}");
        }
        rest = &tail[end + 1..];
    }
    Ok(())
}

fn exact_placeholder(value: &str) -> Option<&str> {
    value.strip_prefix('{')?.strip_suffix('}')
}

fn is_shell(program: &str) -> bool {
    matches!(program, "sh" | "bash" | "dash" | "zsh")
}

fn is_shell_command_argument(arg: &str) -> bool {
    arg.starts_with("--command")
        || (arg.starts_with('-') && !arg.starts_with("--") && arg[1..].contains('c'))
}
