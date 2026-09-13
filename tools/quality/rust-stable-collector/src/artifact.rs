use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Read, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileIdentity {
    pub sha256: String,
    pub bytes: u64,
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn identity(path: &Path) -> Result<FileIdentity> {
    ensure!(
        fs::symlink_metadata(path)?.is_file(),
        "not a regular file: {}",
        path.display()
    );
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut bytes = 0;
    let mut buffer = [0; 65536];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        bytes += read as u64;
        hasher.update(&buffer[..read]);
    }
    Ok(FileIdentity {
        sha256: format!("{:x}", hasher.finalize()),
        bytes,
    })
}

pub fn write(path: &Path, value: &impl Serialize) -> Result<()> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.sync_all()?;
    Ok(())
}

/// No symlinks, special files or silently omitted source. Build and VCS state
/// are excluded only at the project root; the output must be outside this root.
pub fn inventory(root: &Path, project: bool) -> Result<BTreeMap<String, FileIdentity>> {
    fn visit(
        root: &Path,
        dir: &Path,
        project: bool,
        out: &mut BTreeMap<String, FileIdentity>,
    ) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = path
                .strip_prefix(root)?
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("non UTF-8 path"))?
                .to_owned();
            if project && matches!(name.as_str(), "target" | ".git") {
                continue;
            }
            let kind = entry.file_type()?;
            if kind.is_dir() {
                visit(root, &path, project, out)?;
            } else if kind.is_file() {
                out.insert(name, identity(&path)?);
            } else {
                bail!("symlink or special file: {}", path.display());
            }
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    visit(root, root, project, &mut files)?;
    ensure!(!files.is_empty(), "empty inventory");
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inventories_reject_symlinks_and_detect_changes() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("source.rs"), "a").unwrap();
        let first = inventory(root.path(), true).unwrap();
        fs::write(root.path().join("source.rs"), "b").unwrap();
        assert_ne!(first, inventory(root.path(), true).unwrap());
        std::os::unix::fs::symlink("source.rs", root.path().join("alias")).unwrap();
        assert!(inventory(root.path(), true).is_err());
    }
}
