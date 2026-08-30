use anyhow::{bail, Context, Result};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

pub(super) fn ensure_writable(path: &Path, force: bool) -> Result<()> {
    if fs::symlink_metadata(path).is_ok() && !force {
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

/// Stage a group of files before replacing any destination. Existing files are
/// moved aside during the commit and restored if a later rename fails.
pub(super) fn atomic_write_batch(entries: &[(&Path, &[u8])]) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut staged = Vec::with_capacity(entries.len());
    for (index, (path, content)) in entries.iter().enumerate() {
        let result = (|| -> Result<PathBuf> {
            let parent = path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("path has no parent: {}", path.display()))?;
            fs::create_dir_all(parent)?;
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("harness-gate");
            let temporary = parent.join(format!(
                ".{name}.harness-gate-batch-{}-{unique}-{index}.tmp",
                std::process::id()
            ));
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .with_context(|| format!("create temporary file {}", temporary.display()))?;
            let write_result = file.write_all(content).and_then(|()| file.sync_all());
            if let Err(error) = write_result {
                fs::remove_file(&temporary).ok();
                return Err(error.into());
            }
            Ok(temporary)
        })();
        match result {
            Ok(temporary) => staged.push((*path, temporary)),
            Err(error) => {
                for (_, temporary) in &staged {
                    fs::remove_file(temporary).ok();
                }
                return Err(error);
            }
        }
    }

    let mut backups = Vec::new();
    let mut committed = Vec::new();
    let commit = (|| -> Result<()> {
        for (index, (path, _)) in staged.iter().enumerate() {
            if fs::symlink_metadata(path).is_ok() {
                let backup = path.with_file_name(format!(
                    ".{}.harness-gate-backup-{}-{index}",
                    path.file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("file"),
                    unique
                ));
                fs::rename(path, &backup)
                    .with_context(|| format!("backup existing file {}", path.display()))?;
                backups.push((*path, backup));
            }
        }
        for (path, temporary) in &staged {
            fs::rename(temporary, path)
                .with_context(|| format!("replace {} atomically", path.display()))?;
            committed.push(*path);
        }
        Ok(())
    })();

    if commit.is_ok() {
        for (_, backup) in backups {
            fs::remove_file(backup).ok();
        }
        return Ok(());
    }

    for path in committed {
        fs::remove_file(path).ok();
    }
    for (path, backup) in backups.into_iter().rev() {
        fs::rename(backup, path).ok();
    }
    for (_, temporary) in staged {
        fs::remove_file(temporary).ok();
    }
    commit
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
