use super::*;
use crate::test_support::TestWorkspace;
use std::collections::BTreeSet;
use std::fs;

fn repository_config() -> FlowConfig {
    toml::from_str(include_str!("../../presets/rust-api.flow.toml")).expect("parse fixture")
}

#[test]
fn repository_configuration_is_valid() {
    repository_config().validate().expect("validate config");
}

#[test]
fn execution_policy_defaults_to_serial_and_four_workers_when_enabled() {
    let config = repository_config();
    assert!(!config.execution.parallel);
    assert_eq!(config.execution.effective_max_parallel(), 4);

    let mut config = config;
    config.execution.parallel = true;
    assert_eq!(config.execution.effective_max_parallel(), 4);
    config.execution.max_parallel = Some(8);
    assert_eq!(config.execution.effective_max_parallel(), 8);
    config.validate().expect("valid execution policy");
}

#[test]
fn execution_policy_rejects_zero_and_values_above_the_bound() {
    let mut config = repository_config();
    for value in [0, 65] {
        config.execution.max_parallel = Some(value);
        let error = config.validate().expect_err("invalid execution bound");
        assert!(error.to_string().contains("execution.max_parallel"));
    }
}

#[test]
fn execution_policy_diagnostic_points_to_max_parallel() {
    let mut config = repository_config();
    config.execution.max_parallel = Some(65);
    let source = toml::to_string_pretty(&config).expect("serialize invalid execution config");
    let error = FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("invalid execution bound must produce diagnostics");
    assert_eq!(error.report().diagnostics[0].path, "execution.max_parallel");
}

#[test]
fn existing_v2_config_defaults_the_secret_rule_path() {
    let source = include_str!("../../presets/rust-api.flow.toml")
        .lines()
        .filter(|line| !line.starts_with("secrets_config = "))
        .collect::<Vec<_>>()
        .join("\n");
    let config = FlowConfig::from_source(&source).expect("compatible v2 config");

    assert_eq!(config.paths.secrets_config, ".harness-gate/secrets.toml");
}

#[test]
fn components_and_profiles_are_not_hard_coded() {
    let mut config = repository_config();
    config.steps[0].component = "mobile".into();
    config.steps[0].profiles.insert("ci".into());
    config.project.default_profile = "ci".into();
    config.scope.rules[0].components = BTreeSet::from(["mobile".into()]);
    config.validate().expect("custom component is valid");
}

#[test]
fn policy_steps_cannot_be_missing() {
    let mut config = repository_config();
    config.steps.retain(|step| step.id != "rust.tests");
    let error = config.validate().expect_err("missing step must fail");
    assert!(error.to_string().contains("rust.tests"));
}

#[test]
fn step_dependency_cycles_are_rejected() {
    let mut config = repository_config();
    let first = config.steps[0].id.clone();
    let second = config.steps[1].id.clone();
    config.steps[0].depends_on = vec![second];
    config.steps[1].depends_on = vec![first];
    let error = config.validate().expect_err("cycle must fail");
    assert!(error.to_string().contains("cycle"));
}

#[test]
fn shell_command_strings_are_rejected() {
    let mut config = repository_config();
    let step = config.steps.first_mut().expect("step");
    step.program = "sh".into();
    step.args = vec!["-lc".into(), "cargo fmt".into()];
    let error = config.validate().expect_err("shell command must fail");
    assert!(error.to_string().contains("shell command string"));
}

#[test]
fn duplicate_service_injection_variables_are_rejected() {
    let mut config = repository_config();
    config.services.insert(
        "test-cache".into(),
        ServiceConfig::Environment {
            source_env: "CACHE_URL".into(),
            inject_env: "TEST_DATABASE_URL".into(),
        },
    );
    // Add a service to the first step and try to inject duplicate env var
    let step = config.steps.first_mut().expect("step exists");
    step.services.push("test-cache".into());

    // Add another service with same inject_env
    config.services.insert(
        "test-db".into(),
        ServiceConfig::Environment {
            source_env: "DB_URL".into(),
            inject_env: "TEST_DATABASE_URL".into(),
        },
    );
    step.services.push("test-db".into());

    let error = config.validate().expect_err("injection must be unique");
    assert!(error.to_string().contains("multiple services injecting"));
}

#[test]
fn unknown_fields_are_rejected() {
    let source = include_str!("../../presets/rust-api.flow.toml").replacen(
        "version = 2",
        "version = 2\nunknown = true",
        1,
    );
    assert!(toml::from_str::<FlowConfig>(&source).is_err());
}

#[test]
fn built_in_gate_declarations_are_validated_as_typed_steps() {
    let mut config = repository_config();
    let gate = &mut config.steps[0];
    gate.id = "builtin.secret-scan".into();
    gate.label = "secret scan".into();
    gate.component.clear();
    gate.program.clear();
    gate.args.clear();
    gate.cwd.clear();
    gate.log.clear();
    gate.timeout_secs = 0;
    gate.timeout_env = None;
    gate.parser = None;
    gate.services.clear();
    gate.remove_env.clear();
    gate.depends_on.clear();
    gate.kind = Some("builtin-gate".into());
    gate.gate_type = Some("secret-scan".into());
    config.policy.required_steps.clear();
    config.validate().expect("built-in gate is valid");
}

#[test]
fn both_builtin_gate_declarations_can_coexist_without_log_conflicts() {
    let mut config = repository_config();
    let mut secret = config.steps[0].clone();
    secret.id = "builtin.secret-scan".into();
    secret.label = "repository secret policy".into();
    secret.component.clear();
    secret.program.clear();
    secret.args.clear();
    secret.cwd.clear();
    secret.log.clear();
    secret.timeout_secs = 0;
    secret.timeout_env = None;
    secret.parser = None;
    secret.services.clear();
    secret.remove_env.clear();
    secret.depends_on.clear();
    secret.kind = Some("builtin-gate".into());
    secret.gate_type = Some("secret-scan".into());

    let mut audit = secret.clone();
    audit.id = "builtin.architecture-audit".into();
    audit.label = "architecture policy".into();
    audit.gate_type = Some("architecture-audit".into());
    config.steps.extend([secret, audit]);

    config.validate().expect("both built-in gates are valid");
}

#[test]
fn unknown_built_in_gate_types_fail_closed() {
    let mut config = repository_config();
    let gate = &mut config.steps[0];
    gate.id = "builtin.future".into();
    gate.component.clear();
    gate.program.clear();
    gate.args.clear();
    gate.cwd.clear();
    gate.log.clear();
    gate.timeout_secs = 0;
    gate.timeout_env = None;
    gate.parser = None;
    gate.services.clear();
    gate.remove_env.clear();
    gate.depends_on.clear();
    gate.kind = Some("builtin-gate".into());
    gate.gate_type = Some("future-gate".into());
    config.policy.required_steps.clear();
    let error = config.validate().expect_err("unknown gate must fail");
    assert!(error.to_string().contains("unknown gate_type"));
}

#[test]
fn external_steps_reject_gate_fields() {
    let mut config = repository_config();
    config.steps[0].kind = Some("external-step".into());
    config.steps[0].gate_type = Some("secret-scan".into());
    let error = config
        .validate()
        .expect_err("external gate field must fail");
    assert!(error.to_string().contains("may not declare gate_type"));
}

#[test]
fn environment_interpolation_supports_defaults() {
    let source = include_str!("../../presets/rust-api.flow.toml").replace(
        "name = \"rust-api\"",
        "name = \"${HG_TEST_NAME:-interpolated}\"",
    );
    let config = FlowConfig::from_source(&source).expect("interpolated config");
    assert_eq!(config.project.name, "interpolated");
}

#[test]
fn environment_interpolation_requires_defined_variables() {
    let source = include_str!("../../presets/rust-api.flow.toml")
        .replace("name = \"rust-api\"", "name = \"${HG_MISSING_VARIABLE}\"");
    let error = FlowConfig::from_source(&source).expect_err("missing variable must fail");
    assert!(error.to_string().contains("HG_MISSING_VARIABLE"));
}

#[test]
fn audit_configuration_path_rejects_environment_interpolation() {
    let source = include_str!("../../presets/rust-api.flow.toml").replace(
        "audit_config = \".harness-gate/audit.toml\"",
        "audit_config = \"${HG_PROJECT_AUDIT_CONFIG:-policies/audit.toml}\"",
    );
    let error = FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("audit configuration must remain project-scoped");
    let diagnostic = error
        .report()
        .diagnostics
        .into_iter()
        .next()
        .expect("project-scoped diagnostic");

    assert_eq!(diagnostic.id, "HGCFG-PROJECT-SCOPED-CONFIG");
    assert_eq!(diagnostic.path, "paths.audit_config");
}

#[test]
fn version_one_configuration_can_be_migrated() {
    let source = r#"
version = 1

[paths]
backend = "backend"
frontend = "frontend"
reports = "reports"
tool_manifest = "tools/arc-flow/Cargo.toml"
audit_config = "audit.toml"

[doctor]
required_commands = ["git"]
node_version_file = ".node-version"
hooks_path = "hooks"

[database]
image = "postgres:16-alpine"
startup_timeout_secs = 30
container_port = 5432
user = "test"
password = "test"
name = "test"

[[scope.rules]]
patterns = ["src/**"]
components = ["app"]

[[steps]]
id = "app.check"
label = "app check"
component = "app"
profiles = ["full", "hook"]
program = "git"
args = ["diff", "--check"]
cwd = "{root}"
log = "app_check.log"
timeout_secs = 60
"#;
    let migrated = migrate_v1(source, "example").expect("migrate");
    assert_eq!(migrated.version, 2);
    assert_eq!(migrated.project.name, "example");
    assert_eq!(migrated.paths.secrets_config, ".harness-gate/secrets.toml");
    assert!(migrated.policy.required_steps.contains(&"app.check".into()));
}

#[test]
fn unordered_steps_reusing_a_service_are_rejected_with_field_paths() {
    let mut config = repository_config();
    config.services.insert(
        "test-db".into(),
        ServiceConfig::Environment {
            source_env: "DATABASE_URL".into(),
            inject_env: "TEST_DATABASE_URL".into(),
        },
    );
    config.steps[0].services = vec!["test-db".into()];
    config.steps[1].services = vec!["test-db".into()];

    let source = toml::to_string_pretty(&config).expect("serialize fixture");
    let error = FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("unordered service reuse must fail");
    let report = error.report();

    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.id == "HGCFG-SHARED-SERVICE"
            && diagnostic.path == "steps[1].services[0]"
            && diagnostic
                .related
                .iter()
                .any(|related| related.path == "steps[0].services[0]")
    }));
}

#[test]
fn dependency_order_allows_service_reuse_but_not_log_reuse() {
    let mut config = repository_config();
    config.services.insert(
        "test-db".into(),
        ServiceConfig::Environment {
            source_env: "DATABASE_URL".into(),
            inject_env: "TEST_DATABASE_URL".into(),
        },
    );
    config.steps[0].services = vec!["test-db".into()];
    config.steps[1].services = vec!["test-db".into()];
    config.steps[1].depends_on = vec![config.steps[0].id.clone()];
    config.steps[1].log = config.steps[0].log.clone();

    let source = toml::to_string_pretty(&config).expect("serialize fixture");
    let error = FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("duplicate log must fail");
    let report = error.report();

    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.id == "HGCFG-DUPLICATE-LOG"));
    assert!(!report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.id == "HGCFG-SHARED-SERVICE"));
}

#[test]
fn step_logs_reject_windows_and_path_like_names_on_every_platform() {
    let mut config = repository_config();
    for value in [
        "..\\outside.log",
        "C:\\outside.log",
        "\\\\server\\share.log",
        "nested/output.log",
        "stream:output.log",
    ] {
        config.steps[0].log = value.into();
        let error = config
            .validate()
            .expect_err("path-like log name must fail closed");
        assert!(error.to_string().contains("log"), "{value}: {error:#}");
    }
}

#[test]
fn duplicate_logs_use_a_conservative_case_normalized_identity() {
    let mut config = repository_config();
    config.steps[0].log = "Unit-tests.log".into();
    config.steps[1].log = "unit-tests.log".into();

    let source = toml::to_string_pretty(&config).expect("serialize fixture");
    let error = FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("case-insensitive log collision must fail");
    assert!(error
        .report()
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.id == "HGCFG-DUPLICATE-LOG"));
}

#[test]
fn independent_services_with_the_same_injection_are_rejected() {
    let mut config = repository_config();
    config.services.insert(
        "test-db".into(),
        ServiceConfig::Environment {
            source_env: "DATABASE_URL".into(),
            inject_env: "TEST_SERVICE_URL".into(),
        },
    );
    config.services.insert(
        "test-cache".into(),
        ServiceConfig::Environment {
            source_env: "CACHE_URL".into(),
            inject_env: "TEST_SERVICE_URL".into(),
        },
    );
    config.steps[0].services = vec!["test-db".into()];
    config.steps[1].services = vec!["test-cache".into()];

    let source = toml::to_string_pretty(&config).expect("serialize fixture");
    let error = FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("injection conflict must fail");
    assert!(error
        .report()
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.id == "HGCFG-SERVICE-INJECT-COLLISION"));
}

#[test]
fn repeated_service_references_emit_one_shared_resource_diagnostic_per_pair() {
    let mut config = repository_config();
    config.services.insert(
        "test-db".into(),
        ServiceConfig::Environment {
            source_env: "DATABASE_URL".into(),
            inject_env: "TEST_DATABASE_URL".into(),
        },
    );
    config.steps[0].services = vec!["test-db".into()];
    config.steps[1].services = vec!["test-db".into()];

    let source = toml::to_string_pretty(&config).expect("serialize fixture");
    let error = FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("unordered service reuse must fail");
    let shared = error
        .report()
        .diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.id == "HGCFG-SHARED-SERVICE")
        .collect::<Vec<_>>();
    assert_eq!(shared.len(), 1);
}

#[test]
fn transitive_dependency_allows_service_reuse() {
    let mut config = repository_config();
    config.services.insert(
        "test-db".into(),
        ServiceConfig::Environment {
            source_env: "DATABASE_URL".into(),
            inject_env: "TEST_DATABASE_URL".into(),
        },
    );
    config.steps[0].services = vec!["test-db".into()];
    config.steps[2].services = vec!["test-db".into()];
    config.steps[1].depends_on = vec![config.steps[0].id.clone()];
    config.steps[2].depends_on = vec![config.steps[1].id.clone()];

    let source = toml::to_string_pretty(&config).expect("serialize fixture");
    FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect("transitive dependency must order service reuse");
}

#[test]
fn independent_semantic_errors_are_aggregated_with_precise_paths() {
    let mut config = repository_config();
    config.steps[0].timeout_secs = 0;
    config.steps[1].parser = Some("missing-parser".into());

    let source = toml::to_string_pretty(&config).expect("serialize fixture");
    let error = FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("independent configuration errors must be aggregated");
    let report = error.report();

    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.path == "steps[0].timeout_secs"
            && diagnostic.id == "HGCFG-INVALID-FIELD"
            && diagnostic.location.is_some()
    }));
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.path == "steps[1].parser"
            && diagnostic.id == "HGCFG-UNKNOWN-REFERENCE"
            && diagnostic.location.is_some()
    }));
}

#[test]
fn human_diagnostic_location_has_one_closing_parenthesis() {
    let mut config = repository_config();
    config.steps[0].timeout_secs = 0;
    let source = toml::to_string_pretty(&config).expect("serialize fixture");

    let error = FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("timeout must fail validation");
    let rendered = error.to_string();
    assert!(rendered.contains("(line "), "{rendered}");
    assert!(!rendered.contains("))"), "{rendered}");
}

#[test]
fn diagnostics_truncate_at_the_documented_maximum() {
    let mut diagnostics = ConfigDiagnostics::empty();
    for index in 0..51 {
        diagnostics.push(ConfigDiagnostic {
            id: format!("HGCFG-TEST-{index}"),
            severity: DiagnosticSeverity::Error,
            path: format!("steps[{index}]"),
            message: "test diagnostic".into(),
            help: "test help".into(),
            location: None,
            related: Vec::new(),
        });
    }

    let report = diagnostics.report();
    assert_eq!(report.diagnostics.len(), 50);
    assert!(report.truncated);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.id == "HGCFG-DIAGNOSTICS-TRUNCATED"));
}

#[test]
fn discovery_errors_keep_configuration_field_paths_in_json_reports() {
    let error = anyhow::anyhow!("required secret scan configuration is missing: /tmp/secrets.toml");
    let report = report_for_error(&error);
    assert!(!report.valid);
    assert_eq!(report.diagnostics[0].id, "HGCFG-REQUIRED-FILE");
    assert_eq!(report.diagnostics[0].path, "paths.secrets_config");
}

#[test]
fn template_paths_must_stay_inside_a_disjoint_repository_root() {
    let workspace = TestWorkspace::new("template-config");
    fs::create_dir_all(workspace.root.join("templates")).expect("create template root");
    fs::write(workspace.root.join("templates/report.tera"), "report").expect("write template");
    let path = workspace.root.join("flow.toml");
    let source = format!(
        "{}\n[report_templates]\nroot = \"templates\"\ntemplate = \"templates/report.tera\"\n",
        include_str!("../../presets/rust-api.flow.toml")
    );
    fs::write(&path, source).expect("write config");
    FlowConfig::load_with_diagnostics(&path, Some(&workspace.root)).expect("safe template config");

    fs::write(
        &path,
        format!(
            "{}\n[report_templates]\nroot = \".harness-gate/reports\"\ntemplate = \".harness-gate/reports/report.tera\"\n",
            include_str!("../../presets/rust-api.flow.toml")
        ),
    )
    .expect("write overlapping config");
    fs::create_dir_all(workspace.root.join(".harness-gate/reports"))
        .expect("create report directory");
    fs::write(
        workspace.root.join(".harness-gate/reports/report.tera"),
        "report",
    )
    .expect("write report template");
    let error = FlowConfig::load_with_diagnostics(&path, Some(&workspace.root))
        .expect_err("report overlap must fail");
    assert!(error
        .report()
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.id == "HGCFG-TEMPLATE-PATH"));
}

#[test]
fn windows_prefixed_paths_are_rejected_on_every_platform() {
    let source = include_str!("../../presets/rust-api.flow.toml").replace(
        "reports = \".harness-gate/reports\"",
        "reports = \"C:\\\\reports\"",
    );
    let error = FlowConfig::from_source(&source).expect_err("Windows prefix must fail");
    assert!(error.to_string().contains("paths.reports"));
}

#[test]
fn windows_style_parent_traversal_is_rejected_on_every_platform() {
    let source = include_str!("../../presets/rust-api.flow.toml").replace(
        "reports = \".harness-gate/reports\"",
        "reports = \"..\\\\outside\"",
    );
    let error = FlowConfig::from_source(&source).expect_err("Windows traversal must fail");
    assert!(error.to_string().contains("paths.reports"));
}

#[cfg(unix)]
#[test]
fn template_symlink_escape_is_rejected() {
    use std::os::unix::fs::symlink;

    let workspace = TestWorkspace::new("template-symlink");
    let outside = TestWorkspace::new("template-outside");
    fs::create_dir_all(workspace.root.join("templates")).expect("create template root");
    fs::write(outside.root.join("outside.tera"), "outside").expect("write outside template");
    symlink(
        outside.root.join("outside.tera"),
        workspace.root.join("templates/escape.tera"),
    )
    .expect("link outside template");
    let path = workspace.root.join("flow.toml");
    fs::write(
        &path,
        format!(
            "{}\n[report_templates]\nroot = \"templates\"\ntemplate = \"templates/escape.tera\"\n",
            include_str!("../../presets/rust-api.flow.toml")
        ),
    )
    .expect("write config");

    let error = FlowConfig::load_with_diagnostics(&path, Some(&workspace.root))
        .expect_err("symlink escape must fail");
    assert!(error
        .report()
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.id == "HGCFG-TEMPLATE-PATH"));
}

#[test]
fn junit_report_path_must_stay_relative_to_the_report_directory() {
    let source = format!(
        "{}\n[report_templates]\njunit = \"../junit.xml\"\n",
        include_str!("../../presets/rust-api.flow.toml")
    );
    let error = FlowConfig::from_source(&source).expect_err("JUnit traversal must fail");
    assert!(error.to_string().contains("report_templates.junit"));
}

#[test]
fn webhook_configuration_requires_http_url_and_an_enabled_result() {
    let invalid_scheme = format!(
        "{}\n[[notifications.webhooks]]\nurl = \"ftp://example.test/hook\"\n",
        include_str!("../../presets/rust-api.flow.toml")
    );
    let error = FlowConfig::from_source(&invalid_scheme).expect_err("FTP webhook must fail");
    assert!(error.to_string().contains("notifications"));

    let disabled = format!(
        "{}\n[[notifications.webhooks]]\nurl = \"https://example.test/hook\"\non_failure = false\non_success = false\n",
        include_str!("../../presets/rust-api.flow.toml")
    );
    let error = FlowConfig::from_source(&disabled).expect_err("disabled webhook must fail");
    assert!(error.to_string().contains("notifications"));
}
