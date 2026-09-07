use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_harness-gate"));
    command.arg("--project-root").arg(root);
    command
}

fn success(output: Output) {
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn fixture() -> TempDir {
    let root = TempDir::new().unwrap();
    success(
        command(root.path())
            .args(["init", "--preset", "generic"])
            .output()
            .unwrap(),
    );
    success(
        Command::new("git")
            .arg("init")
            .current_dir(root.path())
            .output()
            .unwrap(),
    );
    fs::write(
        root.path().join("probe.sh"),
        "fixture root must be a directory",
    )
    .unwrap();
    root
}

#[test]
fn compatibility_cli_preserves_comparison_and_rollback_evidence() {
    let root = TempDir::new().unwrap();
    let old = root.path().join("old.json");
    let new = root.path().join("new.json");
    let comparison = root.path().join("comparison.json");
    let state = root.path().join("canary.json");
    fs::write(&old, r#"{"status":"pass"}"#).unwrap();
    for (value, equivalent) in [("pass", true), ("fail", false)] {
        fs::write(&new, format!(r#"{{"status":"{value}"}}"#)).unwrap();
        let output = command(root.path())
            .args(["compat", "compare", "--old"])
            .arg(&old)
            .arg("--new")
            .arg(&new)
            .arg("--output")
            .arg(&comparison)
            .output()
            .unwrap();
        assert_eq!(output.status.success(), equivalent);
        let report: Value = serde_json::from_slice(&fs::read(&comparison).unwrap()).unwrap();
        assert_eq!(report["equivalent"], equivalent);
        assert_eq!(
            report["differences"].as_array().unwrap().is_empty(),
            equivalent
        );
    }
    success(
        command(root.path())
            .args(["compat", "canary", "--slice", "ci", "--state"])
            .arg(&state)
            .output()
            .unwrap(),
    );
    let enabled: Value = serde_json::from_slice(&fs::read(&state).unwrap()).unwrap();
    assert_eq!(enabled["enabled"], true);
    success(
        command(root.path())
            .args(["compat", "rollback", "--state"])
            .arg(&state)
            .output()
            .unwrap(),
    );
    let disabled: Value = serde_json::from_slice(&fs::read(&state).unwrap()).unwrap();
    assert_eq!(disabled["enabled"], false);
    assert_eq!(disabled["history"].as_array().unwrap().len(), 2);
}

#[test]
fn config_cli_discovers_nested_project_and_rejects_file_root() {
    let root = fixture();
    let nested = root.path().join("nested");
    fs::create_dir(&nested).unwrap();
    for config in [None, Some(root.path().join(".harness-gate/flow.toml"))] {
        let mut cli = Command::new(env!("CARGO_BIN_EXE_harness-gate"));
        cli.current_dir(&nested);
        if let Some(path) = config {
            cli.arg("--config").arg(path);
        }
        success(cli.args(["config", "check"]).output().unwrap());
    }
    let output = command(&root.path().join("probe.sh"))
        .args(["config", "check"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not a directory"));
}
