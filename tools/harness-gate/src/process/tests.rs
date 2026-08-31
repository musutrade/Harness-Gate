use super::{capture, Task};
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
#[test]
fn timeout_terminates_the_task() {
    let log = std::env::temp_dir().join(format!("harness-gate-timeout-{}.log", std::process::id()));
    let result = Task::new("timeout fixture", "sleep", Path::new("."), log.clone())
        .args(["5"])
        .timeout(0)
        .run()
        .expect("run timeout fixture");

    assert!(!result.passed);
    assert!(result.timed_out);
    assert!(log.is_file());
    let _ = fs::remove_file(log);
}

#[cfg(unix)]
#[test]
fn task_can_remove_an_inherited_environment_variable() {
    let log = std::env::temp_dir().join(format!("harness-gate-env-{}.log", std::process::id()));
    let result = Task::new("environment fixture", "env", Path::new("."), log.clone())
        .env("HARNESS_GATE_REMOVE_FIXTURE", "must-not-leak")
        .env_remove("HARNESS_GATE_REMOVE_FIXTURE")
        .run()
        .expect("run environment fixture");

    assert!(result.passed);
    let output = fs::read_to_string(&log).expect("read environment log");
    assert!(!output.contains("HARNESS_GATE_REMOVE_FIXTURE"));
    let _ = fs::remove_file(log);
}

#[cfg(target_os = "linux")]
#[test]
fn task_runs_in_an_isolated_session() {
    let log = std::env::temp_dir().join(format!("harness-gate-session-{}.log", std::process::id()));
    let result = Task::new("session fixture", "sh", Path::new("."), log.clone())
        .args(["-c", "ps -o sid= -p $$"])
        .run()
        .expect("run session fixture");

    assert!(result.passed);
    let child_session = fs::read_to_string(&log)
        .expect("read session log")
        .trim()
        .parse::<i32>()
        .expect("parse child session id");
    let parent_session = unsafe { libc::getsid(0) };
    assert_ne!(child_session, parent_session);
    let _ = fs::remove_file(log);
}

#[cfg(unix)]
#[test]
fn captured_command_has_a_hard_timeout() {
    let args = vec!["-c".to_string(), "sleep 5".to_string()];
    let error = capture("sh", &args, Path::new("/tmp"), Duration::from_millis(100))
        .expect_err("capture must time out");

    assert!(error.to_string().contains("timed out"));
}

/// The test binary doubles as a child-process fixture. Keeping the fixture in
/// Rust avoids relying on `sleep`, `sh`, or other platform-specific utilities.
#[test]
fn process_tree_child_fixture() {
    if env::var_os("HARNESS_GATE_PROCESS_TREE_MARKER").is_none() {
        return;
    }
    std::thread::sleep(Duration::from_secs(2));
    let marker = env::var_os("HARNESS_GATE_PROCESS_TREE_MARKER").expect("marker path");
    fs::write(marker, b"descendant survived").expect("write process-tree marker");
}

#[test]
fn process_tree_parent_fixture() {
    if env::var_os("HARNESS_GATE_PROCESS_TREE_MARKER").is_none() {
        return;
    }
    let executable = env::current_exe().expect("test executable");
    let status = Command::new(executable)
        .args([
            "--exact",
            "process::tests::process_tree_child_fixture",
            "--nocapture",
        ])
        .envs(env::vars_os().filter(|(key, _)| key != "HARNESS_GATE_PROCESS_TREE_PARENT"))
        .status()
        .expect("start descendant fixture");
    // The parent remains alive until the task timeout kills the process tree.
    // If the descendant exits early, keep the parent alive long enough for the
    // task runner to exercise its termination path anyway.
    let _ = status;
    std::thread::sleep(Duration::from_secs(10));
}

#[test]
fn timeout_terminates_process_tree_without_a_descendant_leak() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    let suffix = format!("{}-{timestamp}", std::process::id());
    let log = env::temp_dir().join(format!("harness-gate-process-tree-{suffix}.log"));
    let marker = env::temp_dir().join(format!("harness-gate-process-tree-{suffix}.marker"));
    let executable = env::current_exe().expect("test executable");
    let result = Task::new(
        "process tree fixture",
        executable,
        Path::new("."),
        log.clone(),
    )
    .args([
        "--exact",
        "process::tests::process_tree_parent_fixture",
        "--nocapture",
    ])
    .env("HARNESS_GATE_PROCESS_TREE_MARKER", &marker)
    .env("HARNESS_GATE_PROCESS_TREE_PARENT", "1")
    .timeout(1)
    .run()
    .expect("run process tree fixture");

    assert!(result.timed_out, "fixture must reach the timeout boundary");
    assert!(!result.passed);
    // The child fixture writes after two seconds. Waiting beyond that deadline
    // makes a surviving descendant observable on every supported platform.
    std::thread::sleep(Duration::from_secs(3));
    assert!(!marker.exists(), "timed-out task left a descendant process");
    let _ = fs::remove_file(log);
    let _ = fs::remove_file(marker);
}

#[test]
fn abnormal_or_cancelled_worker_removes_isolation_state() {
    let root = tempfile::tempdir().expect("temporary isolation root");
    let state = root.path().join("worker.json");
    fs::write(&state, b"{\"worker\":true}").expect("seed isolation state");
    let log = root.path().join("worker.log");
    let executable = env::current_exe().expect("test executable");
    let result = Task::new("abnormal worker", executable, Path::new("."), log)
        .args(["--exact", "process::tests::process_tree_child_fixture"])
        .timeout(0)
        .isolation_state(state.clone())
        .run()
        .expect("run worker fixture");
    assert!(!result.passed);
    assert!(
        !state.exists(),
        "terminal worker state must not be reusable"
    );
    assert!(state.with_extension("terminal.json").is_file());
}
