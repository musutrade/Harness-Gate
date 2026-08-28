use super::*;
use std::collections::BTreeSet;

fn repository_config() -> FlowConfig {
    toml::from_str(include_str!("../../presets/rust-api.flow.toml")).expect("parse fixture")
}

#[test]
fn repository_configuration_is_valid() {
    repository_config().validate().expect("validate config");
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
