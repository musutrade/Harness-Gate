use super::parser::parse_result_count;
use super::*;
use crate::config::{FlowConfig, ParserConfig, ServiceConfig};
use crate::test_support::TestWorkspace;
use std::fs;
use std::path::PathBuf;

#[test]
fn configurable_regex_parser_counts_multiple_outputs() {
    let parser = ParserConfig::Regex {
        patterns: vec![r"(?m)^running ([0-9]+) tests?$".into()],
        capture: 1,
        minimum: 1,
    };
    let log = "running 3 tests\n...\nrunning 2 tests\n";
    assert_eq!(parse_result_count(log, &parser).expect("count"), (5, 1));
}

#[test]
fn a_new_test_framework_can_supply_its_own_pattern() {
    let parser = ParserConfig::Regex {
        patterns: vec![r"passed: ([0-9]+)".into()],
        capture: 1,
        minimum: 2,
    };
    assert_eq!(
        parse_result_count("passed: 7", &parser).expect("count"),
        (7, 2)
    );
}

#[test]
fn regex_parser_ignores_ansi_color_sequences() {
    let parser = ParserConfig::Regex {
        patterns: vec![r"Tests\s+([0-9]+) passed".into()],
        capture: 1,
        minimum: 1,
    };
    let log = "\u{1b}[1mTests\u{1b}[22m  \u{1b}[32m58 passed\u{1b}[39m";
    assert_eq!(parse_result_count(log, &parser).expect("count"), (58, 1));
}

#[test]
fn service_failure_does_not_skip_unrelated_steps() {
    let workspace = TestWorkspace::new("verify");
    crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
    let flow_path = workspace.root.join(".harness-gate/flow.toml");
    let source = fs::read_to_string(&flow_path).expect("read fixture config");
    let mut config: FlowConfig = toml::from_str(&source).expect("parse fixture config");
    let source_env = "HARNESS_GATE_MISSING_TEST".to_string();
    assert!(std::env::var_os(&source_env).is_none());
    config.services.insert(
        "missing-service".into(),
        ServiceConfig::Environment {
            source_env,
            inject_env: "TEST_SERVICE_URL".into(),
        },
    );
    config.steps[0].services = vec!["missing-service".into()];
    config.steps[1].profiles.insert("full".into());
    fs::write(
        &flow_path,
        toml::to_string_pretty(&config).expect("serialize fixture config"),
    )
    .expect("write fixture config");
    workspace.init_git();
    let project = Project::discover(Some(workspace.root.clone()), None).expect("discover fixture");

    let report = run(&project, ScopeResult::all(&project), "full", false).expect("verify fixture");

    assert!(!report.passed);
    assert!(report
        .steps
        .iter()
        .any(|step| step.label == "staged Git whitespace check" && step.passed));
}

#[cfg(unix)]
#[test]
fn independent_steps_run_in_parallel_and_publish_in_plan_order() {
    let (_workspace, mut project) = generic_project("verify-parallel");
    project.config.execution.parallel = true;
    project.config.execution.max_parallel = Some(2);
    for step in &mut project.config.steps {
        step.profiles.insert("full".into());
        step.program = "sh".into();
        step.args = vec!["-c".into(), "sleep 2".into()];
    }

    let started = std::time::Instant::now();
    let report = run(&project, ScopeResult::all(&project), "full", false)
        .expect("parallel verification should pass");

    assert!(started.elapsed() < std::time::Duration::from_millis(3500));
    let labels = report
        .steps
        .iter()
        .map(|step| step.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        labels,
        vec![
            "secret scan",
            "architecture audit",
            "Git whitespace check",
            "staged Git whitespace check",
        ]
    );
}

fn generic_project(name: &str) -> (TestWorkspace, Project) {
    let workspace = TestWorkspace::new(name);
    crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
    workspace.init_git();
    let project = Project::discover(Some(workspace.root.clone()), None).expect("discover fixture");
    (workspace, project)
}

#[test]
fn non_zero_step_retains_exit_detail_and_log() {
    let (workspace, mut project) = generic_project("verify-non-zero");
    project.config.steps[0].program = "sh".into();
    project.config.steps[0].args = vec!["-c".into(), "exit 7".into()];
    let report = run(&project, ScopeResult::all(&project), "full", false).expect("verify fixture");
    let step = report
        .steps
        .iter()
        .find(|step| step.label == project.config.steps[0].label)
        .expect("failed step");
    assert!(!step.passed);
    assert_eq!(step.detail.as_deref(), Some("exit code 7"));
    assert!(PathBuf::from(&step.log).is_file());
    assert!(workspace
        .root
        .join(".harness-gate/reports/test_result.json")
        .is_file());
}

#[test]
fn parser_failure_marks_step_failed() {
    let (_workspace, mut project) = generic_project("verify-parser-failure");
    project.config.steps[0].program = "sh".into();
    project.config.steps[0].args = vec!["-c".into(), "printf 'done\\n'".into()];
    project.config.parsers.insert(
        "required-count".into(),
        ParserConfig::Regex {
            patterns: vec!["count: ([0-9]+)".into()],
            capture: 1,
            minimum: 1,
        },
    );
    project.config.steps[0].parser = Some("required-count".into());
    let report = run(&project, ScopeResult::all(&project), "full", false).expect("verify fixture");
    let step = report
        .steps
        .iter()
        .find(|step| step.label == project.config.steps[0].label)
        .expect("parsed step");
    assert!(!step.passed);
    assert!(step
        .detail
        .as_deref()
        .unwrap_or_default()
        .contains("expected at least 1"));
}

#[test]
fn gate_failure_writes_compatible_report() {
    let (workspace, project) = generic_project("verify-gate-failure");
    let token = format!("{}{}", "ghp_", "123456789012345678901234567890123456");
    fs::write(
        workspace.root.join("leaked.txt"),
        format!("token = \\\"{token}\\\"\\n"),
    )
    .expect("write secret fixture");
    let report = run(&project, ScopeResult::all(&project), "full", false).expect("gate report");
    assert!(!report.passed);
    assert!(report
        .steps
        .iter()
        .any(|step| step.label == "secret scan" && !step.passed));
    assert!(workspace
        .root
        .join(".harness-gate/reports/test_result.json")
        .is_file());
    assert!(workspace
        .root
        .join(".harness-gate/reports/test_result.md")
        .is_file());
}

#[test]
fn report_write_failure_returns_error() {
    let (workspace, mut project) = generic_project("verify-report-failure");
    let report_path = workspace.root.join(".harness-gate/reports");
    fs::create_dir_all(&report_path).expect("create report directory");
    fs::create_dir(report_path.join("test_result.json")).expect("block JSON report");
    project.reports = report_path;
    let error =
        run(&project, ScopeResult::all(&project), "full", false).expect_err("report must fail");
    assert!(matches!(error, VerifyError::Report { .. }));
}
