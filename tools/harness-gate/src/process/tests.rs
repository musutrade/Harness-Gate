use super::*;
use std::fs;
use std::path::Path;
use std::time::Duration;

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
