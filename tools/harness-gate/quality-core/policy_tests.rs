//! Differential tasks 3–4 tests. The Python process is a test oracle, never runtime.
use super::{evidence::ValidationContext, json, policy, project_report, ratchet, Result};
use serde_json::{json, Value};
use std::{fs, path::Path, process::Command, sync::LazyLock};
use tempfile::TempDir;

static REFERENCE: LazyLock<(TempDir, Vec<Value>)> = LazyLock::new(|| {
    let temp = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("python3")
        .arg(root.join("quality-core/tests/policy_reference.py"))
        .arg(root.parent().unwrap().join("quality"))
        .arg(temp.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("{}", String::from_utf8_lossy(&output.stdout));
    let cases = json::parse(&fs::read_to_string(temp.path().join("cases.json")).unwrap()).unwrap();
    (temp, cases.as_array().unwrap().clone())
});

fn context(value: &Value) -> ValidationContext<'_> {
    ValidationContext {
        project: &value["project"],
        expected: &value["expected"],
        source_root: Path::new(value["source_root"].as_str().unwrap()),
        artifact_root: Path::new(value["artifact_root"].as_str().unwrap()),
    }
}

fn run(case: &Value) -> Result<Value> {
    let a = &case["args"];
    let k = &case["kwargs"];
    match case["kind"].as_str().unwrap() {
        "evaluate" => {
            let ctx = context(k);
            let base = k.get("base_context").filter(|v| !v.is_null()).map(context);
            policy::evaluate(
                &a[0],
                &a[1],
                &ctx,
                &policy::EvaluationOptions {
                    selection: k.get("selection"),
                    base_records: k.get("base_records").filter(|v| !v.is_null()),
                    base_context: base.as_ref(),
                    mappings: k.get("mappings").filter(|v| !v.is_null()),
                    exceptions: k.get("exceptions"),
                    now: k["now"].as_str(),
                },
            )
        }
        "report" => project_report::report(&a[0], &a[1], &a[2]),
        "decision" => ratchet::decision(&a[0], &a[1], Some(&a[2])).map(|v| json!(v)),
        "compare" => policy::compare(&a[0], a[1].as_str().unwrap(), &a[2]).map(Value::Bool),
        "aggregate" => policy::aggregate(
            &a[0],
            &serde_json::from_value::<Vec<policy::GateResult>>(a[1].clone()).unwrap(),
        ),
        "validate_policy" => policy::validate_policy(&a[0], &a[1]).map(|_| a[0].clone()),
        "_select" => policy::select(&a[0], &a[1], &a[2], &a[3]).map(|v| json!(v)),
        "review_exceptions" => Ok(ratchet::review_exceptions(
            &a[0],
            &a[1],
            &a[2],
            k["now"].as_str(),
        )),
        other => panic!("unknown test kind {other}"),
    }
}

fn difference(a: &Value, b: &Value, path: &str) -> Option<String> {
    // GH-147 already classified runtime-specific missing-file diagnostics.
    // Compare the path and failure category; retain every semantic field exactly.
    if path.ends_with(".reason") {
        if let (Some(a), Some(b)) = (a.as_str(), b.as_str()) {
            if super::replay::missing_file_source(a).is_some()
                && super::replay::missing_file_source(a) == super::replay::missing_file_source(b)
            {
                return None;
            }
        }
    }
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        if a.keys().eq(b.keys()) {
            return a
                .iter()
                .find_map(|(key, value)| difference(value, &b[key], &format!("{path}.{key}")));
        }
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        if a.len() == b.len() {
            return a
                .iter()
                .zip(b)
                .enumerate()
                .find_map(|(i, (a, b))| difference(a, b, &format!("{path}[{i}]")));
        }
    }
    Some(format!("{path}: Rust {a} versus Python {b}"))
}

#[test]
fn frozen_policy_and_reference_acceptance_match_python() {
    let mut mismatches = Vec::new();
    for case in &REFERENCE.1 {
        let actual = match run(case) {
            Ok(value) => {
                if let Some(expected) = case.get("project_report") {
                    let report = project_report::report(
                        &value,
                        &case["kwargs"]["project"],
                        &case["args"][0],
                    )
                    .unwrap();
                    if let Some(diff) = difference(&report, expected, "$.project_report") {
                        mismatches.push(format!("{}: {diff}", case["name"]));
                    }
                }
                json!({"accepted":true,"value":value})
            }
            Err(e) => {
                json!({"accepted":false,"reason_class":format!("{:?}",e.class),"reason":e.message})
            }
        };
        if let Some(diff) = difference(&actual, &case["oracle"], "$") {
            mismatches.push(format!("{}: {diff}", case["name"]));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} mismatches:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

#[test]
fn green_local_gates_and_breaking_contract_block_project_with_provenance() {
    let case = REFERENCE
        .1
        .iter()
        .find(|c| c["name"] == "contract-breaking")
        .unwrap();
    let result = run(case).unwrap();
    let report =
        project_report::report(&result, &case["kwargs"]["project"], &case["args"][0]).unwrap();
    assert_eq!(report["aggregate"]["state"], "fail");
    for component in ["api", "frontend"] {
        assert_eq!(report["components"][component]["local"]["state"], "pass");
        assert_eq!(
            report["components"][component]["cross_component"]["state"],
            "fail"
        );
        assert_eq!(
            report["components"][component]["aggregate"]["state"],
            "fail"
        );
    }
    assert_eq!(report["gates"].as_object().unwrap().len(), 5);
    assert_eq!(report["aggregate"]["blockers"].as_array().unwrap().len(), 3);
    let gate = report["gates"]
        .as_object()
        .unwrap()
        .values()
        .find(|g| g["policy"] == "contract.breaking_changes")
        .unwrap();
    assert_eq!(gate["record"]["relationship"]["producer"], "api");
    assert_eq!(gate["record"]["relationship"]["consumer"], "frontend");
    assert!(!gate["record"]["evidence_links"]["head"]["artifacts"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(difference(&report, &case["project_report"], "$").is_none());
}
