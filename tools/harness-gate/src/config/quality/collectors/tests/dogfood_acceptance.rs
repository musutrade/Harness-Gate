//! Controlled transport fixtures, not Arc-Admin application tests or measurements.
use super::*;

fn receipt(fixture: &Fixture, id: &str, expected: &str, passed: bool) -> Value {
    let root = fixture.dir.path();
    let source = fs::read(root.join("src/lib.rs")).ok();
    let baseline = fs::read(root.join(".harness-gate/base-request.json")).ok();
    let project = crate::project::Project::discover(Some(root.to_path_buf()), None).unwrap();
    let report = crate::verify::run(
        &project,
        crate::scope::ScopeResult::all(&project),
        &fixture.state.profile,
        false,
    )
    .unwrap();
    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(report.passed, passed, "{id}: {value:#}");
    assert_eq!(value["quality"]["status"], expected, "{id}: {value:#}");
    assert_eq!(source, fs::read(root.join("src/lib.rs")).ok());
    assert_eq!(
        baseline,
        fs::read(root.join(".harness-gate/base-request.json")).ok()
    );
    let human =
        fs::read_to_string(Path::new(&report.report_directory).join("test_result.md")).unwrap();
    let mut encoded = serde_json::to_string(&json!({
        "id": id, "expected_quality_status": expected, "expected_workflow_passed": passed,
        "source_unchanged": true, "baseline_request_unchanged": true,
        "report": value, "human": human
    }))
    .unwrap()
    .replace(root.to_str().unwrap(), "<fixture>");
    if let Some(base) = value["quality"]["baseline"]["retained_directory"].as_str() {
        encoded = encoded.replace(base, "<baseline>");
        fs::remove_dir_all(base).unwrap();
    }
    serde_json::from_str(&encoded).unwrap()
}

#[test]
fn arc_admin_controlled_quality_negatives_retain_truthful_outcomes() {
    let mut cases = Vec::new();
    cases.push(receipt(
        &Fixture::workflow("pass", true, false),
        "control",
        "pass",
        true,
    ));
    for (mode, error) in [
        ("crash", "ADAPTER_PROTOCOL_FAILURE: adapter exited with 17"),
        (
            "missing-evidence",
            "missing or unexpected subject/capability/series",
        ),
        ("tamper-artifact", "artifact/source digest mismatch"),
        ("stale-context", "stale commit/base/target/run evidence"),
    ] {
        let case = receipt(
            &Fixture::workflow(mode, true, false),
            mode,
            "blocked",
            false,
        );
        assert_eq!(case["report"]["steps"][0]["passed"], true);
        assert!(case["report"]["quality"]["error"]
            .as_str()
            .unwrap()
            .contains(error));
        cases.push(case);
    }
    for mode in ["stale-baseline", "incompatible-baseline"] {
        let fixture = Fixture::workflow("crap", true, true);
        let path = fixture.dir.path().join(".harness-gate/base-request.json");
        let mut request: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        if mode == "stale-baseline" {
            request["state"]["expected"]["commit"] =
                json!("ffffffffffffffffffffffffffffffffffffffff");
        } else {
            request["state"]["series"]["stargazer"]["tool"]["version"] = json!("incompatible");
        }
        write(&path, &request);
        let case = receipt(&fixture, mode, "blocked", false);
        let error = if mode == "stale-baseline" {
            "baseline source/target identity mismatch"
        } else {
            "incompatible baseline profile/tool/measurement series"
        };
        assert!(case["report"]["quality"]["error"]
            .as_str()
            .unwrap()
            .contains(error));
        assert!(case["report"]["quality"]["project_report"].is_null());
        cases.push(case);
    }
    let crap = receipt(
        &Fixture::workflow("crap", true, true),
        "crap-ratchet",
        "fail",
        false,
    );
    for text in ["risk.crap", "35", "40", "30", "ratchet", "debt regressed"] {
        assert!(crap["human"].as_str().unwrap().contains(text), "{text}");
    }
    let gate = crap["report"]["quality"]["project_report"]["gates"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap();
    assert_eq!(gate["record"]["ratchet"]["debt"], "regressed");
    assert_eq!(gate["record"]["ratchet"]["legacy_debt_allowed"], false);
    cases.push(crap);
    for profile in ["hook", "full", "ci"] {
        let mut fixture = Fixture::workflow("crash", true, false);
        fixture.select_profile(profile, true);
        let case = receipt(
            &fixture,
            &format!("{profile}-omitted"),
            "not_collected",
            true,
        );
        let quality = &case["report"]["quality"];
        assert_eq!(quality["full_quality_status"], "not_collected");
        assert_eq!(quality["evidence"], json!([]));
        assert_eq!(quality["producers"], json!({}));
        assert!(!fixture.request.artifact_root.join("raw.json").exists());
        cases.push(case);
        let mut fixture = Fixture::workflow("not_collected", true, false);
        fixture.select_profile(profile, false);
        cases.push(receipt(
            &fixture,
            &format!("{profile}-required-not-collected"),
            "fail",
            false,
        ));
    }
    let mut matrix: Value = serde_json::from_str(include_str!(
        "../../../../../../quality/fixtures/workflow/ci-matrix.json"
    ))
    .unwrap();
    // Isolate CRAP from coverage: the negative keeps coverage at its passing value.
    for shape in matrix["shapes"].as_array_mut().unwrap() {
        for component in shape["components"].as_array_mut().unwrap() {
            for capability in component["capabilities"].as_array_mut().unwrap() {
                if capability["name"] == "coverage.line" {
                    capability["failure"] = capability["value"].clone();
                }
            }
        }
    }
    for (shape, mode, expected, passed) in [
        ("rust", "pass", "pass", true),
        ("rust", "policy-failure", "fail", false),
        ("angular-reference", "pass", "pass", true),
    ] {
        let shape = matrix["shapes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == shape)
            .unwrap();
        let fixture = ci_acceptance::configured_fixture(shape, mode, "ci");
        let case = receipt(
            &fixture,
            &format!("{}-{mode}", shape["id"].as_str().unwrap()),
            expected,
            passed,
        );
        let quality = &case["report"]["quality"];
        if shape["id"] == "angular-reference" {
            for record in quality["evidence"].as_array().unwrap() {
                assert!(record["capabilities"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|c| c["metric"] == "risk.crap" && c["state"] == "unsupported"));
                assert!(!record["metrics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|m| m["name"] == "risk.crap"));
            }
        } else if !passed {
            assert!(quality["project_report"]["gates"]
                .as_object()
                .unwrap()
                .values()
                .any(|g| g["state"] == "fail" && g["record"]["metric"] == "risk.crap"));
        }
        cases.push(case);
    }
    if let Some(output) = std::env::var_os("HARNESS_GATE_DOGFOOD_NEGATIVES") {
        let output = Path::new(&output);
        fs::create_dir_all(output).unwrap();
        write(
            &output.join("quality.json"),
            &json!({"schema":"arc-admin-negative-quality/v1", "measurement_origin":"synthetic signed transport fixtures", "cases":cases}),
        );
    }
}

#[test]
fn quality_preparation_failure_retains_requested_profile_and_blocks() {
    for name in ["workflow-state.json", "workflow-keys.json"] {
        let fixture = Fixture::workflow("pass", true, false);
        fs::remove_file(fixture.dir.path().join(".harness-gate").join(name)).unwrap();
        let case = receipt(&fixture, name, "blocked", false);
        let quality = &case["report"]["quality"];
        assert_eq!(quality["participation"]["profile"], "full");
        assert_eq!(quality["phase"], "configuration");
        assert_eq!(quality["full_quality_status"], "blocked");
        assert!(quality["project_report"].is_null());
        assert!(case["human"]
            .as_str()
            .unwrap()
            .contains("Quality profile \"full\": blocked"));
    }
}
