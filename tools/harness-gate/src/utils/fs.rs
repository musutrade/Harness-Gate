use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::path::Path;

/// Write a report, creating its parent directory when necessary.
pub fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create report directory {}", parent.display()))?;
    }
    fs::write(path, contents).with_context(|| format!("write report {}", path.display()))
}

/// Serialize a report as stable, human-readable JSON and write it to disk.
pub fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    let contents = serde_json::to_string_pretty(value).context("serialize JSON report")?;
    write(path, contents)
}
