//! Core authenticates protocol-v2 requests before spawning this adapter. Signed
//! arguments pin a capture binding; this module never signs or adopts baselines.
use crate::{artifact, collect, coverage, ownership, source};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeSet, env, fs, io::Read, path::Path};

const NAME: &str = "harness-gate-rust-stable-collector";
const METRICS: [(&str, &str); 5] = [
    ("complexity.cyclomatic", "count"),
    ("coverage.function", "ratio"),
    ("coverage.line", "ratio"),
    ("coverage.region", "ratio"),
    ("risk.crap", "rational"),
];

fn hash(value: &Value) -> Result<String> {
    Ok(artifact::digest(&serde_json::to_vec(value)?))
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .filter(|v| !v.is_empty())
        .context(format!("missing {key}"))
}
fn keys(value: &Value, expected: &str) -> Result<()> {
    let actual: BTreeSet<_> = value
        .as_object()
        .context("expected object")?
        .keys()
        .map(String::as_str)
        .collect();
    ensure!(
        actual == expected.split_whitespace().collect(),
        "unexpected object fields"
    );
    Ok(())
}
fn read(path: &Path) -> Result<Value> {
    ensure!(
        artifact::identity(path)?.bytes <= 8 * 1024 * 1024,
        "JSON exceeds 8 MiB"
    );
    crate::strict_json::parse(&fs::read(path)?)
}

pub fn describe(root: &Path, anchor: &str, request_digest: &str) -> Result<Value> {
    collect::verify(root, anchor, request_digest)?;
    let request = read(&root.join("request.json"))?;
    let binary = artifact::identity(&env::current_exe()?)?;
    let mut series = json!({
        "name":"rust-stable-lexical-candidate",
        "collector":{"name":NAME,"version":env!("CARGO_PKG_VERSION")},
        "tool":{"name":"stable-rust-coverage-tools","version":hash(&request["tools"])?},
        "rule":{"name":"rust-source-decisions","version":source::SERIES},
        "runtime":{"name":"rustc","version":request["tools"]["commit"]},
        "target":request["tools"]["host"],
        "source_identity":{"name":"rust-lexical-source-span","version":"1"},
        "normalization":{"name":"stable-rust-core-candidate","version":binary.sha256},
        "metrics":METRICS.iter().map(|(name, kind)| json!({"name":name,"type":kind})).collect::<Vec<_>>()
    });
    series["id"] = json!(format!("measurement-series/v1:{}", hash(&series)?));
    let analysis = read(&root.join("source-analysis.json"))?;
    let raw = coverage::parse(&fs::read(root.join("coverage.json"))?)?;
    let mut owners = Vec::new();
    for (path, file) in analysis["files"].as_object().context("source inventory")? {
        // Recompute facts from the currently pinned source, not supplied metrics.
        let fresh = serde_json::to_value(source::analyze(&fs::read_to_string(
            Path::new(text(&request, "project_root")?).join(path),
        )?)?)?;
        ensure!(
            fresh == *file,
            "source analysis differs from recomputed facts"
        );
        let mapping = ownership::file(Path::new(text(&request, "project_root")?), path, &raw)?;
        for function in file["functions"].as_array().context("function inventory")? {
            owners.push(json!({"path":path,"source_sha256":request["source_files"][path]["sha256"],
                "discriminator":format!("rust-source-span/v1:{}", hash(&json!({"name":function["name"],"span":function["span"]}))?),
                "coverage_owner": mapping["functions"].as_array().context("mapped owners")?.iter().find(|o| o["name"] == function["name"] && o["span"] == function["span"]),
                "coverage_state":mapping["state"],"function":function,"file_supported":file["unsupported"].as_array().is_some_and(Vec::is_empty)}));
        }
    }
    Ok(
        json!({"schema":"rust-stable-core-description/v1", "series":series,"owners":owners,"workspace_root":request["project_root"]}),
    )
}

pub fn run(path: &Path, digest: &str) -> Result<Value> {
    let mut raw = Vec::new();
    std::io::stdin()
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut raw)?;
    ensure!(raw.len() <= 8 * 1024 * 1024, "request exceeds 8 MiB");
    let request = crate::strict_json::parse(&raw)?;
    project(&request, path, digest)
}

fn project(request: &Value, path: &Path, digest: &str) -> Result<Value> {
    keys(request, "protocol_version result_schema_version adapter invocation_id step_id timeout_ms config_digest artifact_root nonce issued_at_ms expires_at_ms args environment capabilities input")?;
    ensure!(
        request["protocol_version"] == 2 && request["result_schema_version"] == "1",
        "incompatible Core protocol"
    );
    ensure!(
        path.is_absolute() && artifact::identity(path)?.sha256 == digest,
        "binding identity mismatch"
    );
    ensure!(
        request["args"] == json!(["adapter", "--binding", path, "--binding-sha256", digest]),
        "binding differs from signed arguments"
    );
    let binding = read(path)?;
    keys(
        &binding,
        "schema input invocation_id config_digest capture project series",
    )?;
    ensure!(
        binding["schema"] == "rust-stable-core-binding/v1",
        "binding schema"
    );
    for key in ["input", "invocation_id", "config_digest"] {
        ensure!(
            binding[key] == request[key],
            "stale or altered binding: {key}"
        );
    }
    let inner = &request["input"];
    keys(
        inner,
        "schema project collector context workspace_root output_root selection bindings",
    )?;
    ensure!(
        inner["schema"] == "harness-project-collector-request/v1",
        "collector protocol"
    );
    ensure!(
        inner["output_root"] == request["artifact_root"],
        "artifact root mismatch"
    );
    ensure!(
        inner["collector"] == json!({"name":NAME,"version":env!("CARGO_PKG_VERSION")}),
        "collector identity mismatch"
    );
    for key in ["name", "version"] {
        ensure!(
            request["adapter"][key] == inner["collector"][key],
            "adapter identity mismatch"
        );
    }
    ensure!(
        request["adapter"]["source_digest"] == artifact::identity(&env::current_exe()?)?.sha256,
        "executable identity mismatch"
    );
    for (variable, field) in [
        ("HARNESS_GATE_INVOCATION_ID", "invocation_id"),
        ("HARNESS_GATE_STEP_ID", "step_id"),
        ("HARNESS_GATE_ARTIFACT_ROOT", "artifact_root"),
    ] {
        ensure!(
            env::var(variable).ok().as_deref() == request[field].as_str(),
            "Core invocation environment mismatch"
        );
    }
    let capture = &binding["capture"];
    keys(capture, "path manifest_sha256 request_sha256")?;
    let root = Path::new(text(capture, "path")?);
    ensure!(
        root.is_absolute() && !fs::symlink_metadata(root)?.is_symlink(),
        "unsafe capture root"
    );
    let description = describe(
        root,
        text(capture, "manifest_sha256")?,
        text(capture, "request_sha256")?,
    )?;
    ensure!(
        description["workspace_root"] == inner["workspace_root"],
        "workspace mismatch"
    );
    ensure!(
        inner["context"]["target"] == description["series"]["target"],
        "context target mismatch"
    );
    ensure!(
        binding["series"] == description["series"],
        "incompatible measurement series"
    );
    ensure!(
        binding["project"]["id"] == inner["project"],
        "project mismatch"
    );
    let mut claims = BTreeSet::new();
    for claim in inner["bindings"].as_array().context("claims")? {
        keys(claim, "subject capability series")?;
        ensure!(
            claim["series"] == description["series"]["id"],
            "claim series mismatch"
        );
        ensure!(
            claims.insert((
                text(claim, "subject")?.to_owned(),
                text(claim, "capability")?.to_owned()
            )),
            "duplicate claim"
        );
    }
    ensure!(!claims.is_empty(), "empty claims");
    let selected: BTreeSet<_> = claims.iter().map(|(s, _)| s.clone()).collect();
    let expected: BTreeSet<_> = selected
        .iter()
        .flat_map(|s| METRICS.iter().map(move |(m, _)| (s.clone(), m.to_string())))
        .collect();
    ensure!(claims == expected, "incomplete capability contract");
    let subjects = binding["project"]["subjects"]
        .as_array()
        .context("subjects")?;
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    for subject in subjects {
        let id = text(subject, "id")?;
        ensure!(seen.insert(id), "duplicate subject");
        if !selected.contains(id) {
            continue;
        }
        ensure!(subject["kind"] == "function/v1", "unsupported subject kind");
        let owners: Vec<_> = description["owners"]
            .as_array()
            .context("owners")?
            .iter()
            .filter(|owner| {
                owner["path"] == subject["path"]
                    && owner["discriminator"] == subject["discriminator"]
            })
            .collect();
        ensure!(owners.len() == 1, "missing or ambiguous source owner");
        let owner = owners[0];
        ensure!(
            subject["source_sha256"] == owner["source_sha256"],
            "subject source identity mismatch"
        );
        let span = &owner["function"]["span"];
        if let Some(actual) = subject.get("span") {
            ensure!(
                *actual
                    == json!({"start_line":span[0],"start_column":span[1],"end_line":span[2],"end_column":span[3]}),
                "subject span mismatch"
            );
        }
        rows.push((subject, owner));
    }
    ensure!(rows.len() == selected.len(), "missing selected subject");
    let output = Path::new(text(inner, "output_root")?);
    ensure!(
        output.is_absolute()
            && fs::symlink_metadata(output)?.is_dir()
            && fs::read_dir(output)?.next().is_none(),
        "output must be an empty real directory"
    );
    let mut records = Vec::new();
    let mut artifacts = Vec::new();
    for (subject, owner) in rows {
        let supported =
            owner["file_supported"] == true && owner["function"]["state"] == "supported";
        let source = json!({"path":subject["path"],"sha256":subject["source_sha256"]});
        let name = format!("{}.json", hash(&subject["id"])?);
        artifact::write(
            &output.join(&name),
            &json!({"schema":"rust-stable-source-owner/v1","capture":capture,"series":description["series"],"owner":owner,"context":inner["context"]}),
        )?;
        let identity = artifact::identity(&output.join(&name))?;
        let artifact = json!({"id":"source","kind":"raw","media_type":"application/json","path":name,"sha256":identity.sha256,"bytes":identity.bytes,"source":source,"context":inner["context"]});
        let capabilities: Vec<_> = METRICS.iter().map(|(metric,_)| {
            let available = supported && (*metric == "complexity.cyclomatic" || matches!(*metric, "coverage.function" | "coverage.region") && owner["coverage_state"] == "supported");
            json!({"metric":metric,"state":if available {"supported"} else {"unsupported"},"reason":if available {if matches!(*metric, "coverage.function" | "coverage.region") {"exact source span and unique LLVM free function; explicit test spans excluded"} else {"lexical source decisions; no expansion or reachability claim"}} else {"source/coverage ownership or source activation is not certified"},"artifacts":["source"]})
        }).collect();
        let mut metrics = Vec::new();
        if supported {
            metrics.push(json!({"name":"complexity.cyclomatic","value":{"type":"count","value":owner["function"]["complexity"]},"artifacts":["source"]}));
            if owner["coverage_state"] == "supported" {
                ensure!(
                    !owner["coverage_owner"].is_null(),
                    "certified coverage owner missing"
                );
                metrics.push(json!({"name":"coverage.function","value":owner["coverage_owner"]["coverage_function"],"artifacts":["source"]}));
                metrics.push(json!({"name":"coverage.region","value":owner["coverage_owner"]["coverage_region"],"artifacts":["source"]}));
            }
        }
        records.push(json!({"schema":"harness-evidence/v1","id":format!("stable-{}",hash(&subject["id"])?),"project":inner["project"],"component":subject["component"],"collector":inner["collector"],"series":description["series"],"subject":subject,"context":inner["context"],"source":source,"artifacts":[artifact],"capabilities":capabilities,"status":if supported {"measured"} else {"unavailable"},"metrics":metrics}));
        artifacts.push(artifact);
    }
    // Recheck every pinned input after conversion; partial files on failure never
    // yield a successful response and Core will reject the failed process.
    collect::verify(
        root,
        text(capture, "manifest_sha256")?,
        text(capture, "request_sha256")?,
    )?;
    ensure!(
        artifact::identity(path)?.sha256 == digest,
        "binding changed during conversion"
    );
    Ok(
        json!({"schema_version":"1","status":"PASS","invocation_id":request["invocation_id"],"artifacts":artifacts,"collection":{"schema":"harness-project-collector-response/v1","evidence":records,"error":null}}),
    )
}
