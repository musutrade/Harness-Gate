use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Write a report, creating its parent directory when necessary.
pub(crate) fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create report directory {}", parent.display()))?;
    }
    fs::write(path, contents).with_context(|| format!("write report {}", path.display()))
}

/// Serialize a report as stable, human-readable JSON and write it to disk.
pub(crate) fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    let contents = serde_json::to_string_pretty(value).context("serialize JSON report")?;
    write(path, contents)
}

/// Publish a complete file through a same-directory temporary and rename.
/// Callers opt in to replacing a legacy output; temporary files are removed
/// on every failed write.
pub(crate) fn atomic_write(
    path: &Path,
    contents: impl AsRef<[u8]>,
    replace_existing: bool,
) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("output has no parent directory"))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("create output directory {}", parent.display()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("output has an invalid filename"))?;
    let temporary = PathBuf::from(parent).join(format!(
        ".{name}.{}.tmp",
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .with_context(|| format!("create temporary output {}", temporary.display()))?;
        file.write_all(contents.as_ref())?;
        file.sync_all()?;
        drop(file);
        if replace_existing && fs::symlink_metadata(path).is_ok() {
            fs::remove_file(path)
                .with_context(|| format!("replace existing output {}", path.display()))?;
        }
        fs::rename(&temporary, path)
            .with_context(|| format!("publish output {}", path.display()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
