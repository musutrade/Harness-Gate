//! Authenticate explicitly supplied registry archives against the project lock.
//! Archive locations are inputs, never inferred from Cargo's private cache layout.
use crate::artifact::{self, FileIdentity};
use anyhow::{ensure, Context, Result};
use flate2::read::GzDecoder;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

pub type Archives = BTreeMap<String, PathBuf>;
const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";
const FILE_LIMIT: u64 = 16 * 1024 * 1024;
const CRATE_LIMIT: u64 = 128 * 1024 * 1024;

pub fn check_archives(archives: &Archives) -> Result<()> {
    for (checksum, path) in archives {
        ensure!(
            path.is_absolute() && path.canonicalize()? == *path,
            "archive path must be canonical and absolute"
        );
        ensure!(
            artifact::identity(path)?.sha256 == *checksum,
            "registry archive checksum mismatch: {}",
            path.display()
        );
    }
    Ok(())
}

fn authenticated_source(
    archive: &Path,
    root: &Path,
    prefix: &str,
) -> Result<BTreeMap<String, FileIdentity>> {
    // Stream without extraction, bounding both each file and expanded tar input.
    let decoder = GzDecoder::new(fs::File::open(archive)?);
    let mut tar = tar::Archive::new(decoder.take(CRATE_LIMIT + 1));
    let mut files = BTreeMap::new();
    let mut total = 0;
    for entry in tar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        ensure!(
            path.components().all(|c| matches!(c, Component::Normal(_))),
            "unsafe crate member path"
        );
        let relative = path
            .strip_prefix(prefix)
            .context("crate member prefix mismatch")?;
        if entry.header().entry_type().is_dir() {
            continue;
        }
        ensure!(
            entry.header().entry_type().is_file() && !relative.as_os_str().is_empty(),
            "nonregular crate member"
        );
        let name = relative.to_str().context("non UTF-8 crate member")?;
        ensure!(
            !matches!(name, ".cargo-ok" | ".cargo-checksum.json"),
            "reserved Cargo marker in archive"
        );
        let bytes = entry.size();
        total += bytes.min(CRATE_LIMIT + 1);
        ensure!(
            bytes <= FILE_LIMIT && total <= CRATE_LIMIT,
            "crate expands beyond input bound"
        );
        let mut content = Vec::new();
        entry.read_to_end(&mut content)?;
        let identity = FileIdentity {
            sha256: artifact::digest(&content),
            bytes,
        };
        ensure!(
            files.insert(name.into(), identity.clone()).is_none(),
            "duplicate crate member"
        );
        ensure!(
            artifact::identity(&root.join(relative))? == identity,
            "cached source differs from locked archive: {name}"
        );
    }
    // Finish the gzip stream to check its checksum, trailer and total bound.
    let mut decoder = tar.into_inner();
    std::io::copy(&mut decoder, &mut std::io::sink())?;
    ensure!(decoder.limit() > 0, "crate expands beyond input bound");
    let mut actual = artifact::inventory(root, false)?;
    actual.remove(".cargo-ok");
    actual.remove(".cargo-checksum.json");
    ensure!(
        !files.is_empty() && actual == files,
        "undeclared cached crate files"
    );
    Ok(files)
}

pub fn snapshot(metadata: &Value, root: &Path, archives: &Archives) -> Result<Value> {
    check_archives(archives)?;
    ensure!(
        metadata["version"] == 1 && metadata["workspace_root"].as_str() == root.to_str(),
        "project must be the Cargo workspace root with metadata format 1"
    );
    let lock: toml::Value = toml::from_str(&fs::read_to_string(root.join("Cargo.lock"))?)?;
    ensure!(
        lock.get("version").and_then(toml::Value::as_integer) == Some(4),
        "unsupported Cargo lock format"
    );
    let mut locked = BTreeMap::new();
    for package in lock
        .get("package")
        .and_then(toml::Value::as_array)
        .context("lock packages missing")?
    {
        let key = (
            package
                .get("name")
                .and_then(toml::Value::as_str)
                .context("lock name missing")?
                .to_owned(),
            package
                .get("version")
                .and_then(toml::Value::as_str)
                .context("lock version missing")?
                .to_owned(),
            package
                .get("source")
                .and_then(toml::Value::as_str)
                .map(str::to_owned),
        );
        ensure!(
            locked
                .insert(key, package.get("checksum").and_then(toml::Value::as_str))
                .is_none(),
            "duplicate lock package"
        );
    }
    let mut seen = BTreeSet::new();
    let mut used_archives = BTreeSet::new();
    let mut result = BTreeMap::new();
    for package in metadata["packages"]
        .as_array()
        .context("Cargo packages missing")?
    {
        let name = package["name"].as_str().context("package name missing")?;
        let version = package["version"]
            .as_str()
            .context("package version missing")?;
        let source = package["source"].as_str();
        ensure!(
            package["source"].is_null() || source.is_some(),
            "invalid Cargo source"
        );
        let key = (
            name.to_owned(),
            version.to_owned(),
            source.map(str::to_owned),
        );
        let checksum = locked
            .get(&key)
            .context("metadata package absent from lock")?;
        ensure!(seen.insert(key), "duplicate metadata package");
        let manifest = Path::new(
            package["manifest_path"]
                .as_str()
                .context("manifest path missing")?,
        );
        ensure!(
            manifest.is_absolute() && manifest.canonicalize()? == manifest,
            "manifest must be canonical"
        );
        let package_root = manifest.parent().context("package root missing")?;
        let proof = if let Some(source) = source {
            ensure!(
                source == REGISTRY,
                "registry source not supported: {source}"
            );
            let checksum = checksum.context("registry lock checksum missing")?;
            let archive = archives.get(checksum).context("locked registry archive missing: supply --registry-archives JSON mapping Cargo.lock checksums to absolute .crate paths")?;
            let files = authenticated_source(archive, package_root, &format!("{name}-{version}"))?;
            used_archives.insert(checksum.to_owned());
            json!({"kind":"locked-registry-archive", "archive":archive, "checksum":checksum, "source_root":package_root, "files":files})
        } else {
            ensure!(
                manifest.starts_with(root) && checksum.is_none(),
                "dependency outside measured workspace is unsupported"
            );
            json!({"kind":"workspace-source-inventory", "manifest":manifest})
        };
        for target in package["targets"]
            .as_array()
            .context("Cargo targets missing")?
        {
            let path = Path::new(
                target["src_path"]
                    .as_str()
                    .context("target source missing")?,
            );
            ensure!(
                path.canonicalize()?.starts_with(if source.is_some() {
                    package_root
                } else {
                    root
                }),
                "target source outside authenticated package"
            );
            if source.is_some() {
                let kinds = target["kind"].as_array().context("target kinds missing")?;
                ensure!(
                    !kinds
                        .iter()
                        .any(|k| k == "custom-build" || k == "proc-macro"),
                    "registry build scripts/proc macros unsupported"
                );
            }
        }
        let id = package["id"].as_str().context("package ID missing")?;
        ensure!(
            result.insert(id, proof).is_none(),
            "duplicate Cargo package ID"
        );
    }
    // The bounded candidate rejects inactive/unresolved lock entries rather than
    // silently claiming provenance for a partial dependency inventory.
    ensure!(
        !seen.is_empty() && seen == locked.keys().cloned().collect(),
        "lock/metadata package inventory mismatch: inactive dependencies unsupported"
    );
    ensure!(
        used_archives == archives.keys().cloned().collect(),
        "unused registry archive input"
    );
    Ok(json!({"schema":"rust-stable-dependencies/v1-candidate", "packages":result}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::GzEncoder, Compression};
    use std::io::Write;

    fn archive(root: &Path, duplicate: bool, symlink: bool) -> PathBuf {
        let path = root.join("input.crate");
        let mut builder = tar::Builder::new(GzEncoder::new(
            fs::File::create(&path).unwrap(),
            Compression::fast(),
        ));
        for _ in 0..if duplicate { 2 } else { 1 } {
            let mut header = tar::Header::new_gnu();
            header.set_size(3);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, "probe-1.0.0/src.rs", &b"abc"[..])
                .unwrap();
        }
        if symlink {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_link_name("/outside").unwrap();
            header.set_cksum();
            builder
                .append_data(&mut header, "probe-1.0.0/link", std::io::empty())
                .unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap();
        path
    }

    #[test]
    fn locked_archive_does_not_authorize_modified_cache_or_extra_files() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("src.rs"), "abc").unwrap();
        let archive = archive(temp.path(), false, false);
        assert!(authenticated_source(&archive, &source, "probe-1.0.0").is_ok());
        fs::write(source.join("src.rs"), "xyz").unwrap();
        assert!(authenticated_source(&archive, &source, "probe-1.0.0")
            .unwrap_err()
            .to_string()
            .contains("cached source differs"));
        fs::write(source.join("src.rs"), "abc").unwrap();
        fs::write(source.join("extra.rs"), "abc").unwrap();
        assert!(authenticated_source(&archive, &source, "probe-1.0.0")
            .unwrap_err()
            .to_string()
            .contains("undeclared"));
        let archives = BTreeMap::from([(
            artifact::identity(&archive).unwrap().sha256,
            archive.clone(),
        )]);
        assert!(check_archives(&archives).is_ok());
        fs::OpenOptions::new()
            .append(true)
            .open(archive)
            .unwrap()
            .write_all(b"tampered")
            .unwrap();
        assert!(check_archives(&archives)
            .unwrap_err()
            .to_string()
            .contains("checksum mismatch"));
    }

    #[test]
    fn archive_rejects_duplicate_members_and_links() {
        for (duplicate, symlink, message) in
            [(true, false, "duplicate"), (false, true, "nonregular")]
        {
            let temp = tempfile::tempdir().unwrap();
            let source = temp.path().join("source");
            fs::create_dir(&source).unwrap();
            fs::write(source.join("src.rs"), "abc").unwrap();
            let archive = archive(temp.path(), duplicate, symlink);
            assert!(authenticated_source(&archive, &source, "probe-1.0.0")
                .unwrap_err()
                .to_string()
                .contains(message));
        }
    }

    #[test]
    fn incomplete_lock_fails_without_panicking() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("Cargo.lock"), "version = 4").unwrap();
        assert!(snapshot(
            &json!({"version":1,"workspace_root":root.path()}),
            root.path(),
            &Archives::new()
        )
        .is_err());
    }
}
