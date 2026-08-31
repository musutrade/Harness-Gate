use super::{resolve_repo_path, Project};
use crate::config::{resolve_config_path, FlowConfig, DEFAULT_CONFIG_PATH};
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

impl Project {
    pub fn discover(
        override_root: Option<PathBuf>,
        config_override: Option<PathBuf>,
    ) -> Result<Self> {
        let root = match override_root {
            Some(root) => canonical_directory(&root)?,
            None => {
                let start = env::current_dir().context("read current directory")?;
                find_root(&start, config_override.as_deref())?
            }
        };
        let config_path = resolve_config_path(&root, config_override)?;
        let config = FlowConfig::load_with_diagnostics(&config_path, Some(&root))
            .map_err(anyhow::Error::from)?;
        let scope_rules = config.compile_scope_rules()?;
        let reports = resolve_repo_path(
            &root,
            Path::new(&config.paths.reports),
            "report directory",
            false,
        )?;
        let audit_config = resolve_repo_path(
            &root,
            Path::new(&config.paths.audit_config),
            "audit configuration",
            true,
        )?;
        let secrets_config = resolve_repo_path(
            &root,
            Path::new(&config.paths.secrets_config),
            "secret scan configuration",
            true,
        )?;
        let aliases = config
            .paths
            .aliases
            .iter()
            .map(|(name, entry)| {
                resolve_repo_path(
                    &root,
                    Path::new(&entry.path),
                    &format!("path alias {name:?}"),
                    false,
                )
                .map(|path| (name.clone(), path))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;

        let project = Self {
            root,
            config_path,
            config,
            reports: reports.clone(),
            audit_config,
            secrets_config,
            resource_leases: reports.join("leases"),
            aliases,
            scope_rules,
        };
        project.validate()?;
        Ok(project)
    }

    fn validate(&self) -> Result<()> {
        if !self.audit_config.is_file() {
            bail!(
                "required audit configuration is missing: {}",
                self.audit_config.display()
            );
        }
        if !self.secrets_config.is_file() {
            bail!(
                "required secret scan configuration is missing: {}",
                self.secrets_config.display()
            );
        }
        Ok(())
    }
}

fn canonical_directory(path: &Path) -> Result<PathBuf> {
    let path = path
        .canonicalize()
        .with_context(|| format!("resolve project path {}", path.display()))?;
    if !path.is_dir() {
        bail!("project root is not a directory: {}", path.display());
    }
    Ok(path)
}

fn find_root(start: &Path, config_override: Option<&Path>) -> Result<PathBuf> {
    let start = canonical_directory(start)?;
    let configured_path = config_override.map(Path::to_path_buf);
    if let Some(config) = configured_path {
        let candidate = if config.is_absolute() {
            config
        } else {
            start.join(config)
        };
        let config = candidate
            .canonicalize()
            .with_context(|| format!("resolve workflow config {}", candidate.display()))?;
        if let Some(root) = config.ancestors().find(|path| path.join(".git").exists()) {
            return Ok(root.to_path_buf());
        }
    }
    start
        .ancestors()
        .find(|candidate| candidate.join(DEFAULT_CONFIG_PATH).is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "could not find project root above {}; expected {}; run `harness-gate init --preset <name>`",
                start.display(),
                DEFAULT_CONFIG_PATH
            )
        })
}
