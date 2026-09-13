//! Reconcile stable Makefile dep-info with authenticated package inventories.
//! This is not an assertion about arbitrary build-script reads or environment.
use crate::artifact::{self, FileIdentity};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

const SCHEMA: &str = "rust-stable-compiler-inputs/v1-candidate";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Proof {
    schema: String,
    scratch: PathBuf,
    records: BTreeMap<String, Record>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    raw: String,
    cwd: PathBuf,
    inputs: BTreeMap<String, Input>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    path: PathBuf,
    identity: FileIdentity,
    generated: bool,
}

// The candidate accepts the observed Linux rustc format, including escaped
// spaces and #. Make variables, continuations and unknown escapes fail closed.
fn parse(raw: &str) -> Result<BTreeSet<String>> {
    ensure!(raw.len() <= 8 * 1024 * 1024, "dep-info too large");
    let mut inputs = BTreeSet::new();
    for line in raw.lines().filter(|line| !line.is_empty()) {
        if line.starts_with("# env-dep:") {
            continue; // retained verbatim; no environment-closure claim
        }
        let mut words = Vec::new();
        let mut word = String::new();
        let mut separator = None;
        let mut chars = line.chars();
        while let Some(ch) = chars.next() {
            match ch {
                '\\' => {
                    let next = chars.next().context("unsupported dep-info continuation")?;
                    ensure!(
                        matches!(next, ' ' | '#' | '\\'),
                        "unsupported dep-info escape"
                    );
                    word.push(next);
                }
                ':' => {
                    ensure!(separator.is_none(), "ambiguous dep-info colon");
                    ensure!(
                        !word.is_empty() && words.is_empty(),
                        "dep-info target expected"
                    );
                    words.push(std::mem::take(&mut word));
                    separator = Some(words.len());
                }
                ' ' | '\t' => {
                    if !word.is_empty() {
                        words.push(std::mem::take(&mut word));
                    }
                }
                '$' | '#' | '\r' | '\0' => bail!("unsupported dep-info syntax"),
                _ => word.push(ch),
            }
        }
        if !word.is_empty() {
            words.push(word);
        }
        let separator = separator.context("missing dep-info separator")?;
        inputs.extend(words.into_iter().skip(separator));
    }
    ensure!(!inputs.is_empty(), "empty dep-info dependencies");
    Ok(inputs)
}

struct Inventory {
    files: BTreeMap<PathBuf, FileIdentity>,
    roots: BTreeSet<PathBuf>,
}

fn inventory(
    project: &Path,
    sources: &BTreeMap<String, FileIdentity>,
    dependencies: &Value,
) -> Result<Inventory> {
    let mut files: BTreeMap<_, _> = sources
        .iter()
        .map(|(name, id)| (project.join(name), id.clone()))
        .collect();
    let mut roots = BTreeSet::from([project.to_owned()]);
    for package in dependencies["packages"]
        .as_object()
        .context("dependency packages")?
        .values()
    {
        if package["kind"] == "locked-registry-archive" {
            let root = PathBuf::from(
                package["source_root"]
                    .as_str()
                    .context("registry source root")?,
            );
            let entries: BTreeMap<String, FileIdentity> =
                serde_json::from_value(package["files"].clone())?;
            files.extend(entries.into_iter().map(|(name, id)| (root.join(name), id)));
            roots.insert(root);
        } else {
            let manifest = Path::new(package["manifest"].as_str().context("workspace manifest")?);
            roots.insert(manifest.parent().context("package parent")?.to_owned());
        }
    }
    Ok(Inventory { files, roots })
}

fn resolve(token: &str, cwd: &Path) -> Result<PathBuf> {
    Ok(cwd.join(token).canonicalize()?)
}

fn records(directory: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        ensure!(
            kind.is_file() || kind.is_dir(),
            "special file in compiler output"
        );
        if kind.is_dir() {
            records(&entry.path(), output)?;
        } else if entry.path().extension().is_some_and(|s| s == "d") {
            output.push(entry.path());
        }
        ensure!(output.len() <= 10000, "too many dep-info records");
    }
    Ok(())
}

pub fn capture(
    root: &Path,
    scratch: &Path,
    project: &Path,
    sources: &BTreeMap<String, FileIdentity>,
    dependencies: &Value,
    rustc: &Path,
) -> Result<()> {
    let inventory = inventory(project, sources, dependencies)?;
    let mut paths = Vec::new();
    records(scratch, &mut paths)?;
    ensure!(!paths.is_empty(), "compiler dep-info missing");
    let directories = crate::compiler_wrapper::directories(root, scratch, rustc)?;
    ensure!(
        paths.iter().cloned().collect::<BTreeSet<_>>() == directories.keys().cloned().collect(),
        "compiler dep-info producer set differs"
    );
    let mut proof = Proof {
        schema: SCHEMA.into(),
        scratch: scratch.to_owned(),
        records: BTreeMap::new(),
    };
    for path in paths {
        let producer = directories
            .get(&path)
            .context("compiler input cwd missing")?;
        let cwd = &producer.cwd;
        ensure!(
            inventory.roots.contains(cwd),
            "compiler cwd outside authenticated packages"
        );
        ensure!(
            fs::metadata(&path)?.len() <= 8 * 1024 * 1024,
            "dep-info too large"
        );
        let raw = fs::read_to_string(&path)?;
        verify_raw(&raw, &producer.identity)?;
        let mut inputs = BTreeMap::new();
        for token in parse(&raw)? {
            let path = resolve(&token, cwd)?;
            let identity = artifact::identity(&path)?;
            let generated = path.starts_with(scratch);
            if generated {
                ensure!(
                    identity.bytes <= 8 * 1024 * 1024,
                    "generated compiler input too large"
                );
                fs::create_dir_all(root.join("compiler-generated"))?;
                fs::copy(
                    &path,
                    root.join("compiler-generated").join(&identity.sha256),
                )?;
            } else {
                ensure!(
                    inventory.files.get(&path) == Some(&identity),
                    "compiler input outside authenticated inventory: {}",
                    path.display()
                );
            }
            inputs.insert(
                token,
                Input {
                    path,
                    identity,
                    generated,
                },
            );
        }
        proof.records.insert(
            path.strip_prefix(scratch)?
                .to_str()
                .context("non UTF-8 dep-info path")?
                .into(),
            Record {
                raw,
                cwd: cwd.clone(),
                inputs,
            },
        );
    }
    artifact::write(&root.join("compiler-inputs.json"), &proof)
}

pub fn verify(
    root: &Path,
    project: &Path,
    sources: &BTreeMap<String, FileIdentity>,
    dependencies: &Value,
    rustc: &Path,
) -> Result<()> {
    let canonical_root = root.canonicalize()?;
    let root = canonical_root.as_path();
    let proof: Proof = serde_json::from_value(crate::strict_json::parse(&fs::read(
        root.join("compiler-inputs.json"),
    )?)?)?;
    ensure!(
        proof.schema == SCHEMA && !proof.records.is_empty() && proof.records.len() <= 10000,
        "invalid compiler input proof"
    );
    ensure!(
        proof.scratch.parent() == Some(root)
            && proof
                .scratch
                .file_name()
                .is_some_and(|s| s.to_string_lossy().starts_with("build-")),
        "invalid compiler scratch identity"
    );
    let inventory = inventory(project, sources, dependencies)?;
    let directories = crate::compiler_wrapper::directories(root, &proof.scratch, rustc)?;
    ensure!(
        proof
            .records
            .keys()
            .map(|name| proof.scratch.join(name))
            .collect::<BTreeSet<_>>()
            == directories.keys().cloned().collect(),
        "compiler dep-info producer set differs"
    );
    let mut generated = BTreeSet::new();
    for (name, record) in proof.records {
        ensure!(
            directories
                .get(&proof.scratch.join(&name))
                .is_some_and(|p| p.cwd == record.cwd)
                && inventory.roots.contains(&record.cwd),
            "compiler cwd identity differs"
        );
        ensure!(
            Path::new(&name)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
                && name.ends_with(".d"),
            "invalid dep-info record path"
        );
        ensure!(
            parse(&record.raw)? == record.inputs.keys().cloned().collect(),
            "dep-info input set differs"
        );
        verify_raw(
            &record.raw,
            &directories[&proof.scratch.join(&name)].identity,
        )?;
        for (token, input) in record.inputs {
            if input.generated {
                ensure!(
                    Path::new(&token) == input.path
                        && input.path.starts_with(&proof.scratch)
                        && input
                            .path
                            .components()
                            .all(|c| matches!(c, Component::RootDir | Component::Normal(_))),
                    "invalid generated input path"
                );
                ensure!(
                    input.identity.sha256.len() == 64
                        && input.identity.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                    "invalid generated input digest"
                );
                ensure!(
                    artifact::identity(
                        &root.join("compiler-generated").join(&input.identity.sha256)
                    )? == input.identity,
                    "generated compiler input identity changed"
                );
                generated.insert(input.identity.sha256);
            } else {
                ensure!(
                    resolve(&token, &record.cwd)? == input.path
                        && inventory.files.get(&input.path) == Some(&input.identity)
                        && artifact::identity(&input.path)? == input.identity,
                    "compiler input identity differs"
                );
            }
        }
    }
    let directory = root.join("compiler-generated");
    let actual: BTreeSet<_> = if directory.exists() {
        artifact::inventory(&directory, false)?
            .into_keys()
            .collect()
    } else {
        BTreeSet::new()
    };
    ensure!(actual == generated, "generated compiler input set differs");
    Ok(())
}

fn verify_raw(raw: &str, identity: &FileIdentity) -> Result<()> {
    ensure!(
        raw.len() as u64 == identity.bytes && artifact::digest(raw.as_bytes()) == identity.sha256,
        "compiler dep-info bytes differ from producer"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn producer_identity_binds_ignored_comments_and_exact_bytes() {
        let raw = "a: src/lib.rs\n# env-dep:EXAMPLE=observed\n";
        let identity = FileIdentity {
            sha256: artifact::digest(raw.as_bytes()),
            bytes: raw.len() as u64,
        };
        verify_raw(raw, &identity).unwrap();
        for altered in [
            raw.replace("observed", "modified"),
            raw.replace("# env-dep:EXAMPLE=observed\n", ""),
            format!("{raw}\n"),
        ] {
            assert_eq!(parse(raw).unwrap(), parse(&altered).unwrap());
            assert!(verify_raw(&altered, &identity).is_err());
        }
    }
    #[test]
    fn make_dep_info_is_parsed_without_interpreting_make_expressions() {
        assert_eq!(
            parse("/out/a.d: src/a\\ b.rs src/a\\#b.rs\n\nsrc/a\\ b.rs:\n# env-dep:OUT_DIR=/out\n")
                .unwrap(),
            BTreeSet::from(["src/a b.rs".into(), "src/a#b.rs".into()])
        );
        for raw in [
            "a:",
            "a: $(wildcard x)",
            "a: x \\",
            "a: x:y",
            "a: x #hidden",
            "a: x\\q",
        ] {
            assert!(parse(raw).is_err(), "{raw}");
        }
    }
    #[test]
    fn relative_inputs_resolve_from_the_observed_compiler_cwd() {
        let temp = tempfile::tempdir().unwrap();
        let a = temp.path().join("a");
        let b = temp.path().join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        fs::write(a.join("lib.rs"), "a").unwrap();
        fs::write(b.join("lib.rs"), "b").unwrap();
        assert_eq!(resolve("lib.rs", &a).unwrap(), a.join("lib.rs"));
        assert_eq!(resolve("lib.rs", &b).unwrap(), b.join("lib.rs"));
        assert!(resolve("absent.rs", &a).is_err());
    }
}
