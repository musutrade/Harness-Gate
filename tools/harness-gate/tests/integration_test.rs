mod common;

use common::*;

#[test]
fn test_init_with_rust_api_preset() {
    let ctx = TestContext::new();

    // Run init with rust-api preset
    let output = ctx.run_harness_gate(&["init", "--preset", "rust-api"]);
    assert_success(&output);

    // Verify config files were created
    assert!(ctx.file_exists(".harness-gate/flow.toml"));
    assert!(ctx.file_exists(".harness-gate/secrets.toml"));
    assert!(ctx.file_exists(".harness-gate/audit.toml"));

    // Verify flow.toml content
    let flow_content = ctx.read_file(".harness-gate/flow.toml");
    assert!(flow_content.contains("version = 2"));
    assert!(flow_content.contains("[scope]"));
}

#[test]
fn test_config_check_valid() {
    let ctx = TestContext::new();

    // Initialize with preset
    let output = ctx.run_harness_gate(&["init", "--preset", "rust-api"]);
    assert_success(&output);

    // Run config check
    let output = ctx.run_harness_gate(&["config", "check"]);
    assert_success(&output);

    let stdout = stdout_str(&output);
    assert!(stdout.contains("valid") || stdout.contains("OK") || stdout.contains("✓"));
}

#[test]
fn test_config_check_invalid() {
    let ctx = TestContext::new();

    // Create invalid config
    ctx.write_file(".harness-gate/flow.toml", "invalid toml content {{");

    let output = ctx.run_harness_gate(&["config", "check"]);
    assert_failure(&output);

    let stderr = stderr_str(&output);
    assert!(stderr.contains("parse") || stderr.contains("invalid") || stderr.contains("error"));
}

#[test]
fn test_presets_lists_available() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("presets")
        .output()
        .expect("Failed to execute command");

    assert_success(&output);
    let stdout = stdout_str(&output);

    // Should list at least rust-api preset
    assert!(stdout.contains("rust-api"));
}

#[test]
fn test_secrets_scan_basic() {
    let ctx = TestContext::new();

    // Initialize with preset
    ctx.run_harness_gate(&["init", "--preset", "rust-api"]);

    // Initialize git repo
    ctx.init_git();

    // Create a file with no secrets
    ctx.write_file("test.txt", "Hello, world!");

    // Add and commit
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&ctx.project_root)
        .output()
        .ok();

    // Run secrets scan
    let output = ctx.run_harness_gate(&["secrets", "--staged"]);

    // Should succeed (no secrets found)
    assert_success(&output);
}

#[test]
fn test_scope_detection_all() {
    let ctx = TestContext::new();

    // Initialize
    ctx.run_harness_gate(&["init", "--preset", "rust-api"]);

    // Initialize git
    ctx.init_git();

    // Run scope with --all
    let output = ctx.run_harness_gate(&["scope", "--all"]);

    // Should succeed and show components
    assert_success(&output);
}

#[test]
fn test_verify_fails_without_git() {
    let ctx = TestContext::new();

    // Initialize but don't create git repo
    ctx.run_harness_gate(&["init", "--preset", "rust-api"]);

    // Try to verify
    let output = ctx.run_harness_gate(&["verify"]);

    // Should fail because no git repo
    assert_failure(&output);
}

#[test]
fn test_init_creates_gitignore_entry() {
    let ctx = TestContext::new();

    // Run init
    let output = ctx.run_harness_gate(&["init", "--preset", "rust-api"]);
    assert_success(&output);

    // Check if .gitignore was created or updated
    if ctx.file_exists(".gitignore") {
        let gitignore = ctx.read_file(".gitignore");
        assert!(gitignore.contains(".harness-gate/reports") || gitignore.contains("reports"));
    }
}

#[test]
fn test_init_twice_fails() {
    let ctx = TestContext::new();

    // First init should succeed
    let output = ctx.run_harness_gate(&["init", "--preset", "rust-api"]);
    assert_success(&output);

    // Second init should fail (already initialized)
    let output = ctx.run_harness_gate(&["init", "--preset", "rust-api"]);
    assert_failure(&output);

    let stderr = stderr_str(&output);
    assert!(stderr.contains("already") || stderr.contains("exists"));
}
