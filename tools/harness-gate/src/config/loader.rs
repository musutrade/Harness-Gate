use super::diagnostic::{
    audit_config_interpolation_diagnostic, interpolation_diagnostic, parse_diagnostic,
    ConfigDiagnostics, SourceMap,
};
use super::model::{FlowConfig, ParserConfig, ServiceConfig, StepConfig};
use anyhow::{Context, Result};
use schemars::schema_for;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

impl FlowConfig {
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
        reject_audit_config_interpolation(source, &source_map, source_path)?;
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

fn reject_audit_config_interpolation(
    source: &str,
    source_map: &SourceMap,
    source_path: Option<&Path>,
) -> std::result::Result<(), ConfigDiagnostics> {
    let Ok(raw) = toml::from_str::<toml::Value>(source) else {
        return Ok(());
    };
    let Some(value) = raw
        .get("paths")
        .and_then(toml::Value::as_table)
        .and_then(|paths| paths.get("audit_config"))
        .and_then(toml::Value::as_str)
    else {
        return Ok(());
    };
    if !value.contains("${") {
        return Ok(());
    }
    Err(audit_config_interpolation_diagnostic(
        source,
        source_map,
        source_path,
    ))
}

pub fn schema_json() -> Result<String> {
    serde_json::to_string_pretty(&schema_for!(FlowConfig)).context("serialize workflow schema")
}

fn interpolate_environment(
    source: &str,
    source_path: Option<&Path>,
) -> std::result::Result<String, ConfigDiagnostics> {
    let mut output = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut index = 0usize;
    let mut in_basic_string = false;
    let mut in_literal_string = false;
    let mut in_comment = false;

    while index < source.len() {
        let character = source[index..]
            .chars()
            .next()
            .expect("index remains on a UTF-8 boundary");
        let width = character.len_utf8();

        if in_comment {
            output.push_str(&source[index..index + width]);
            index += width;
            if character == '\n' {
                in_comment = false;
            }
            continue;
        }

        if in_basic_string {
            if character == '\\' {
                output.push_str(&source[index..index + width]);
                index += width;
                if index < source.len() {
                    let escaped = source[index..]
                        .chars()
                        .next()
                        .expect("index remains on a UTF-8 boundary");
                    output.push_str(&source[index..index + escaped.len_utf8()]);
                    index += escaped.len_utf8();
                }
                continue;
            }
            if character == '"' {
                in_basic_string = false;
            }
        } else if in_literal_string {
            if character == '\'' {
                in_literal_string = false;
            }
        } else if character == '#' {
            in_comment = true;
        } else if character == '"' {
            in_basic_string = true;
        } else if character == '\'' {
            in_literal_string = true;
        }

        if in_basic_string && bytes[index..].starts_with(b"${") {
            let end = source[index + 2..].find('}').ok_or_else(|| {
                interpolation_diagnostic(
                    source,
                    index..index + 2,
                    "environment interpolation is unterminated",
                    "close the expression with `}` or remove the incomplete `${...` token",
                    source_path,
                )
            })? + index
                + 2;
            let expression = &source[index + 2..end];
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
                    index..end + 1,
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
                        index..end + 1,
                        format!("environment variable {name} is not set and has no default"),
                        format!("set {name} or use `${{{name}:-default}}`"),
                        source_path,
                    )
                })?;
            output.push_str(&escape_toml_basic_string(&value));
            index = end + 1;
            continue;
        }

        output.push_str(&source[index..index + width]);
        index += width;
    }
    Ok(output)
}

fn escape_toml_basic_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                escaped.push_str(&format!("\\u{:04X}", character as u32));
            }
            character => escaped.push(character),
        }
    }
    escaped
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
