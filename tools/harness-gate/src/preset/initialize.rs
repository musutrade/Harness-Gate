use super::catalog::{self, AUDIT_TEMPLATE, GITIGNORE_TEMPLATE, SECRETS_TEMPLATE};
use super::filesystem::{atomic_write, ensure_writable, resolve_inside};
use crate::config::{FlowConfig, DEFAULT_CONFIG_PATH};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub fn init(target: &Path, name: &str, force: bool) -> Result<()> {
    let preset = catalog::find(name)
        .ok_or_else(|| anyhow::anyhow!("unknown preset {name:?}; run `arc-flow presets`"))?;
    fs::create_dir_all(target)
        .with_context(|| format!("create project directory {}", target.display()))?;
    let root = target
        .canonicalize()
        .with_context(|| format!("resolve project directory {}", target.display()))?;
    let flow_path = resolve_inside(&root, PathBuf::from(DEFAULT_CONFIG_PATH))?;
    let audit_path = resolve_inside(&root, PathBuf::from(".harness-gate/audit.toml"))?;
    let secrets_path = resolve_inside(&root, PathBuf::from(".harness-gate/secrets.toml"))?;
    ensure_writable(&flow_path, force)?;
    ensure_writable(&audit_path, force)?;
    ensure_writable(&secrets_path, force)?;

    let mut config = FlowConfig::from_source(preset.flow)?;
    config.project.name = project_id(&root);
    config.validate()?;
    let directory = flow_path.parent().context("flow config has no parent")?;
    fs::create_dir_all(directory)?;
    atomic_write(&audit_path, AUDIT_TEMPLATE.as_bytes())?;
    atomic_write(&secrets_path, SECRETS_TEMPLATE.as_bytes())?;
    atomic_write(&flow_path, toml::to_string_pretty(&config)?.as_bytes())?;
    let gitignore = resolve_inside(&root, PathBuf::from(".harness-gate/.gitignore"))?;
    if !gitignore.exists() {
        atomic_write(&gitignore, GITIGNORE_TEMPLATE.as_bytes())?;
    }

    println!("Initialized preset {name:?} in {}", directory.display());
    println!(
        "Next: arc-flow --project-root {} config check",
        root.display()
    );
    Ok(())
}

pub(super) fn project_id(root: &Path) -> String {
    let name = root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("project");
    let mut id = String::new();
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            id.push(character.to_ascii_lowercase());
        } else if !id.ends_with('-') {
            id.push('-');
        }
    }
    let id = id.trim_matches('-');
    if id.is_empty() {
        "project".into()
    } else {
        id.into()
    }
}
