use super::model::{
    DoctorCheckKind, ExternalValuePolicy, FlowConfig, ParserConfig, ServiceConfig, StepConfig,
    CONFIG_VERSION,
};
use anyhow::{bail, Context, Result};
use globset::Glob;
use regex::Regex;
use std::collections::{BTreeSet, HashSet};
use std::path::{Component as PathComponent, Path};

impl FlowConfig {
    pub fn validate(&self) -> Result<()> {
        if self.version != CONFIG_VERSION {
            bail!(
                "unsupported workflow config version {}; expected {}; run `arc-flow config migrate` for v1 configurations",
                self.version,
                CONFIG_VERSION
            );
        }
        validate_id("project.name", &self.project.name)?;
        validate_id("project.default_profile", &self.project.default_profile)?;
        validate_id("project.hook_profile", &self.project.hook_profile)?;
        validate_repo_path("paths.reports", &self.paths.reports)?;
        validate_repo_path("paths.audit_config", &self.paths.audit_config)?;
        validate_repo_path("paths.secrets_config", &self.paths.secrets_config)?;

        for (alias, entry) in &self.paths.aliases {
            validate_id("path alias", alias)?;
            if matches!(
                alias.as_str(),
                "root" | "reports" | "audit_config" | "secrets_config" | "host_port"
            ) {
                bail!("path alias {alias:?} is reserved");
            }
            validate_repo_path(&format!("paths.aliases.{alias}"), &entry.path)?;
            if let Some(name) = &entry.env {
                validate_env_name(&format!("paths.aliases.{alias}.env"), name)?;
            }
        }

        self.validate_services()?;
        self.validate_parsers()?;
        self.validate_doctor()?;
        self.validate_scope()?;
        self.validate_steps()?;
        Ok(())
    }
    fn validate_services(&self) -> Result<()> {
        for (id, service) in &self.services {
            validate_id("service id", id)?;
            match service {
                ServiceConfig::Environment {
                    source_env,
                    inject_env,
                } => {
                    validate_env_name("service.source_env", source_env)?;
                    validate_env_name("service.inject_env", inject_env)?;
                }
                ServiceConfig::Docker {
                    image,
                    image_env,
                    external_env,
                    inject_env,
                    external_value_policy,
                    startup_timeout_secs,
                    timeout_env,
                    container_port,
                    environment,
                    healthcheck,
                    connection,
                } => {
                    validate_image(image)?;
                    if *startup_timeout_secs == 0 || *startup_timeout_secs > 300 {
                        bail!("service {id:?} startup_timeout_secs must be between 1 and 300");
                    }
                    if *container_port == 0 {
                        bail!("service {id:?} container_port must not be zero");
                    }
                    if let Some(name) = image_env {
                        validate_env_name("service.image_env", name)?;
                    }
                    if let Some(name) = timeout_env {
                        validate_env_name("service.timeout_env", name)?;
                    }
                    if let Some(name) = external_env {
                        validate_env_name("service.external_env", name)?;
                    }
                    if *external_value_policy != ExternalValuePolicy::None && external_env.is_none()
                    {
                        bail!("Docker service {id:?} external_value_policy requires external_env");
                    }
                    validate_env_name("service.inject_env", inject_env)?;
                    if healthcheck.is_empty() {
                        bail!("Docker service {id:?} requires a healthcheck command");
                    }
                    if !connection.contains("{host_port}") {
                        bail!("Docker service {id:?} connection must contain {{host_port}}");
                    }
                    for key in environment.keys() {
                        validate_env_name("service.environment key", key)?;
                    }
                    for value in environment.values().chain(healthcheck.iter()) {
                        if value.contains('\0') {
                            bail!("Docker service {id:?} contains a NUL value");
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_parsers(&self) -> Result<()> {
        for (id, parser) in &self.parsers {
            validate_id("parser id", id)?;
            match parser {
                ParserConfig::Regex {
                    patterns,
                    capture,
                    minimum,
                } => {
                    if patterns.is_empty() || *minimum == 0 {
                        bail!("parser {id:?} requires patterns and minimum greater than zero");
                    }
                    for pattern in patterns {
                        let regex = Regex::new(pattern)
                            .with_context(|| format!("parser {id:?} has invalid regex"))?;
                        if *capture >= regex.captures_len() {
                            bail!("parser {id:?} regex has no capture group {capture}");
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_doctor(&self) -> Result<()> {
        let mut ids = HashSet::new();
        for check in &self.doctor.checks {
            validate_id("doctor check id", &check.id)?;
            if !ids.insert(check.id.as_str()) {
                bail!("duplicate doctor check id {:?}", check.id);
            }
            if check.label.trim().is_empty() {
                bail!("doctor check {:?} requires a label", check.id);
            }
            if check.timeout_secs == 0 || check.timeout_secs > 300 {
                bail!(
                    "doctor check {:?} timeout_secs must be between 1 and 300",
                    check.id
                );
            }
            match &check.kind {
                DoctorCheckKind::Command { program, args } => {
                    validate_program("doctor command", program)?;
                    validate_arguments(self, &check.id, args)?;
                }
                DoctorCheckKind::Path { path, .. } | DoctorCheckKind::Glob { pattern: path } => {
                    validate_template(self, &check.id, path)?;
                }
                DoctorCheckKind::Env { name } => validate_env_name("doctor env", name)?,
                DoctorCheckKind::EnvOrFile {
                    env,
                    path,
                    contains,
                } => {
                    validate_env_name("doctor env", env)?;
                    validate_template(self, &check.id, path)?;
                    if contains.is_empty() {
                        bail!("doctor check {:?} requires non-empty contains", check.id);
                    }
                }
                DoctorCheckKind::GitConfig { key, expected } => {
                    if key.is_empty() || expected.is_empty() {
                        bail!(
                            "doctor Git config check {:?} requires key and expected",
                            check.id
                        );
                    }
                }
                DoctorCheckKind::GitRemotes => {}
                DoctorCheckKind::Version {
                    program,
                    args,
                    path,
                    ..
                } => {
                    validate_program("doctor version program", program)?;
                    validate_arguments(self, &check.id, args)?;
                    validate_template(self, &check.id, path)?;
                }
                DoctorCheckKind::Service { service } => {
                    if !self.services.contains_key(service) {
                        bail!(
                            "doctor check {:?} references unknown service {service:?}",
                            check.id
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_scope(&self) -> Result<()> {
        if self.scope.rules.is_empty() {
            bail!("scope.rules must not be empty");
        }
        let components = self.components();
        for (index, rule) in self.scope.rules.iter().enumerate() {
            if rule.patterns.is_empty() || rule.components.is_empty() {
                bail!("scope.rules[{index}] requires patterns and components");
            }
            for component in &rule.components {
                validate_id("scope component", component)?;
                if !components.contains(component) {
                    bail!("scope.rules[{index}] references component {component:?} with no steps");
                }
            }
            for pattern in &rule.patterns {
                if pattern.contains("..") || Path::new(pattern).is_absolute() {
                    bail!("scope.rules[{index}] contains unsafe pattern {pattern:?}");
                }
                Glob::new(pattern).with_context(|| {
                    format!("scope.rules[{index}] contains invalid pattern {pattern:?}")
                })?;
            }
        }
        Ok(())
    }

    fn validate_steps(&self) -> Result<()> {
        if self.steps.is_empty() {
            bail!("steps must not be empty");
        }
        let mut ids = HashSet::new();
        let mut profiles = BTreeSet::new();
        for step in &self.steps {
            if !ids.insert(step.id.as_str()) {
                bail!("duplicate verification step id {:?}", step.id);
            }
            validate_step(self, step)?;
            profiles.extend(step.profiles.iter().cloned());
        }
        for profile in [&self.project.default_profile, &self.project.hook_profile] {
            if !profiles.contains(profile) {
                bail!("configured profile {profile:?} is not used by any step");
            }
        }
        let mut required = HashSet::new();
        for id in &self.policy.required_steps {
            if !required.insert(id.as_str()) {
                bail!("duplicate policy.required_steps entry {id:?}");
            }
            if !ids.contains(id.as_str()) {
                bail!("policy requires missing verification step {id:?}");
            }
        }
        Ok(())
    }
}
fn validate_step(config: &FlowConfig, step: &StepConfig) -> Result<()> {
    validate_id("verification step id", &step.id)?;
    validate_id("step component", &step.component)?;
    if step.label.trim().is_empty() || step.profiles.is_empty() {
        bail!(
            "step {:?} requires a label and at least one profile",
            step.id
        );
    }
    for profile in &step.profiles {
        validate_id("step profile", profile)?;
    }
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
    let log = Path::new(&step.log);
    if log.components().count() != 1 || log.extension().is_none_or(|value| value != "log") {
        bail!("step {:?} log must be a single .log file name", step.id);
    }
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

fn validate_arguments(config: &FlowConfig, owner: &str, args: &[String]) -> Result<()> {
    for arg in args {
        validate_template(config, owner, arg)?;
    }
    Ok(())
}

fn validate_template(config: &FlowConfig, owner: &str, value: &str) -> Result<()> {
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

fn validate_id(name: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || !value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '-' | '_')
        })
    {
        bail!("{name} must be a lowercase identifier, found {value:?}");
    }
    Ok(())
}

fn validate_program(name: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.contains('/')
        || value.contains('\\')
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '+')
        })
    {
        bail!("{name} must be a bare executable name, found {value:?}");
    }
    Ok(())
}

fn validate_env_name(name: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || !value.chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        })
    {
        bail!("{name} must be an uppercase environment variable name, found {value:?}");
    }
    Ok(())
}

fn validate_image(value: &str) -> Result<()> {
    if value.is_empty()
        || value.starts_with('-')
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '.' | '_' | '/' | ':' | '@' | '-')
        })
    {
        bail!("Docker image must be an OCI image reference, found {value:?}");
    }
    Ok(())
}

fn validate_repo_path(name: &str, value: &str) -> Result<()> {
    let path = Path::new(value);
    if value.is_empty() || path.is_absolute() {
        bail!("{name} must be a non-empty repository-relative path");
    }
    if path.components().any(|component| {
        matches!(
            component,
            PathComponent::ParentDir | PathComponent::RootDir | PathComponent::Prefix(_)
        )
    }) {
        bail!("{name} may not escape the repository: {value:?}");
    }
    Ok(())
}
