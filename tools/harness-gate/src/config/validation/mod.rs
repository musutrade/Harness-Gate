use super::diagnostic::{
    ConfigDiagnostic, ConfigDiagnostics, DiagnosticSeverity, RelatedDiagnostic, SourceMap,
};
use super::model::{
    DoctorCheckKind, ExternalValuePolicy, FlowConfig, ParserConfig, ServiceConfig, CONFIG_VERSION,
};
use anyhow::{bail, Context, Result};
use globset::Glob;
use regex::Regex;
use std::collections::{BTreeSet, HashSet};
use std::path::Path;

impl FlowConfig {
    pub(super) fn validate_with_diagnostics(
        &self,
        source_map: &SourceMap,
        source_path: Option<&Path>,
        repository_root: Option<&Path>,
    ) -> std::result::Result<(), ConfigDiagnostics> {
        let mut diagnostics = ConfigDiagnostics::empty();
        if let Some(source) = source_path {
            diagnostics = diagnostics.with_source(source);
        }
        self.collect_semantic_diagnostics(source_map, &mut diagnostics);
        if !diagnostics.is_empty() {
            diagnostics.sort();
            return Err(diagnostics);
        }
        validate_resource_conflicts(self, source_map, &mut diagnostics);
        if let Some(root) = repository_root {
            validate_report_template_paths(self, root, source_map, &mut diagnostics);
        }
        diagnostics.sort();
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    fn collect_semantic_diagnostics(
        &self,
        source_map: &SourceMap,
        diagnostics: &mut ConfigDiagnostics,
    ) {
        collect_result(
            self,
            source_map,
            diagnostics,
            "version",
            validate_version(self.version),
        );
        for (path, value) in [
            ("project.name", &self.project.name),
            ("project.default_profile", &self.project.default_profile),
            ("project.hook_profile", &self.project.hook_profile),
        ] {
            collect_result(
                self,
                source_map,
                diagnostics,
                path,
                validate_id(path, value),
            );
        }
        for (path, value) in [
            ("paths.reports", &self.paths.reports),
            ("paths.audit_config", &self.paths.audit_config),
            ("paths.secrets_config", &self.paths.secrets_config),
        ] {
            collect_result(
                self,
                source_map,
                diagnostics,
                path,
                validate_repo_path(path, value),
            );
        }
        for (alias, entry) in &self.paths.aliases {
            let path = format!("paths.aliases[\"{alias}\"]");
            collect_result(
                self,
                source_map,
                diagnostics,
                &path,
                validate_path_alias(alias, entry),
            );
        }
        for (id, service) in &self.services {
            let mut candidate = self.clone();
            candidate.services.clear();
            candidate.services.insert(id.clone(), service.clone());
            let path = format!("services[\"{id}\"]");
            collect_result(
                self,
                source_map,
                diagnostics,
                &path,
                candidate.validate_services(),
            );
        }
        for (id, parser) in &self.parsers {
            let mut candidate = self.clone();
            candidate.parsers.clear();
            candidate.parsers.insert(id.clone(), parser.clone());
            let path = format!("parsers[\"{id}\"]");
            collect_result(
                self,
                source_map,
                diagnostics,
                &path,
                candidate.validate_parsers(),
            );
        }
        for (index, check) in self.doctor.checks.iter().enumerate() {
            let mut candidate = self.clone();
            candidate.doctor.checks = vec![check.clone()];
            collect_result(
                self,
                source_map,
                diagnostics,
                &format!("doctor.checks[{index}]"),
                candidate.validate_doctor(),
            );
        }
        collect_duplicate_doctor_ids(self, source_map, diagnostics);
        if self.scope.rules.is_empty() {
            push_diagnostic(
                self,
                source_map,
                diagnostics,
                "scope.rules",
                "scope.rules must not be empty",
            );
        } else {
            for (index, rule) in self.scope.rules.iter().enumerate() {
                let mut candidate = self.clone();
                candidate.scope.rules = vec![rule.clone()];
                collect_result(
                    self,
                    source_map,
                    diagnostics,
                    &format!("scope.rules[{index}]"),
                    candidate.validate_scope(),
                );
            }
        }
        self.collect_step_diagnostics(source_map, diagnostics);
        collect_result(
            self,
            source_map,
            diagnostics,
            "report_templates",
            self.validate_report_templates(),
        );
        collect_result(
            self,
            source_map,
            diagnostics,
            "execution",
            self.validate_execution(),
        );
        collect_result(
            self,
            source_map,
            diagnostics,
            "notifications",
            self.validate_notifications(),
        );
    }

    fn collect_step_diagnostics(
        &self,
        source_map: &SourceMap,
        diagnostics: &mut ConfigDiagnostics,
    ) {
        if self.steps.is_empty() {
            push_diagnostic(
                self,
                source_map,
                diagnostics,
                "steps",
                "steps must not be empty",
            );
            return;
        }

        let mut ids = HashSet::new();
        let mut profiles = BTreeSet::new();
        let mut dependencies_are_valid = true;
        for (index, step) in self.steps.iter().enumerate() {
            let path = format!("steps[{index}]");
            if !ids.insert(step.id.as_str()) {
                push_diagnostic(
                    self,
                    source_map,
                    diagnostics,
                    &format!("{path}.id"),
                    "duplicate verification step id",
                );
            }
            collect_step_field_diagnostics(self, step, index, source_map, diagnostics);
            profiles.extend(step.profiles.iter().cloned());
        }
        for (index, step) in self.steps.iter().enumerate() {
            for dependency in &step.depends_on {
                let path = format!("steps[{index}].depends_on");
                if dependency == &step.id {
                    dependencies_are_valid = false;
                    push_diagnostic(
                        self,
                        source_map,
                        diagnostics,
                        &path,
                        "a step may not depend on itself",
                    );
                } else if !ids.contains(dependency.as_str()) {
                    dependencies_are_valid = false;
                    push_diagnostic(
                        self,
                        source_map,
                        diagnostics,
                        &path,
                        "a dependency references a missing step",
                    );
                }
            }
        }
        if dependencies_are_valid {
            collect_result(
                self,
                source_map,
                diagnostics,
                "steps",
                validate_step_dependencies(self),
            );
        }
        for (path, profile) in [
            ("project.default_profile", &self.project.default_profile),
            ("project.hook_profile", &self.project.hook_profile),
        ] {
            if !profiles.contains(profile) {
                push_diagnostic(
                    self,
                    source_map,
                    diagnostics,
                    path,
                    "configured profile is not used by any step",
                );
            }
        }
        let mut required = HashSet::new();
        for id in &self.policy.required_steps {
            if !required.insert(id.as_str()) {
                push_diagnostic(
                    self,
                    source_map,
                    diagnostics,
                    "policy.required_steps",
                    "policy.required_steps contains a duplicate step id",
                );
            } else if !ids.contains(id.as_str()) {
                push_diagnostic(
                    self,
                    source_map,
                    diagnostics,
                    "policy.required_steps",
                    "policy requires a missing verification step",
                );
            }
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != CONFIG_VERSION {
            bail!(
                "unsupported workflow config version {}; expected {}; run `harness-gate config migrate` for v1 configurations",
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
        self.validate_report_templates()?;
        self.validate_execution()?;
        self.validate_notifications()?;
        Ok(())
    }

    fn validate_notifications(&self) -> Result<()> {
        for (index, webhook) in self.notifications.webhooks.iter().enumerate() {
            let parsed = url::Url::parse(&webhook.url)
                .with_context(|| format!("notifications.webhooks[{index}].url is invalid"))?;
            if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
                bail!("notifications.webhooks[{index}].url must be an http(s) URL");
            }
            if !webhook.on_failure && !webhook.on_success {
                bail!("notifications.webhooks[{index}] must enable on_failure or on_success");
            }
        }
        Ok(())
    }

    fn validate_execution(&self) -> Result<()> {
        if let Some(max_parallel) = self.execution.max_parallel {
            if max_parallel == 0 || max_parallel > 64 {
                bail!("execution.max_parallel must be between 1 and 64 (got {max_parallel})");
            }
        }
        Ok(())
    }

    fn validate_report_templates(&self) -> Result<()> {
        match (&self.report_templates.root, &self.report_templates.template) {
            (None, None) if self.report_templates.junit.is_none() => Ok(()),
            (Some(root), Some(template)) => {
                validate_repo_path("report_templates.root", root)?;
                validate_repo_path("report_templates.template", template)?;
                if !template.ends_with(".html") && !template.ends_with(".tera") {
                    bail!("report_templates.template must use a .html or .tera extension");
                }
                if let Some(junit) = &self.report_templates.junit {
                    validate_repo_path("report_templates.junit", junit)?;
                    if !junit.ends_with(".xml") {
                        bail!("report_templates.junit must use a .xml extension");
                    }
                }
                Ok(())
            }
            (None, None) => {
                if let Some(junit) = &self.report_templates.junit {
                    validate_repo_path("report_templates.junit", junit)?;
                    if !junit.ends_with(".xml") {
                        bail!("report_templates.junit must use a .xml extension");
                    }
                }
                Ok(())
            }
            _ => bail!(
                "report_templates.root and report_templates.template must be configured together"
            ),
        }
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
                    runtime: _,
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

fn infer_error_path(config: &FlowConfig, error: &str) -> String {
    for field in [
        "paths.reports",
        "paths.audit_config",
        "paths.secrets_config",
        "project.name",
        "project.default_profile",
        "project.hook_profile",
        "report_templates.root",
        "report_templates.template",
        "report_templates.junit",
        "execution.max_parallel",
        "notifications.webhooks",
    ] {
        if error.contains(field) {
            return field.into();
        }
    }
    let quoted = error
        .split_once('"')
        .and_then(|(_, rest)| rest.split_once('"').map(|(value, _)| value));
    if error.starts_with("step ") {
        let index = quoted.and_then(|id| config.steps.iter().position(|step| step.id == id));
        let base = index.map_or_else(|| "steps[*]".into(), |index| format!("steps[{index}]"));
        if error.contains("timeout") {
            format!("{base}.timeout_secs")
        } else if error.contains("parser") {
            format!("{base}.parser")
        } else if error.contains("log") {
            format!("{base}.log")
        } else if error.contains("depend") {
            format!("{base}.depends_on")
        } else {
            base
        }
    } else if error.starts_with("service ") {
        let base = quoted.map_or_else(|| "services[*]".into(), |id| format!("services[\"{id}\"]"));
        if error.contains("inject_env") {
            format!("{base}.inject_env")
        } else {
            base
        }
    } else if error.starts_with("parser ") {
        quoted.map_or_else(|| "parsers[*]".into(), |id| format!("parsers[\"{id}\"]"))
    } else if error.contains("timeout_secs") {
        "steps[*].timeout_secs".into()
    } else if error.contains("log") {
        "steps[*].log".into()
    } else if error.contains("scope") {
        "scope".into()
    } else {
        "$".into()
    }
}

fn diagnostic_id(error: &str) -> &'static str {
    if error.contains("unsupported workflow config version") {
        "HGCFG-VERSION"
    } else if error.contains("cycle") {
        "HGCFG-DEPENDENCY-CYCLE"
    } else if error.contains("dependenc") {
        "HGCFG-DEPENDENCY"
    } else if error.contains("multiple services injecting") {
        "HGCFG-SERVICE-INJECT-COLLISION"
    } else if error.contains("unknown service") || error.contains("unknown parser") {
        "HGCFG-UNKNOWN-REFERENCE"
    } else if error.contains("repository-relative") || error.contains("escape the repository") {
        "HGCFG-INVALID-PATH"
    } else if error.contains("environment variable name") {
        "HGCFG-INVALID-ENVIRONMENT"
    } else if error.contains("duplicate") {
        "HGCFG-DUPLICATE-FIELD"
    } else if error.contains("log") {
        "HGCFG-INVALID-LOG"
    } else {
        "HGCFG-INVALID-FIELD"
    }
}

fn redact_error(error: &str) -> String {
    if error.contains("unsupported workflow config version") {
        "workflow configuration version is unsupported".into()
    } else if error.contains("timeout") {
        "timeout is outside the accepted range".into()
    } else if error.contains("unknown parser") || error.contains("unknown service") {
        "reference does not name a declared configuration entry".into()
    } else if error.contains("repository-relative") || error.contains("escape the repository") {
        "path is not a safe repository-relative path".into()
    } else if error.contains("environment variable name") {
        "environment variable name is invalid".into()
    } else if error.contains("duplicate") {
        "configuration declares the same identifier more than once".into()
    } else if error.contains("must be") || error.contains("requires") {
        "field violates a documented configuration constraint".into()
    } else if error.contains("cycle") {
        "dependencies contain a cycle".into()
    } else if error.contains("depend") {
        "dependency reference is invalid".into()
    } else {
        "configuration validation failed".into()
    }
}

fn repair_help(error: &str) -> String {
    if error.contains("unsupported workflow config version") {
        "migrate a v1 file with `harness-gate config migrate` or use version 2".into()
    } else if error.contains("cycle") || error.contains("dependenc") {
        "remove the invalid dependency or add an explicit acyclic prerequisite".into()
    } else if error.contains("multiple services injecting") {
        "use one service or distinct inject_env names".into()
    } else if error.contains("log") {
        "choose a unique single .log filename".into()
    } else if error.contains("repository-relative") || error.contains("escape the repository") {
        "use a non-empty path below the repository without a prefix or parent traversal".into()
    } else if error.contains("environment variable name") {
        "use an uppercase environment-variable name containing only letters, digits, and underscores"
            .into()
    } else if error.contains("duplicate") {
        "keep one declaration or give each declaration a unique identifier".into()
    } else if error.contains("unknown") {
        "use an identifier declared in the configuration".into()
    } else {
        "update the field to satisfy the documented configuration constraints".into()
    }
}

fn validate_resource_conflicts(
    config: &FlowConfig,
    source_map: &SourceMap,
    diagnostics: &mut ConfigDiagnostics,
) {
    let mut logs = std::collections::BTreeMap::<String, usize>::new();
    for (index, step) in config.steps.iter().enumerate() {
        // Built-in gates do not own external log files. Their empty `log`
        // field is intentional and must not collide with another gate.
        if step.kind.as_deref() == Some("builtin-gate") {
            continue;
        }
        let identity = steps::normalize_log_identity(&step.log);
        if let Some(previous) = logs.insert(identity, index) {
            let path = format!("steps[{index}].log");
            let related_path = format!("steps[{previous}].log");
            diagnostics.push(ConfigDiagnostic {
                id: "HGCFG-DUPLICATE-LOG".into(),
                severity: DiagnosticSeverity::Error,
                path: path.clone(),
                message: format!("log filename is also used by step {previous}"),
                help: "choose a unique .log filename for each verification step".into(),
                location: source_map.location(&path),
                related: vec![RelatedDiagnostic {
                    path: related_path.clone(),
                    relation: "conflicts-with".into(),
                    location: source_map.location(&related_path),
                }],
            });
        }
    }

    let ids = config
        .steps
        .iter()
        .map(|step| step.id.as_str())
        .collect::<Vec<_>>();
    let reachable = |from: usize, to: usize| -> bool {
        let mut stack = vec![from];
        let mut seen = std::collections::HashSet::new();
        while let Some(index) = stack.pop() {
            if !seen.insert(index) {
                continue;
            }
            for dependency in &config.steps[index].depends_on {
                if let Some(next) = ids.iter().position(|id| id == dependency) {
                    if next == to {
                        return true;
                    }
                    stack.push(next);
                }
            }
        }
        false
    };

    for left in 0..config.steps.len() {
        for right in (left + 1)..config.steps.len() {
            if reachable(left, right) || reachable(right, left) {
                continue;
            }
            let left_services = config.steps[left]
                .services
                .iter()
                .enumerate()
                .filter_map(|(index, id)| {
                    config.services.get(id).map(|service| (index, id, service))
                })
                .collect::<Vec<_>>();
            let right_services = config.steps[right]
                .services
                .iter()
                .enumerate()
                .filter_map(|(index, id)| {
                    config.services.get(id).map(|service| (index, id, service))
                })
                .collect::<Vec<_>>();

            let mut shared_service_ids = BTreeSet::new();
            for (_, left_service, _) in &left_services {
                if right_services
                    .iter()
                    .any(|(_, right_service, _)| right_service == left_service)
                {
                    shared_service_ids.insert(*left_service);
                }
            }
            for service_id in shared_service_ids {
                let (left_service_index, _, _) = left_services
                    .iter()
                    .find(|(_, id, _)| *id == service_id)
                    .expect("service appears in left step");
                let (right_service_index, _, _) = right_services
                    .iter()
                    .find(|(_, id, _)| *id == service_id)
                    .expect("service appears in right step");
                let path = format!("steps[{right}].services[{right_service_index}]");
                let related_path = format!("steps[{left}].services[{left_service_index}]");
                diagnostics.push(ConfigDiagnostic {
                    id: "HGCFG-SHARED-SERVICE".into(),
                    severity: DiagnosticSeverity::Error,
                    path: path.clone(),
                    message: format!("steps use shared service {service_id:?} without ordering"),
                    help: "add a dependency or define separate service resources".into(),
                    location: source_map.location(&path),
                    related: vec![
                        RelatedDiagnostic {
                            path: related_path.clone(),
                            relation: "conflicts-with".into(),
                            location: source_map.location(&related_path),
                        },
                        RelatedDiagnostic {
                            path: format!("services[\"{service_id}\"]"),
                            relation: "shared-resource".into(),
                            location: source_map.location(&format!("services[\"{service_id}\"]")),
                        },
                    ],
                });
            }

            let mut seen_injections = BTreeSet::new();
            for (left_service_index, left_service, left_config) in &left_services {
                for (right_service_index, right_service, right_config) in &right_services {
                    if left_service == right_service
                        || service_inject_env(left_config) != service_inject_env(right_config)
                    {
                        continue;
                    }
                    let key = (
                        *left_service,
                        *right_service,
                        service_inject_env(left_config),
                    );
                    if !seen_injections.insert(key) {
                        continue;
                    }
                    let path = format!("steps[{right}].services[{right_service_index}]");
                    let related_path = format!("steps[{left}].services[{left_service_index}]");
                    diagnostics.push(ConfigDiagnostic {
                        id: "HGCFG-SERVICE-INJECT-COLLISION".into(),
                        severity: DiagnosticSeverity::Error,
                        path: path.clone(),
                        message: "independent services inject the same environment variable".into(),
                        help:
                            "add a dependency, use distinct inject_env names, or split the workflow"
                                .into(),
                        location: source_map.location(&path),
                        related: vec![
                            RelatedDiagnostic {
                                path: related_path.clone(),
                                relation: "conflicts-with".into(),
                                location: source_map.location(&related_path),
                            },
                            RelatedDiagnostic {
                                path: format!("services[\"{left_service}\"].inject_env"),
                                relation: "injects".into(),
                                location: source_map
                                    .location(&format!("services[\"{left_service}\"].inject_env")),
                            },
                            RelatedDiagnostic {
                                path: format!("services[\"{right_service}\"].inject_env"),
                                relation: "injects".into(),
                                location: source_map
                                    .location(&format!("services[\"{right_service}\"].inject_env")),
                            },
                        ],
                    });
                }
            }
        }
    }
}

fn validate_report_template_paths(
    config: &FlowConfig,
    repository_root: &Path,
    source_map: &SourceMap,
    diagnostics: &mut ConfigDiagnostics,
) {
    let (Some(template_root), Some(template)) = (
        config.report_templates.root.as_deref(),
        config.report_templates.template.as_deref(),
    ) else {
        return;
    };
    let repository_root = match repository_root.canonicalize() {
        Ok(root) => root,
        Err(_) => return,
    };
    let template_root_path = repository_root.join(template_root);
    let template_path = repository_root.join(template);
    let reports_path = repository_root.join(&config.paths.reports);
    let resolved_root = template_root_path.canonicalize();
    let resolved_template = template_path.canonicalize();
    let resolved_reports = reports_path.canonicalize().unwrap_or(reports_path);

    let invalid = |diagnostics: &mut ConfigDiagnostics,
                   path: &str,
                   message: String,
                   help: &str,
                   related: Vec<RelatedDiagnostic>| {
        diagnostics.push(ConfigDiagnostic {
            id: "HGCFG-TEMPLATE-PATH".into(),
            severity: DiagnosticSeverity::Error,
            path: path.into(),
            message,
            help: help.into(),
            location: source_map.location(path),
            related,
        });
    };

    let Ok(resolved_root) = resolved_root else {
        invalid(
            diagnostics,
            "report_templates.root",
            "template root does not exist".into(),
            "create a repository-contained template directory before enabling templates",
            Vec::new(),
        );
        return;
    };
    if !resolved_root.is_dir() || !resolved_root.starts_with(&repository_root) {
        invalid(
            diagnostics,
            "report_templates.root",
            "template root must resolve to a repository-contained directory".into(),
            "use a directory below the repository that does not traverse a symlink outside it",
            Vec::new(),
        );
        return;
    }
    let Ok(resolved_template) = resolved_template else {
        invalid(
            diagnostics,
            "report_templates.template",
            "template file does not exist".into(),
            "create a regular .html or .tera file below the configured template root",
            Vec::new(),
        );
        return;
    };
    if !resolved_template.is_file()
        || !resolved_template.starts_with(&resolved_root)
        || !resolved_template.starts_with(&repository_root)
    {
        invalid(
            diagnostics,
            "report_templates.template",
            "template file escapes the approved template root or is not a regular file".into(),
            "use a regular .html or .tera file below the configured template root without escaping symlinks",
            vec![RelatedDiagnostic {
                path: "report_templates.root".into(),
                relation: "must-be-under".into(),
                location: source_map.location("report_templates.root"),
            }],
        );
        return;
    }
    if resolved_root == resolved_reports
        || resolved_root.starts_with(&resolved_reports)
        || resolved_reports.starts_with(&resolved_root)
    {
        invalid(
            diagnostics,
            "report_templates.root",
            "template root overlaps the report output directory".into(),
            "use a read-only template directory disjoint from paths.reports",
            vec![RelatedDiagnostic {
                path: "paths.reports".into(),
                relation: "must-not-overlap".into(),
                location: source_map.location("paths.reports"),
            }],
        );
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

fn validate_version(version: u32) -> Result<()> {
    if version != CONFIG_VERSION {
        bail!(
            "unsupported workflow config version {}; expected {}; run `harness-gate config migrate` for v1 configurations",
            version,
            CONFIG_VERSION
        );
    }
    Ok(())
}

fn validate_path_alias(alias: &str, entry: &super::model::PathAlias) -> Result<()> {
    validate_id("path alias", alias)?;
    if matches!(
        alias,
        "root" | "reports" | "audit_config" | "secrets_config" | "host_port"
    ) {
        bail!("path alias {alias:?} is reserved");
    }
    validate_repo_path(&format!("paths.aliases.{alias}"), &entry.path)?;
    if let Some(name) = &entry.env {
        validate_env_name(&format!("paths.aliases.{alias}.env"), name)?;
    }
    Ok(())
}

fn push_diagnostic(
    _config: &FlowConfig,
    source_map: &SourceMap,
    diagnostics: &mut ConfigDiagnostics,
    path: &str,
    message: &str,
) {
    diagnostics.push(ConfigDiagnostic {
        id: "HGCFG-INVALID-FIELD".into(),
        severity: DiagnosticSeverity::Error,
        path: path.into(),
        message: message.into(),
        help: repair_help(message),
        location: source_map.location(path),
        related: Vec::new(),
    });
}

fn collect_result(
    config: &FlowConfig,
    source_map: &SourceMap,
    diagnostics: &mut ConfigDiagnostics,
    path: &str,
    result: Result<()>,
) {
    if let Err(error) = result {
        let text = error.to_string();
        let inferred = infer_error_path(config, &text);
        let path = if inferred == "$" || inferred.contains('*') || inferred == "scope" {
            path.to_string()
        } else {
            inferred
        };
        diagnostics.push(ConfigDiagnostic {
            id: diagnostic_id(&text).into(),
            severity: DiagnosticSeverity::Error,
            path: path.clone(),
            message: redact_error(&text),
            help: repair_help(&text),
            location: source_map.location(&path),
            related: Vec::new(),
        });
    }
}

fn collect_duplicate_doctor_ids(
    config: &FlowConfig,
    source_map: &SourceMap,
    diagnostics: &mut ConfigDiagnostics,
) {
    let mut seen = HashSet::new();
    for (index, check) in config.doctor.checks.iter().enumerate() {
        if !seen.insert(check.id.as_str()) {
            push_diagnostic(
                config,
                source_map,
                diagnostics,
                &format!("doctor.checks[{index}].id"),
                "duplicate doctor check id",
            );
        }
    }
}

fn collect_step_field_diagnostics(
    config: &FlowConfig,
    step: &super::model::StepConfig,
    index: usize,
    source_map: &SourceMap,
    diagnostics: &mut ConfigDiagnostics,
) {
    let base = format!("steps[{index}]");
    if let Err(error) = validate_step(config, step) {
        let text = error.to_string();
        let inferred = infer_step_error_path(config, step, index, &text);
        let path = if inferred == "$" || inferred.contains('*') {
            base
        } else {
            inferred
        };
        diagnostics.push(ConfigDiagnostic {
            id: diagnostic_id(&text).into(),
            severity: DiagnosticSeverity::Error,
            path: path.clone(),
            message: redact_error(&text),
            help: repair_help(&text),
            location: source_map.location(&path),
            related: Vec::new(),
        });
    }
}

fn infer_step_error_path(
    config: &FlowConfig,
    step: &super::model::StepConfig,
    index: usize,
    error: &str,
) -> String {
    let base = format!("steps[{index}]");
    if error.contains("multiple services injecting") {
        let injected = error
            .split_once("injecting ")
            .map(|(_, value)| value.trim());
        if let Some(injected) = injected {
            let mut seen = HashSet::new();
            for (service_index, service_id) in step.services.iter().enumerate() {
                if let Some(service) = config.services.get(service_id) {
                    let name = service_inject_env(service);
                    if name == injected && !seen.insert(name) {
                        return format!("{base}.services[{service_index}]");
                    }
                    seen.insert(name);
                }
            }
        }
    }
    if error.contains("duplicate service") {
        if let Some(id) = error
            .split_once("duplicate service ")
            .and_then(|(_, value)| {
                value
                    .strip_prefix('"')
                    .and_then(|rest| rest.split_once('"').map(|(id, _)| id))
            })
        {
            let mut duplicate_seen = false;
            for (service_index, service) in step.services.iter().enumerate() {
                if service == id {
                    if duplicate_seen {
                        return format!("{base}.services[{service_index}]");
                    }
                    duplicate_seen = true;
                }
            }
        }
    }
    if error.contains("unknown service") {
        if let Some(id) = error.split_once("unknown service ").and_then(|(_, value)| {
            value
                .strip_prefix('"')
                .and_then(|rest| rest.split_once('"').map(|(id, _)| id))
        }) {
            if let Some(service_index) = step.services.iter().position(|service| service == id) {
                return format!("{base}.services[{service_index}]");
            }
        }
    }
    if error.contains("remove service injection") {
        return format!("{base}.remove_env");
    }
    if error.contains("gate_type") {
        return format!("{base}.gate_type");
    }
    if error.contains("unknown kind") {
        return format!("{base}.kind");
    }
    let inferred = infer_error_path(config, error);
    if inferred == "$" || inferred.contains('*') {
        base
    } else {
        inferred
    }
}

fn service_inject_env(service: &ServiceConfig) -> &str {
    match service {
        ServiceConfig::Docker { inject_env, .. }
        | ServiceConfig::Environment { inject_env, .. } => inject_env,
    }
}
