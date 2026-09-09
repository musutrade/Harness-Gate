//! Typed measurements and provenance validation; no thresholds or gate decisions.
use super::{
    array, canonical, digest, error, index, json, model, project, require, schema, string,
    ReasonClass, Result,
};
use serde_json::{json, Value};
use std::{collections::BTreeSet, fs, path::Path};

pub fn metric_type(name: &str) -> Option<&'static str> {
    Some(match name {
        "coverage.line" | "coverage.function" | "coverage.region" | "coverage.branch"
        | "mutation.score" => "ratio",
        "complexity.cyclomatic"
        | "complexity.cognitive"
        | "mutation.killed"
        | "mutation.survived"
        | "security.findings"
        | "contract.breaking_changes"
        | "accessibility.violations" => "count",
        "risk.crap" => "decimal",
        "contract.schema_valid"
        | "contract.client_drift"
        | "contract.compatible"
        | "performance.regression" => "boolean",
        "performance.duration" => "duration",
        "bundle.size" => "size",
        _ => return None,
    })
}

fn shape(value: &Value, definition: Option<&str>) -> Result<()> {
    json::domain(value)?;
    schema::shape(value, &schema::EVIDENCE, definition)
}

pub fn series_id(series: &Value) -> Result<String> {
    let mut payload = series.clone();
    payload
        .as_object_mut()
        .ok_or_else(|| error("series must be an object"))?
        .remove("id");
    Ok(format!(
        "measurement-series/v1:{}",
        digest(&canonical(&payload)?)
    ))
}

pub fn validate_series(series: &Value) -> Result<model::Series> {
    let model = validate_series_transport(series)?;
    for contract in array(&series["metrics"]) {
        let name = string(&contract["name"]);
        let expected =
            metric_type(name).ok_or_else(|| error(format!("unknown generic metric: {name}")))?;
        require(
            contract["type"] == expected || (name == "risk.crap" && contract["type"] == "rational"),
            format!("wrong metric type: {name}"),
        )?;
    }
    Ok(model)
}

// Transport checks identities and typed facts without certifying metric semantics.
fn validate_series_transport(series: &Value) -> Result<model::Series> {
    shape(series, Some("Series"))?;
    let contracts = index(&series["metrics"], "name", "series metric")?;
    require(
        array(&series["metrics"])
            .iter()
            .map(|c| string(&c["name"]))
            .eq(contracts.keys().copied()),
        "series metrics must be sorted by name",
    )?;
    require(
        series["id"] == series_id(series)?,
        "noncanonical measurement series identity",
    )?;
    super::json::decode(series)
}

pub fn require_compatible_series(base: Option<&Value>, head: &Value) -> Result<()> {
    let base = base.ok_or_else(|| error("missing base series"))?;
    validate_series(base)?;
    validate_series(head)?;
    require(
        base == head,
        "incompatible measurement series; explicit migration required",
    )
}

fn file(root: &Path, path: &str, expected_digest: &str, size: Option<&Value>) -> Result<()> {
    project::canonical_path(path).map_err(|e| error(format!("artifact/source {path}: {e}")))?;
    let io = |e: std::io::Error| error(format!("artifact/source {path}: {e}"));
    let root = fs::canonicalize(root).map_err(io)?;
    let candidate = fs::canonicalize(root.join(path)).map_err(io)?;
    require(candidate.starts_with(&root), "path escapes declared root")?;
    require(candidate.is_file(), "artifact/source is not a regular file")?;
    let bytes = fs::read(candidate).map_err(io)?;
    require(
        digest(&bytes) == expected_digest,
        "artifact/source digest mismatch",
    )?;
    require(
        size.is_none_or(|n| n == &json!(bytes.len())),
        "artifact byte count mismatch",
    )
}

pub struct ValidationContext<'a> {
    pub project: &'a Value,
    pub source_root: &'a Path,
    pub artifact_root: &'a Path,
    pub expected: &'a Value,
}

fn record(
    record: &Value,
    context: &ValidationContext<'_>,
    series_validator: fn(&Value) -> Result<model::Series>,
) -> Result<()> {
    let ValidationContext {
        project,
        source_root,
        artifact_root,
        expected,
    } = context;
    shape(record, None)?;
    shape(expected, Some("Context"))?;
    series_validator(&record["series"])?;
    let subject = &record["subject"];
    require(record["project"] == project["id"], "project mismatch")?;
    require(
        array(&project["subjects"]).contains(subject),
        "unknown or altered subject",
    )?;
    require(
        record["component"] == subject["component"],
        "component mismatch",
    )?;
    require(
        record["context"] == **expected,
        "stale commit/base/target/run evidence",
    )?;
    require(
        expected["target"] == subject["target"] && subject["target"] == record["series"]["target"],
        "target mismatch",
    )?;
    require(
        record["collector"] == record["series"]["collector"],
        "collector version mismatch",
    )?;
    require(
        record["source"] == json!({"path":subject["path"], "sha256":subject["source_sha256"]}),
        "source identity mismatch",
    )?;
    file(
        source_root,
        string(&record["source"]["path"]),
        string(&record["source"]["sha256"]),
        None,
    )?;
    let artifacts = index(&record["artifacts"], "id", "artifact ID")?;
    index(&record["artifacts"], "path", "artifact path")?;
    for artifact in array(&record["artifacts"]) {
        require(
            artifact["context"] == **expected && artifact["source"] == record["source"],
            "stale artifact provenance",
        )?;
        file(
            artifact_root,
            string(&artifact["path"]),
            string(&artifact["sha256"]),
            Some(&artifact["bytes"]),
        )?;
    }
    validate_capabilities(record, &artifacts)
}

fn validate_capabilities(
    record: &Value,
    artifacts: &std::collections::BTreeMap<&str, &Value>,
) -> Result<()> {
    let capabilities = index(&record["capabilities"], "metric", "capability")?;
    let metrics = index(&record["metrics"], "name", "metric")?;
    let contracts = index(&record["series"]["metrics"], "name", "series metric")?;
    require(
        capabilities.keys().eq(contracts.keys()),
        "series/capability mismatch",
    )?;
    require(
        metrics.keys().all(|name| capabilities.contains_key(name)),
        "metric has no declared capability",
    )?;
    let mut used = BTreeSet::new();
    for capability in array(&record["capabilities"]) {
        let name = string(&capability["metric"]);
        let supported = capability["state"] == "supported";
        require(
            metrics.contains_key(name) == supported,
            format!("capability/evidence mismatch: {name}"),
        )?;
        let linked: BTreeSet<_> = array(&capability["artifacts"]).iter().map(string).collect();
        require(
            linked.len() == array(&capability["artifacts"]).len()
                && linked.iter().all(|id| artifacts.contains_key(id)),
            "unknown or duplicate capability artifact",
        )?;
        used.extend(linked.iter().copied());
        if supported {
            let metric = metrics[name];
            validate_value(&metric["value"], &contracts[name]["type"])?;
            let refs: BTreeSet<_> = array(&metric["artifacts"]).iter().map(string).collect();
            require(
                refs.len() == array(&metric["artifacts"]).len() && refs.is_subset(&linked),
                "metric/capability artifact mismatch",
            )?;
        }
    }
    require(
        used.iter().copied().eq(artifacts.keys().copied()),
        "unlinked raw artifact",
    )?;
    let states: BTreeSet<_> = capabilities.values().map(|c| string(&c["state"])).collect();
    let status = if states.contains("measurement_error") {
        "measurement_error"
    } else if states.contains("supported") {
        "measured"
    } else {
        "unavailable"
    };
    require(
        record["status"] == status,
        "measurement status/capability mismatch",
    )
}

fn validate_value(value: &Value, expected_type: &Value) -> Result<()> {
    let definition = match value["type"].as_str() {
        Some("ratio") => "Ratio",
        Some("count") => "Count",
        Some("boolean") => "Boolean",
        Some("duration") => "Duration",
        Some("size") => "Size",
        Some("decimal") => "Decimal",
        Some("rational") => "Rational",
        _ => return Err(error("unknown metric value type")),
    };
    shape(value, Some(definition))?;
    require(
        value["type"] == *expected_type,
        "metric value/series type mismatch",
    )?;
    if definition == "Ratio" {
        require(
            json::integer_cmp(&value["covered"], &value["total"]).is_le(),
            "covered exceeds total",
        )?;
    }
    Ok(())
}

pub fn validate_evidence(
    records: &Value,
    context: &ValidationContext<'_>,
) -> Result<Vec<model::EvidenceRecord>> {
    validate_batch(records, context, validate_series)
}

/// Validate immutable baseline transport, including arbitrary declared capability
/// names. This does not certify metrics or approve quality; evaluation continues
/// to use `validate_evidence` and its supported metric registry.
pub fn validate_evidence_transport(
    records: &Value,
    context: &ValidationContext<'_>,
) -> Result<Vec<model::EvidenceRecord>> {
    validate_batch(records, context, validate_series_transport)
}

fn validate_batch(
    records: &Value,
    context: &ValidationContext<'_>,
    series_validator: fn(&Value) -> Result<model::Series>,
) -> Result<Vec<model::EvidenceRecord>> {
    let result = (|| {
        project::validate_project(context.project)?;
        require(
            records.as_array().is_some_and(|r| !r.is_empty()),
            "evidence batch must be nonempty",
        )?;
        let mut ids = BTreeSet::new();
        let mut subjects = BTreeSet::new();
        for r in array(records) {
            record(r, context, series_validator)?;
            require(ids.insert(string(&r["id"])), "duplicate evidence ID")?;
            require(
                subjects.insert((string(&r["subject"]["id"]), string(&r["series"]["id"]))),
                "duplicate subject/series evidence",
            )?;
        }
        super::json::decode(records)
    })();
    result.map_err(|mut e: super::Error| {
        e.class = ReasonClass::MeasurementError;
        e
    })
}

pub fn canonical_serialize(records: &Value, context: &ValidationContext<'_>) -> Result<Vec<u8>> {
    validate_evidence(records, context)?;
    canonical(records)
}

pub fn evaluate_requirements(
    records: &Value,
    requirements: &Value,
    context: &ValidationContext<'_>,
) -> Result<Value> {
    validate_evidence(records, context)?;
    json::domain(requirements)?;
    schema::shape(requirements, &schema::REQUIREMENTS, None)?;
    let components = index(&context.project["components"], "id", "component")?;
    let mut seen = BTreeSet::new();
    let mut results = Vec::new();
    for requirement in array(&requirements["requirements"]) {
        let component = string(&requirement["component"]);
        let metric = string(&requirement["metric"]);
        require(
            components.contains_key(component) && metric_type(metric).is_some(),
            "unknown requirement scope",
        )?;
        require(
            seen.insert((component, metric)),
            "duplicate capability requirement",
        )?;
        let mut matching: Vec<_> = array(records)
            .iter()
            .filter(|r| r["component"] == component)
            .collect();
        let missing = Value::Null;
        if matching.is_empty() {
            matching.push(&missing);
        }
        for r in matching {
            let capability = r["capabilities"]
                .as_array()
                .and_then(|caps| caps.iter().find(|c| c["metric"] == metric));
            let state = capability.map_or("measurement_error", |c| string(&c["state"]));
            let status = if state == "supported" {
                "available"
            } else if requirement["mode"] == "required" && state != "measurement_error" {
                string(&requirement["on_unavailable"])
            } else {
                state
            };
            results.push(json!({"component":component, "metric":metric, "evidence_id":r["id"], "state":state, "status":status}));
        }
    }
    Ok(json!(results))
}
