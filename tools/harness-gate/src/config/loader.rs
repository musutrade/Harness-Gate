use super::model::{FlowConfig, ParserConfig, ServiceConfig, StepConfig};
use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

impl FlowConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let source = fs::read_to_string(path)
            .with_context(|| format!("read workflow config {}", path.display()))?;
        let mut config: Self = toml::from_str(&source)
            .with_context(|| format!("parse workflow config {}", path.display()))?;
        config.apply_environment()?;
        config.validate()?;
        Ok(config)
    }

    pub fn from_source(source: &str) -> Result<Self> {
        let config: Self = toml::from_str(source).context("parse workflow config")?;
        config.validate()?;
        Ok(config)
    }

    pub fn components(&self) -> BTreeSet<String> {
        self.steps
            .iter()
            .map(|step| step.component.clone())
            .collect()
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
