mod common;

use common::*;
use std::process::Command;

#[test]
fn test_help_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("--help")
        .output()
        .expect("Failed to execute command");

    assert_success(&output);
    let stdout = stdout_str(&output);
    assert!(stdout.contains("harness-gate"));
    assert!(stdout.contains("Usage:"));
}

#[test]
fn test_version_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("--version")
        .output()
        .expect("Failed to execute command");

    assert_success(&output);
    let stdout = stdout_str(&output);
    assert!(stdout.contains("harness-gate"));
    assert!(stdout.contains("0.1.0"));
}

#[test]
fn test_invalid_subcommand() {
    let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("invalid-subcommand")
        .output()
        .expect("Failed to execute command");

    assert_failure(&output);
    let stderr = stderr_str(&output);
    assert!(stderr.contains("unrecognized subcommand") || stderr.contains("unexpected argument"));
}

#[test]
fn test_presets_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("presets")
        .output()
        .expect("Failed to execute command");

    assert_success(&output);
    let stdout = stdout_str(&output);

    // Should list available presets
    assert!(stdout.contains("rust-api") || stdout.contains("Available presets"));
}

#[test]
fn test_init_with_invalid_preset() {
    let ctx = TestContext::new();
    let output = ctx.run_harness_gate(&["init", "--preset", "nonexistent-preset"]);

    assert_failure(&output);
    let stderr = stderr_str(&output);
    assert!(
        stderr.contains("preset") || stderr.contains("not found") || stderr.contains("unknown")
    );
}

#[test]
fn test_verify_without_project() {
    let ctx = TestContext::new();
    let output = ctx.run_harness_gate(&["verify", "--all"]);

    assert_failure(&output);
    let stderr = stderr_str(&output);
    // Should error about missing config or not being in a project
    assert!(
        stderr.contains("not found")
            || stderr.contains("No such file")
            || stderr.contains("config")
    );
}

#[test]
fn test_config_check_without_config() {
    let ctx = TestContext::new();
    let output = ctx.run_harness_gate(&["config", "check"]);

    // Should fail when no config exists
    assert_failure(&output);
    // Error message format varies by platform, just verify it failed
}

#[test]
fn test_scope_without_git() {
    let ctx = TestContext::new();
    assert_success(&ctx.run_harness_gate(&["init", "--preset", "generic"]));
    let output = ctx.run_harness_gate(&["scope"]);

    assert_failure(&output);
    assert!(stderr_str(&output).contains("ERROR [E1301]"));
}

#[test]
fn test_audit_reports_a_typed_configuration_error() {
    let ctx = TestContext::new();
    assert_success(&ctx.run_harness_gate(&["init", "--preset", "generic"]));
    ctx.write_file(
        ".harness-gate/audit.toml",
        "version = 999\n[engine]\nignore_filename = \".gitignore\"\n",
    );

    let output = ctx.run_harness_gate(&["audit"]);

    assert_failure(&output);
    assert!(stderr_str(&output).contains("ERROR [E1101]"));
}

#[test]
fn test_secrets_reports_a_typed_configuration_error() {
    let ctx = TestContext::new();
    assert_success(&ctx.run_harness_gate(&["init", "--preset", "generic"]));
    ctx.init_git();
    ctx.write_file(
        ".harness-gate/secrets.toml",
        "version = 2\nrules = []\n[placeholders]\nminimum_unique_characters = 4\nmaximum_nonalphanumeric_characters = 2\nprefixes = []\nmarkers = []\nexact = []\n",
    );

    let output = ctx.run_harness_gate(&["secrets"]);

    assert_failure(&output);
    assert!(stderr_str(&output).contains("ERROR [E1201]"));
}

#[test]
fn test_verify_reports_a_typed_selection_error() {
    let ctx = TestContext::new();
    assert_success(&ctx.run_harness_gate(&["init", "--preset", "generic"]));

    let output = ctx.run_harness_gate(&["verify", "--all", "--profile", "missing"]);

    assert_failure(&output);
    assert!(stderr_str(&output).contains("ERROR [E1401]"));
}

#[test]
fn test_doctor_basic() {
    let ctx = TestContext::new();

    // Create minimal config to avoid error
    ctx.write_file(
        ".harness-gate/flow.toml",
        r#"
version = 2
[scope]
default = []
"#,
    );

    let output = ctx.run_harness_gate(&["doctor"]);

    // Doctor should run (might report issues but shouldn't crash)
    // We don't assert success because it depends on system state
    let stderr = stderr_str(&output);
    assert!(!stderr.contains("panic") && !stderr.contains("thread panicked"));
}

#[test]
fn test_color_always_styles_human_readable_output() {
    let ctx = TestContext::new();
    ctx.write_file(
        ".harness-gate/flow.toml",
        "version = 2\n[scope]\ndefault = []\n",
    );

    let output = ctx.run_harness_gate(&["--color", "always", "doctor"]);

    assert!(stdout_str(&output).contains("\x1b[") || stderr_str(&output).contains("\x1b["));
}

#[test]
fn test_color_never_keeps_human_readable_output_plain() {
    let ctx = TestContext::new();
    ctx.write_file(
        ".harness-gate/flow.toml",
        "version = 2\n[scope]\ndefault = []\n",
    );

    let output = ctx.run_harness_gate(&["--color", "never", "doctor"]);

    assert!(!stdout_str(&output).contains("\x1b["));
    assert!(!stderr_str(&output).contains("\x1b["));
}
