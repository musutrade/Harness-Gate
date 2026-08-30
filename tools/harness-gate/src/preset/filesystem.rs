use anyhow::{bail, Context, Result};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

pub(super) fn ensure_writable(path: &Path, force: bool) -> Result<()> {
    if path.exists() && !force {
        bail!(
            "{} already exists; pass --force to replace it",
            path.display()
        );
    }
    Ok(())
}

pub(super) fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("harness-gate");
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = parent.join(format!(
        ".{name}.harness-gate-{}-{unique}.tmp",
        std::process::id()
    ));

    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .with_context(|| format!("create temporary file {}", temporary.display()))?;
        file.write_all(content)?;
        file.sync_all()?;
        fs::rename(&temporary, path)
            .with_context(|| format!("replace {} atomically", path.display()))?;
        Ok(())
    })();
    if result.is_err() {
        fs::remove_file(&temporary).ok();
    }
    result
}

pub(super) fn resolve_inside(root: &Path, path: PathBuf) -> Result<PathBuf> {
    let path = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    if fs::symlink_metadata(&path).is_ok() {
        let resolved = path.canonicalize()?;
        if !resolved.starts_with(root) {
            bail!("path must remain inside the project: {}", path.display());
        }
        return Ok(path);
    }
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("path has no parent: {}", path.display()))?;
    let resolved_parent = if parent.exists() {
        parent.canonicalize()?
    } else {
        let mut existing = parent;
        while !existing.exists() {
            existing = existing
                .parent()
                .ok_or_else(|| anyhow::anyhow!("cannot resolve {}", path.display()))?;
        }
        existing.canonicalize()?
    };
    if !resolved_parent.starts_with(root) {
        bail!("path must remain inside the project: {}", path.display());
    }
    Ok(path)
}
