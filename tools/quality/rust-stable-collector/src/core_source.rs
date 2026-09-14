//! Explicit, immutable-by-verification source view for the existing Core contract.
//! This is evidence export, not a new build root or a replacement project baseline.
use crate::{
    artifact::{self, FileIdentity},
    collect,
};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

const MANIFEST: &str = "source-workspace.json";

pub fn generated_path(entry: &Value) -> Result<String> {
    let logical = entry["logical_path"]
        .as_str()
        .context("generated logical path")?;
    relative(logical)?;
    Ok(format!(
        "generated/{}/{}.rs",
        artifact::digest(logical.as_bytes()),
        entry["sha256"].as_str().context("generated digest")?
    ))
}

fn relative(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && !name.contains(['\\', ':'])
            && name
                .split('/')
                .all(|p| !p.is_empty() && p != "." && p != "..")
            && Path::new(name)
                .components()
                .all(|p| matches!(p, Component::Normal(_))),
        "noncanonical source workspace path"
    );
    Ok(())
}

struct Plan {
    files: BTreeMap<String, (PathBuf, FileIdentity)>,
    manifest: Value,
}

fn plan(root: &Path, anchor: &str, request_digest: &str) -> Result<Plan> {
    let canonical = root.canonicalize()?;
    let root = canonical.as_path();
    let request: Value = crate::strict_json::parse(&fs::read(root.join("request.json"))?)?;
    let generated: Value =
        crate::strict_json::parse(&fs::read(root.join("generated-owners.json"))?)?;
    let project = Path::new(request["project_root"].as_str().context("project root")?);
    let sources: BTreeMap<String, FileIdentity> =
        serde_json::from_value(request["source_files"].clone())?;
    let mut files = BTreeMap::new();
    for (name, identity) in sources {
        relative(&name)?;
        files.insert(format!("project/{name}"), (project.join(&name), identity));
    }
    let mut origins = BTreeMap::new();
    for entry in generated["owners"].as_array().context("generated owners")? {
        let path = generated_path(entry)?;
        let identity = FileIdentity {
            sha256: entry["sha256"].as_str().context("generated digest")?.into(),
            bytes: entry["bytes"].as_u64().context("generated length")?,
        };
        let source = root.join("compiler-generated").join(&identity.sha256);
        ensure!(
            files.insert(path.clone(), (source, identity)).is_none(),
            "duplicate generated source path"
        );
        origins.insert(path, entry.clone());
    }
    let inventory: BTreeMap<_, _> = files.iter().map(|(name, (_, id))| (name, id)).collect();
    let manifest = json!({"schema":"rust-stable-core-source-workspace/v1-candidate",
        "capture":{"path":root,"manifest_sha256":anchor,"request_sha256":request_digest},
        "original_project_root":project,"files":inventory,"generated":origins});
    Ok(Plan { files, manifest })
}

/// Caller verifies the capture first. Recompute the entire view; the exported
/// manifest cannot authorize substitutions, omissions, or additional files.
pub fn verify(root: &Path, anchor: &str, request_digest: &str, workspace: &Path) -> Result<()> {
    ensure!(
        workspace.is_absolute()
            && workspace.canonicalize()? == workspace
            && fs::symlink_metadata(workspace)?.is_dir(),
        "source workspace must be canonical and real"
    );
    let plan = plan(root, anchor, request_digest)?;
    let mut expected: BTreeMap<_, _> = plan
        .files
        .into_iter()
        .map(|(name, (_, id))| (name, id))
        .collect();
    let bytes = serde_json::to_vec_pretty(&plan.manifest)?;
    expected.insert(
        MANIFEST.into(),
        FileIdentity {
            sha256: artifact::digest(&bytes),
            bytes: bytes.len() as u64,
        },
    );
    ensure!(
        artifact::inventory(workspace, false)? == expected,
        "source workspace differs from authenticated capture"
    );
    Ok(())
}

pub fn export(root: &Path, anchor: &str, request_digest: &str, output: &Path) -> Result<Value> {
    collect::verify(root, anchor, request_digest)?;
    let plan = plan(root, anchor, request_digest)?;
    let parent = output
        .parent()
        .context("source workspace parent")?
        .canonicalize()?;
    let output = parent.join(output.file_name().context("source workspace name")?);
    ensure!(
        !output.starts_with(root.canonicalize()?)
            && !output.starts_with(Path::new(
                plan.manifest["original_project_root"].as_str().unwrap()
            )),
        "source workspace must be outside project and capture"
    );
    // Never replace an existing directory or user file. Incomplete exports have
    // no completed manifest and fail verification; a retry uses a fresh path.
    fs::create_dir(&output).context("source workspace must be a new directory")?;
    for (name, (source, identity)) in &plan.files {
        let target = output.join(name);
        fs::create_dir_all(target.parent().context("source parent")?)?;
        fs::copy(source, &target)?;
        ensure!(
            artifact::identity(&target)? == *identity,
            "source changed during export"
        );
    }
    collect::verify(root, anchor, request_digest)?;
    artifact::write(&output.join(MANIFEST), &plan.manifest)?;
    verify(root, anchor, request_digest, &output)?;
    Ok(
        json!({"schema":"rust-stable-core-source-export/v1-candidate", "workspace_root":output,
        "manifest_sha256":artifact::identity(&output.join(MANIFEST))?.sha256}),
    )
}
