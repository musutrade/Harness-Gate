use super::model::{
    DoctorCheckKind, ExternalValuePolicy, FlowConfig, ParserConfig, ServiceConfig, CONFIG_VERSION,
};
use anyhow::{bail, Context, Result};
use globset::Glob;
use regex::Regex;
use std::collections::{BTreeSet, HashSet};
use std::path::Path;

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
        for step in &self.steps {
            for dependency in &step.depends_on {
                if dependency == &step.id {
                    bail!("step {:?} may not depend on itself", step.id);
                }
                if !ids.contains(dependency.as_str()) {
                    bail!(
                        "step {:?} depends on missing step {:?}",
                        step.id,
                        dependency
                    );
                }
            }
        }
        validate_step_dependencies(self)?;
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

fn validate_step_dependencies(config: &FlowConfig) -> Result<()> {
    fn visit(
        id: &str,
        config: &FlowConfig,
        visiting: &mut HashSet<String>,
        visited: &mut HashSet<String>,
    ) -> Result<()> {
        if visited.contains(id) {
            return Ok(());
        }
        if !visiting.insert(id.to_string()) {
            bail!("verification step dependency cycle includes {:?}", id);
        }
        let step = config.step(id).expect("validated dependency");
        for dependency in &step.depends_on {
            visit(dependency, config, visiting, visited)?;
        }
        visiting.remove(id);
        visited.insert(id.to_string());
        Ok(())
    }
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for step in &config.steps {
        visit(&step.id, config, &mut visiting, &mut visited)?;
    }
    Ok(())
}

mod primitives;
mod steps;

use primitives::{
    validate_env_name, validate_id, validate_image, validate_program, validate_repo_path,
};
use steps::{validate_arguments, validate_step, validate_template};
