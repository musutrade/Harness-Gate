use super::diagnostic::{interpolation_diagnostic, parse_diagnostic, ConfigDiagnostics, SourceMap};
use super::model::{FlowConfig, ParserConfig, ServiceConfig, StepConfig};
use anyhow::{Context, Result};
use schemars::schema_for;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

impl FlowConfig {
    #[allow(dead_code)]
    pub fn load(path: &Path) -> Result<Self> {
        Self::load_with_diagnostics(path, None).map_err(anyhow::Error::from)
    }

    pub fn load_with_diagnostics(
        path: &Path,
        repository_root: Option<&Path>,
    ) -> std::result::Result<Self, ConfigDiagnostics> {
        let source = fs::read_to_string(path).map_err(|_| {
            ConfigDiagnostics::single(
                "HGCFG-READ",
                "$",
                "workflow configuration could not be read",
                "check that the configured file exists and is readable",
            )
            .with_source(path)
        })?;
        Self::from_source_with_diagnostics(&source, Some(path), repository_root)
    }

    pub fn from_source(source: &str) -> Result<Self> {
        Self::from_source_with_diagnostics(source, None, None).map_err(anyhow::Error::from)
    }

    pub fn from_source_with_diagnostics(
        source: &str,
        source_path: Option<&Path>,
        repository_root: Option<&Path>,
    ) -> std::result::Result<Self, ConfigDiagnostics> {
        let source_map = SourceMap::from_source(source);
        let source = interpolate_environment(source, source_path)?;
        let mut config: Self = toml::from_str(&source)
            .map_err(|error| parse_diagnostic(&source, error, source_path))?;
        config.apply_environment().map_err(|error| {
            let path = if error.to_string().contains("integer") {
                "steps[*].timeout_env"
            } else {
                "$"
            };
            ConfigDiagnostics::single(
                "HGCFG-ENVIRONMENT-OVERRIDE",
                path,
                "an environment override has an invalid value",
                "set the named override to the required value type or unset it",
            )
            .with_source_opt(source_path)
        })?;
        config.validate_with_diagnostics(&source_map, source_path, repository_root)?;
        Ok(config)
    }

    pub fn components(&self) -> BTreeSet<String> {
        self.steps
            .iter()
            .filter(|step| !step.component.is_empty())
            .map(|step| step.component.clone())
            .collect()
    }

    pub fn diagnostics_report(&self) -> super::diagnostic::ConfigCheckReport {
        ConfigDiagnostics::empty().report()
    }

    pub fn step(&self, id: &str) -> Option<&StepConfig> {
        self.steps.iter().find(|step| step.id == id)
    }

    pub fn parser(&self, id: &str) -> Option<&ParserConfig> {
        self.parsers.get(id)
    }

    pub fn service(&self, id: &str) -> Option<&ServiceConfig> {
        self.services.get(id)
    }

    pub fn allowed_placeholder(&self, name: &str) -> bool {
        matches!(name, "root" | "reports" | "audit_config" | "secrets_config")
            || self.paths.aliases.contains_key(name)
    }

    fn apply_environment(&mut self) -> Result<()> {
        override_string("REPORT_DIR", &mut self.paths.reports);
        override_string("HARNESS_GATE_REPORTS", &mut self.paths.reports);
        override_string("AUDITOR_CONFIG", &mut self.paths.audit_config);
        override_string("HARNESS_GATE_AUDIT_CONFIG", &mut self.paths.audit_config);
        override_string(
            "HARNESS_GATE_SECRETS_CONFIG",
            &mut self.paths.secrets_config,
        );

        for entry in self.paths.aliases.values_mut() {
            if let Some(name) = &entry.env {
                override_string(name, &mut entry.path);
            }
        }
        for service in self.services.values_mut() {
            if let ServiceConfig::Docker {
                runtime: _,
                image,
                image_env,
                startup_timeout_secs,
                timeout_env,
                ..
            } = service
            {
                if let Some(name) = image_env {
                    override_string(name, image);
                }
                if let Some(name) = timeout_env {
                    override_u64(name, startup_timeout_secs)?;
                }
            }
        }
        for step in &mut self.steps {
            if let Some(name) = &step.timeout_env {
                override_u64(name, &mut step.timeout_secs)?;
            }
        }
        Ok(())
    }
}

pub fn schema_json() -> Result<String> {
    serde_json::to_string_pretty(&schema_for!(FlowConfig)).context("serialize workflow schema")
}

fn interpolate_environment(
    source: &str,
    source_path: Option<&Path>,
) -> std::result::Result<String, ConfigDiagnostics> {
    let mut output = String::with_capacity(source.len());
    let mut rest = source;
    let mut consumed = 0usize;
    while let Some(start) = rest.find("${") {
        output.push_str(&rest[..start]);
        let end = rest[start + 2..].find('}').ok_or_else(|| {
            interpolation_diagnostic(
                source,
                consumed..consumed + start + 2,
                "environment interpolation is unterminated",
                "close the expression with `}` or remove the incomplete `${...` token",
                source_path,
            )
        })? + start
            + 2;
        let expression = &rest[start + 2..end];
        let (name, default) = expression
            .split_once(":-")
            .map_or((expression, None), |(name, default)| (name, Some(default)));
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(interpolation_diagnostic(
                source,
                consumed + start..consumed + end + 1,
                "environment interpolation has an invalid variable name",
                "use an ASCII letter, digit, or underscore after `${`",
                source_path,
            ));
        }
        let value = std::env::var(name)
            .ok()
            .or_else(|| default.map(str::to_owned))
            .ok_or_else(|| {
                interpolation_diagnostic(
                    source,
                    consumed + start..consumed + end + 1,
                    format!("environment variable {name} is not set and has no default"),
                    format!("set {name} or use `${{{name}:-default}}`"),
                    source_path,
                )
            })?;
        output.push_str(&value);
        rest = &rest[end + 1..];
        consumed += end + 1;
    }
    output.push_str(rest);
    Ok(output)
}

trait DiagnosticsContext {
    fn with_source_opt(self, source: Option<&Path>) -> Self;
}

impl DiagnosticsContext for ConfigDiagnostics {
    fn with_source_opt(self, source: Option<&Path>) -> Self {
        match source {
            Some(path) => self.with_source(path),
            None => self,
        }
    }
}

fn override_string(name: &str, target: &mut String) {
    if let Ok(value) = env::var(name) {
        *target = value;
    }
}

fn override_u64(name: &str, target: &mut u64) -> Result<()> {
    if let Ok(value) = env::var(name) {
        *target = value
            .parse()
            .with_context(|| format!("environment variable {name} must be an integer"))?;
    }
    Ok(())
}
