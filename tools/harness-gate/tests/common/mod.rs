use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

/// Test context providing isolated environment for integration tests
pub struct TestContext {
    #[allow(dead_code)]
    pub temp_dir: TempDir,
    pub project_root: PathBuf,
}

impl TestContext {
    /// Create a new test context with a temporary directory
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let project_root = temp_dir.path().to_path_buf();
        Self {
            temp_dir,
            project_root,
        }
    }

    /// Run harness-gate with the given arguments
    pub fn run_harness_gate(&self, args: &[&str]) -> Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_harness-gate"));
        cmd.args(args)
            .arg("--project-root")
            .arg(&self.project_root)
            .output()
            .expect("Failed to execute harness-gate")
    }

    /// Run harness-gate without project-root argument
    #[allow(dead_code)]
    pub fn run_harness_gate_raw(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_harness-gate"))
            .args(args)
            .current_dir(&self.project_root)
            .output()
            .expect("Failed to execute harness-gate")
    }

    /// Write a file relative to project root
    pub fn write_file(&self, path: impl AsRef<Path>, content: &str) {
        let full_path = self.project_root.join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).expect("Failed to create parent dirs");
        }
        std::fs::write(full_path, content).expect("Failed to write file");
    }

    /// Read a file relative to project root
    #[allow(dead_code)]
    pub fn read_file(&self, path: impl AsRef<Path>) -> String {
        let full_path = self.project_root.join(path);
        std::fs::read_to_string(full_path).expect("Failed to read file")
    }

    /// Check if a file exists relative to project root
    #[allow(dead_code)]
    pub fn file_exists(&self, path: impl AsRef<Path>) -> bool {
        self.project_root.join(path).exists()
    }

    /// Initialize a Git repository in the project root
    #[allow(dead_code)]
    pub fn init_git(&self) {
        Command::new("git")
            .args(&["init"])
            .current_dir(&self.project_root)
            .output()
            .expect("Failed to init git repo");

        // Configure git user for commits
        Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .current_dir(&self.project_root)
            .output()
            .ok();

        Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(&self.project_root)
            .output()
            .ok();
    }
}

/// Helper to assert command succeeded
pub fn assert_success(output: &Output) {
    if !output.status.success() {
        eprintln!("Command failed!");
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("Command exited with status: {}", output.status);
    }
}

/// Helper to assert command failed
pub fn assert_failure(output: &Output) {
    if output.status.success() {
        eprintln!("Command unexpectedly succeeded!");
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        panic!("Expected command to fail");
    }
}

/// Helper to get stdout as string
pub fn stdout_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Helper to get stderr as string
pub fn stderr_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}
