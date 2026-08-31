use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Publish a complete file through a same-directory temporary and rename.
/// Callers opt in to replacing a legacy output; temporary files are removed
/// on every failed write.
pub(crate) fn atomic_write(
    path: &Path,
    contents: impl AsRef<[u8]>,
    replace_existing: bool,
) -> Result<()> {
    let mut output = create_atomic_output(path, replace_existing)?;
    output.write_all(contents.as_ref())?;
    output.publish()
}

/// A same-directory output that owns its temporary file until publication.
/// Dropping an unpublished value removes the temporary entry, including when
/// a caller returns early because a child process or filesystem operation
/// failed. Keeping the file handle in the guard also gives Windows a chance to
/// close it before a failed cleanup attempts to unlink the path.
pub(crate) struct AtomicOutput {
    file: Option<fs::File>,
    temporary: PathBuf,
    target: PathBuf,
    replace_existing: bool,
    published: bool,
}

impl AtomicOutput {
    pub(crate) fn try_clone(&self) -> io::Result<fs::File> {
        self.file
            .as_ref()
            .ok_or_else(|| io::Error::other("atomic output is already published"))?
            .try_clone()
    }

    pub(crate) fn publish(mut self) -> Result<()> {
        if let Some(mut file) = self.file.take() {
            file.flush()?;
            file.sync_all()?;
            drop(file);
        }
        publish_atomic_output(&self.temporary, &self.target, self.replace_existing)?;
        self.published = true;
        Ok(())
    }
}

impl Write for AtomicOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("atomic output is already published"))?
            .write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("atomic output is already published"))?
            .flush()
    }
}

impl Drop for AtomicOutput {
    fn drop(&mut self) {
        if self.published {
            return;
        }
        // Drop the handle before unlinking so this is also best-effort safe on
        // platforms that do not permit deleting an open file.
        let _ = self.file.take();
        let _ = fs::remove_file(&self.temporary);
    }
}

/// Create a same-directory temporary file for a streaming output. The target
/// is checked before creation and again at publication, so callers can write
/// process output without exposing a partially written predictable path.
pub(crate) fn create_atomic_output(path: &Path, replace_existing: bool) -> Result<AtomicOutput> {
    let parent = output_parent(path)?;
    ensure_confined_parent(parent)?;
    validate_target(path, replace_existing)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("output has an invalid filename"))?;
    for _ in 0..64 {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(".{name}.{counter}.tmp"));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => {
                return Ok(AtomicOutput {
                    file: Some(file),
                    temporary,
                    target: path.to_path_buf(),
                    replace_existing,
                    published: false,
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("create temporary output {}", temporary.display()))
            }
        }
    }
    bail!(
        "could not allocate a collision-free temporary output for {}",
        path.display()
    )
}

/// Publish a completed temporary output by replacing only the destination
/// directory entry. This function never follows a symbolic-link target.
pub(crate) fn publish_atomic_output(
    temporary: &Path,
    target: &Path,
    replace_existing: bool,
) -> Result<()> {
    let parent = output_parent(target)?;
    ensure_confined_parent(parent)?;
    let temporary_parent = temporary
        .parent()
        .ok_or_else(|| anyhow::anyhow!("temporary output has no parent directory"))?;
    if temporary_parent != parent {
        bail!("temporary output is not a sibling of its destination");
    }
    let temporary_metadata = fs::symlink_metadata(temporary)
        .with_context(|| format!("inspect temporary output {}", temporary.display()))?;
    if temporary_metadata.file_type().is_symlink() || !temporary_metadata.is_file() {
        bail!(
            "temporary output is not a regular file: {}",
            temporary.display()
        );
    }
    validate_target(target, replace_existing)?;
    #[cfg(windows)]
    if replace_existing && fs::symlink_metadata(target).is_ok() {
        fs::remove_file(target)
            .with_context(|| format!("replace existing output {}", target.display()))?;
    }
    fs::rename(temporary, target)
        .with_context(|| format!("publish output {}", target.display()))?;
    sync_directory(parent)
}

fn output_parent(path: &Path) -> Result<&Path> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("output has no parent directory"))?;
    Ok(if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    })
}

fn validate_target(path: &Path, replace_existing: bool) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
                bail!("output target is not a regular file: {}", path.display());
            }
            if !replace_existing {
                bail!("output already exists: {}", path.display());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("inspect output {}", path.display()))
        }
    }
    Ok(())
}

/// Publish a repository-adjacent output below an explicit allowed root. Both
/// existing and newly-created parent components are checked with
/// `symlink_metadata`; callers never follow a repository-controlled symlink.
pub(crate) fn confined_atomic_write(
    root: &Path,
    relative: &Path,
    contents: impl AsRef<[u8]>,
    replace_existing: bool,
) -> Result<PathBuf> {
    ensure_confined_parent(root)?;
    let root = root
        .canonicalize()
        .with_context(|| format!("resolve output root {}", root.display()))?;
    if !root.is_dir() {
        bail!("output root is not a directory: {}", root.display());
    }
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        bail!("output path must be relative and confined below the root");
    }
    let target = root.join(relative);
    let parent = target
        .parent()
        .ok_or_else(|| anyhow::anyhow!("confined output has no parent directory"))?;
    ensure_confined_parent_below(&root, parent)?;
    atomic_write(&target, contents, replace_existing)?;
    Ok(target)
}

pub(crate) fn confined_write_json<T: Serialize + ?Sized>(
    root: &Path,
    relative: &Path,
    value: &T,
    replace_existing: bool,
) -> Result<PathBuf> {
    let contents = serde_json::to_vec_pretty(value).context("serialize JSON report")?;
    confined_atomic_write(root, relative, contents, replace_existing)
}

fn ensure_confined_parent(parent: &Path) -> Result<()> {
    let mut components = parent.components();
    let mut current = PathBuf::new();
    if let Some(prefix) = components.next() {
        current.push(prefix.as_os_str());
    }
    for component in components {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                bail!("output parent is a symbolic link: {}", current.display())
            }
            Ok(metadata) if !metadata.is_dir() => {
                bail!("output parent is not a directory: {}", current.display())
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current)
                    .with_context(|| format!("create output directory {}", current.display()))?;
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("inspect output directory {}", current.display()));
            }
        }
    }
    Ok(())
}

fn ensure_confined_parent_below(root: &Path, parent: &Path) -> Result<()> {
    if !parent.starts_with(root) {
        bail!("output parent escapes the allowed root");
    }
    ensure_confined_parent(parent)?;
    let resolved = parent
        .canonicalize()
        .with_context(|| format!("resolve output parent {}", parent.display()))?;
    if !resolved.starts_with(root) {
        bail!("output parent escapes the allowed root");
    }
    Ok(())
}

fn sync_directory(directory: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        fs::File::open(directory)
            .with_context(|| format!("open output directory {}", directory.display()))?
            .sync_all()
            .with_context(|| format!("sync output directory {}", directory.display()))?;
    }
    #[cfg(not(unix))]
    let _ = directory;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{atomic_write, confined_atomic_write};
    use tempfile::tempdir;

    #[test]
    fn confined_writer_publishes_complete_content() {
        let directory = tempdir().expect("tempdir");
        let target = confined_atomic_write(
            directory.path(),
            std::path::Path::new("nested/result.json"),
            b"complete",
            false,
        )
        .expect("publish");
        assert_eq!(std::fs::read_to_string(target).expect("read"), "complete");
    }

    #[cfg(unix)]
    #[test]
    fn confined_writer_rejects_parent_and_target_symlinks() {
        use std::os::unix::fs::symlink;
        let directory = tempdir().expect("tempdir");
        let outside = tempdir().expect("outside");
        symlink(outside.path(), directory.path().join("linked")).expect("parent symlink");
        assert!(confined_atomic_write(
            directory.path(),
            std::path::Path::new("linked/out.txt"),
            b"blocked",
            false,
        )
        .is_err());
        let target = directory.path().join("target.txt");
        std::fs::write(&target, b"original").expect("target");
        symlink(&target, directory.path().join("alias.txt")).expect("target symlink");
        assert!(confined_atomic_write(
            directory.path(),
            std::path::Path::new("alias.txt"),
            b"blocked",
            true,
        )
        .is_err());
        assert_eq!(std::fs::read(&target).expect("read target"), b"original");
    }

    #[test]
    fn atomic_write_honors_no_replace() {
        let directory = tempdir().expect("tempdir");
        let path = directory.path().join("result");
        atomic_write(&path, b"one", false).expect("first");
        assert!(atomic_write(&path, b"two", false).is_err());
        assert_eq!(std::fs::read(&path).expect("read"), b"one");
    }
}
