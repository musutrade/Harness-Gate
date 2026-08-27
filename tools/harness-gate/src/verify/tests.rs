use super::parser::parse_result_count;
use super::*;
use crate::config::{FlowConfig, ParserConfig, ServiceConfig};
use crate::test_support::TestWorkspace;
use std::fs;

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
