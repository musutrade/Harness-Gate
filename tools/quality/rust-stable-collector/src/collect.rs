//! Candidate capture, deliberately not a Core adapter or signed release format.
use crate::{
    artifact::{self, FileIdentity},
    compiler_inputs, coverage, dependencies, ownership,
    process::Runner,
    source,
    tools::{self, Tools},
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

const REQUEST: &str = "rust-stable-candidate-request/v1";
const MANIFEST: &str = "rust-stable-candidate-manifest/v1";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: String,
    project_root: PathBuf,
    output_root: PathBuf,
    source_files: BTreeMap<String, FileIdentity>,
    config_files: BTreeMap<String, FileIdentity>,
    tools: Tools,
    features: Vec<String>,
    timeout_seconds: u64,
    #[serde(default)]
    registry_archives: dependencies::Archives,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    request_sha256: String,
    files: BTreeMap<String, FileIdentity>,
}

// Cargo configuration outside the source tree also affects compilation. Record
// exact files without exposing their potentially private contents in evidence.
pub fn config_files(project: &Path) -> Result<BTreeMap<String, FileIdentity>> {
    let mut directories: Vec<PathBuf> = project.ancestors().map(|p| p.join(".cargo")).collect();
    if let Some(home) = env::var_os("CARGO_HOME") {
        directories.push(PathBuf::from(home));
    } else if let Some(home) = env::var_os("HOME") {
        directories.push(PathBuf::from(home).join(".cargo"));
    }
    let mut files = BTreeMap::new();
    for directory in directories {
        for name in ["config", "config.toml"] {
            let path = directory.join(name);
            if path.try_exists()? {
                validate_cargo_config(&fs::read_to_string(&path)?).with_context(|| {
                    format!("unsupported Cargo configuration: {}", path.display())
                })?;
                files.insert(
                    path.to_str()
                        .context("non UTF-8 Cargo configuration path")?
                        .into(),
                    artifact::identity(&path)?,
                );
            }
        }
    }
    Ok(files)
}

fn validate_cargo_config(text: &str) -> Result<()> {
    let config: toml::Table = toml::from_str(text)?;
    // This first candidate permits only a target-directory preference, which
    // the isolated capture overrides. Unknown flags, wrappers, aliases, env and
    // unstable sections fail before Cargo can execute them.
    for (key, value) in config {
        ensure!(key == "build", "Cargo section {key} is not yet supported");
        for (key, value) in value.as_table().context("Cargo build table expected")? {
            ensure!(
                key == "target-dir" && value.is_str(),
                "Cargo build.{key} is not yet supported"
            );
        }
    }
    Ok(())
}

pub fn prepare(
    project: &Path,
    output: &Path,
    doctor: &Path,
    archives: Option<&Path>,
) -> Result<Value> {
    let registry_archives = match archives {
        Some(path) => serde_json::from_slice(&fs::read(path)?)?,
        None => dependencies::Archives::new(),
    };
    dependencies::check_archives(&registry_archives)?;
    let project_root = project.canonicalize()?;
    let output_root = output
        .parent()
        .context("output parent required")?
        .canonicalize()?
        .join(output.file_name().context("output filename required")?);
    let tools: Tools = serde_json::from_slice(&fs::read(doctor)?)?;
    Ok(serde_json::to_value(Request {
        schema: REQUEST.into(),
        source_files: artifact::inventory(&project_root, true)?,
        config_files: config_files(&project_root)?,
        project_root,
        output_root,
        tools,
        features: vec![],
        timeout_seconds: 300,
        registry_archives,
    })?)
}

fn validate_request(request: &Request) -> Result<()> {
    ensure!(request.schema == REQUEST, "unsupported request schema");
    dependencies::check_archives(&request.registry_archives)?;
    ensure!(
        request.project_root.is_absolute()
            && request.project_root.canonicalize()? == request.project_root,
        "project must be canonical and absolute"
    );
    ensure!(request.output_root.is_absolute(), "output must be absolute");
    ensure!(
        !request.output_root.starts_with(&request.project_root),
        "output must be outside project"
    );
    ensure!(
        request.source_files == artifact::inventory(&request.project_root, true)?,
        "source identity changed"
    );
    ensure!(
        request.config_files == config_files(&request.project_root)?,
        "Cargo configuration identity changed"
    );
    for feature in &request.features {
        ensure!(
            !feature.is_empty()
                && feature
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '/')),
            "unsupported feature name"
        );
    }
    Ok(())
}

fn recheck_tools(tools: &Tools) -> Result<()> {
    for tool in [
        &tools.rustc,
        &tools.cargo,
        &tools.rustdoc,
        &tools.cargo_llvm_cov,
        &tools.llvm_cov,
        &tools.llvm_profdata,
    ] {
        ensure!(
            artifact::identity(&tool.path)? == tool.identity,
            "tool identity changed: {}",
            tool.path.display()
        );
    }
    Ok(())
}

fn analyze_sources(request: &Request) -> Result<Value> {
    let mut sources = BTreeMap::new();
    let mut exclusions = Vec::new();
    for name in request.source_files.keys().filter(|n| n.ends_with(".rs")) {
        if name.starts_with("tests/")
            || name.starts_with("benches/")
            || name.starts_with("examples/")
            || name == "build.rs"
        {
            exclusions.push(name);
            continue;
        }
        sources.insert(
            name,
            source::analyze(&fs::read_to_string(request.project_root.join(name))?)
                .with_context(|| format!("parse source {name}"))?,
        );
    }
    ensure!(!sources.is_empty(), "no analyzable Rust sources");
    Ok(
        json!({"series":source::SERIES,"scope":"lexical source, no expansion or reachability claim","files":sources,"excluded_paths":exclusions}),
    )
}

pub fn collect(path: &Path) -> Result<String> {
    let request_bytes = fs::read(path)?;
    let request: Request = serde_json::from_slice(&request_bytes)?;
    validate_request(&request)?;
    let mut runner = Runner::create(
        &request.output_root,
        &request.project_root,
        request.timeout_seconds,
    )?;
    ensure!(
        tools::discover(&mut runner)? == request.tools,
        "tool selection or identity changed since doctor"
    );
    fs::write(runner.root.join("request.json"), &request_bytes)?;
    artifact::write(&runner.root.join("tools.json"), &request.tools)?;
    let scratch = tempfile::Builder::new()
        .prefix("build-")
        .tempdir_in(&runner.root)?;
    let toolset = &request.tools;
    let mut extra = BTreeMap::from([
        (
            "CARGO_TARGET_DIR".into(),
            scratch.path().display().to_string(),
        ),
        (
            "CARGO_LLVM_COV_TARGET_DIR".into(),
            scratch.path().display().to_string(),
        ),
        (
            "LLVM_COV".into(),
            toolset.llvm_cov.path.display().to_string(),
        ),
        (
            "LLVM_PROFDATA".into(),
            toolset.llvm_profdata.path.display().to_string(),
        ),
        ("RUSTC".into(), toolset.rustc.path.display().to_string()),
        ("RUSTDOC".into(), toolset.rustdoc.path.display().to_string()),
        ("CARGO".into(), toolset.cargo.path.display().to_string()),
    ]);
    let path = env::join_paths(
        std::iter::once(
            toolset
                .rustc
                .path
                .parent()
                .context("rustc directory missing")?
                .to_path_buf(),
        )
        .chain(env::split_paths(&env::var_os("PATH").unwrap_or_default())),
    )?;
    extra.insert(
        "PATH".into(),
        path.to_str().context("non UTF-8 PATH")?.into(),
    );
    let mut metadata_args: Vec<String> =
        ["metadata", "--format-version", "1", "--locked", "--offline"]
            .iter()
            .map(|s| (*s).into())
            .collect();
    if !request.features.is_empty() {
        metadata_args.extend(["--features".into(), request.features.join(",")]);
    }
    let metadata: Value =
        serde_json::from_str(&runner.run(&toolset.cargo.path, &metadata_args, &extra)?)?;
    let dependencies =
        dependencies::snapshot(&metadata, &request.project_root, &request.registry_archives)?;
    artifact::write(&runner.root.join("dependencies.json"), &dependencies)?;
    artifact::write(&runner.root.join("cargo-metadata.json"), &metadata)?;
    fs::create_dir(runner.root.join("compiler-invocations"))?;
    extra.insert(
        "RUSTC_WRAPPER".into(),
        env::current_exe()?.display().to_string(),
    );
    extra.insert(
        crate::compiler_wrapper::CONTEXT.into(),
        runner.root.display().to_string(),
    );
    let coverage_path = runner.root.join("coverage.json");
    let mut args: Vec<String> = ["llvm-cov", "--verbose", "--json", "--output-path"]
        .iter()
        .map(|s| (*s).into())
        .collect();
    args.push(coverage_path.display().to_string());
    args.extend(
        [
            "--locked",
            "--offline",
            "--workspace",
            "--tests",
            "--target",
            &toolset.host,
        ]
        .iter()
        .map(|s| (*s).into()),
    );
    if !request.features.is_empty() {
        args.extend(["--features".into(), request.features.join(",")]);
    }
    runner.run(&toolset.cargo_llvm_cov.path, &args, &extra)?;
    compiler_inputs::capture(
        &runner.root,
        scratch.path(),
        &request.project_root,
        &request.source_files,
        &dependencies,
        &request.tools.rustc.path,
    )?;
    let coverage: Value = coverage::parse(&fs::read(&coverage_path)?)?;
    coverage::validate(&coverage)?;
    let analysis = analyze_sources(&request)?;
    for name in analysis["files"]
        .as_object()
        .context("source files")?
        .keys()
    {
        ownership::file(&request.project_root, name, &coverage)?;
    }
    artifact::write(&runner.root.join("source-analysis.json"), &analysis)?;
    artifact::write(
        &runner.root.join("candidate.json"),
        &json!({
            "schema":"rust-stable-candidate/v1", "state":"unsupported",
            "coverage":{"series":"rust-llvm-source-coverage/v1-candidate", "format":coverage["version"], "scope":"Cargo workspace tests; test code included; build scripts not instrumented", "artifact":"coverage.json"},
            "complexity":{"series":source::SERIES, "artifact":"source-analysis.json"},
            "function_crap":{"state":"unsupported", "reason":"CRAP model and measurement migration not accepted; region coverage is a separate candidate metric"},
            "core_acceptance":{"state":"unsupported", "reason":"candidate capture is not a signed Core protocol v2 adapter"},
        }),
    )?;
    // No persistent target, profile or external runtime archives in the capture.
    scratch.close()?;
    ensure!(
        dependencies
            == dependencies::snapshot(
                &metadata,
                &request.project_root,
                &request.registry_archives
            )?,
        "dependency identity changed during capture"
    );
    validate_request(&request)?;
    recheck_tools(&request.tools)?;
    compiler_inputs::verify(
        &runner.root,
        &request.project_root,
        &request.source_files,
        &dependencies,
        &request.tools.rustc.path,
    )?;
    let manifest = Manifest {
        schema: MANIFEST.into(),
        request_sha256: artifact::digest(&request_bytes),
        files: artifact::inventory(&runner.root, false)?,
    };
    let manifest_path = runner.root.join("manifest.json");
    artifact::write(&manifest_path, &manifest)?;
    Ok(serde_json::to_string(
        &json!({"state":"unsupported", "capture":"complete", "manifest_sha256":artifact::identity(&manifest_path)?.sha256, "request_sha256":manifest.request_sha256, "core_acceptance":"pending"}),
    )?)
}

pub fn verify(root: &Path, anchor: &str, request_digest: &str) -> Result<()> {
    let manifest_bytes = fs::read(root.join("manifest.json"))?;
    ensure!(
        artifact::digest(&manifest_bytes) == anchor,
        "manifest anchor mismatch"
    );
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)?;
    ensure!(manifest.schema == MANIFEST, "unsupported manifest schema");
    ensure!(
        manifest.request_sha256 == request_digest,
        "request anchor mismatch"
    );
    let mut files = artifact::inventory(root, false)?;
    files.remove("manifest.json");
    ensure!(files == manifest.files, "artifact identity/set mismatch");
    let request_bytes = fs::read(root.join("request.json"))?;
    ensure!(
        artifact::digest(&request_bytes) == request_digest,
        "request identity mismatch"
    );
    let request: Request = serde_json::from_slice(&request_bytes)?;
    ensure!(
        request.output_root.canonicalize()? == root.canonicalize()?,
        "capture root identity mismatch"
    );
    validate_request(&request)?;
    recheck_tools(&request.tools)?;
    let metadata: Value = crate::strict_json::parse(&fs::read(root.join("cargo-metadata.json"))?)?;
    let recorded: Value = crate::strict_json::parse(&fs::read(root.join("dependencies.json"))?)?;
    ensure!(
        recorded
            == dependencies::snapshot(
                &metadata,
                &request.project_root,
                &request.registry_archives
            )?,
        "dependency provenance differs from recomputed facts"
    );
    compiler_inputs::verify(
        root,
        &request.project_root,
        &request.source_files,
        &recorded,
        &request.tools.rustc.path,
    )?;
    let coverage: Value = coverage::parse(&fs::read(root.join("coverage.json"))?)?;
    coverage::validate(&coverage)?;
    let analysis: Value = crate::strict_json::parse(&fs::read(root.join("source-analysis.json"))?)?;
    ensure!(
        analysis == analyze_sources(&request)?,
        "source analysis differs from recomputed facts"
    );
    for path in analysis["files"]
        .as_object()
        .context("source files")?
        .keys()
    {
        ownership::file(&request.project_root, path, &coverage)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_cannot_inject_unstable_flags_or_a_runtime_wrapper() {
        assert!(validate_cargo_config("[build]\ntarget-dir='somewhere'").is_ok());
        for text in [
            "[build]\nrustflags=['-Zanything']",
            "[build]\nrustc-wrapper='python3'",
            "[env]\nRUSTC_BOOTSTRAP='1'",
            "[alias]\nllvm-cov='!python3 hidden.py'",
        ] {
            assert!(validate_cargo_config(text).is_err());
        }
    }
    #[test]
    fn malformed_llvm_exports_are_not_measurements() {
        for value in [
            json!({}),
            json!({"type":"llvm.coverage.json.export", "version":"999", "data":[]}),
            json!({"type":"llvm.coverage.json.export", "version":"3.1.0", "data":[{"files":[{"filename":"x", "segments":[[1,2,-1,true,true,false]]}],"functions":[]}]}),
        ] {
            assert!(coverage::validate(&value).is_err());
        }
    }
    #[test]
    fn artifacts_cannot_self_authorize_a_new_manifest() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("manifest.json"), "{}").unwrap();
        assert!(verify(dir.path(), &"0".repeat(64), &"0".repeat(64))
            .unwrap_err()
            .to_string()
            .contains("anchor mismatch"));
    }
}
