use crate::artifact;
use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    env, fs,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const OUTPUT_LIMIT: u64 = 16 * 1024 * 1024;

pub struct Runner {
    pub root: PathBuf,
    pub project: PathBuf,
    pub environment: BTreeMap<String, String>,
    timeout: Duration,
    sequence: usize,
}

impl Runner {
    pub fn create(root: &Path, project: &Path, seconds: u64) -> Result<Self> {
        ensure!(
            (1..=3600).contains(&seconds),
            "timeout must be 1..3600 seconds"
        );
        // Reject inherited compiler injection instead of making it invisible.
        for key in [
            "RUSTC_BOOTSTRAP",
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "RUSTC",
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
            "RUSTDOCFLAGS",
            "CARGO_ENCODED_RUSTDOCFLAGS",
        ] {
            ensure!(
                env::var_os(key).is_none(),
                "unsupported environment {key}: unset {key} before collection"
            );
        }
        fs::create_dir(root)
            .with_context(|| format!("output must be a new directory: {}", root.display()))?;
        let root = root.canonicalize()?;
        ensure!(
            !root.starts_with(project),
            "output must be outside the measured project"
        );
        fs::create_dir(root.join("commands"))?;
        let mut environment = BTreeMap::new();
        for key in [
            "PATH",
            "HOME",
            "CARGO_HOME",
            "RUSTUP_HOME",
            "RUSTUP_TOOLCHAIN",
            "TMPDIR",
        ] {
            if let Ok(value) = env::var(key) {
                environment.insert(key.to_owned(), value);
            }
        }
        environment.insert("LANG".into(), "C.UTF-8".into());
        environment.insert("CARGO_NET_OFFLINE".into(), "true".into());
        environment.insert("RUSTUP_AUTO_INSTALL".into(), "0".into());
        environment.insert("CARGO_INCREMENTAL".into(), "0".into());
        Ok(Self {
            root,
            project: project.to_owned(),
            environment,
            timeout: Duration::from_secs(seconds),
            sequence: 0,
        })
    }

    pub fn run(
        &mut self,
        program: &Path,
        args: &[String],
        extra: &BTreeMap<String, String>,
    ) -> Result<String> {
        self.sequence += 1;
        let prefix = self
            .root
            .join("commands")
            .join(format!("{:04}", self.sequence));
        let stdout = prefix.with_extension("stdout");
        let stderr = prefix.with_extension("stderr");
        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(&self.project)
            .env_clear()
            .envs(&self.environment)
            .envs(extra)
            .stdin(Stdio::null())
            .stdout(fs::File::create(&stdout)?)
            .stderr(fs::File::create(&stderr)?)
            .process_group(0);
        let started = Instant::now();
        let mut record = json!({"program": program, "args": args, "cwd": self.project, "environment": self.environment, "extra_environment": extra});
        let result = (|| -> Result<String> {
            let mut child = command
                .spawn()
                .with_context(|| format!("cannot run {}", program.display()))?;
            let pid = child.id() as i32;
            let outcome = (|| loop {
                if let Some(status) = child.try_wait()? {
                    break Ok(status);
                }
                if started.elapsed() >= self.timeout {
                    break Err(anyhow::anyhow!("tool timeout"));
                }
                if fs::metadata(&stdout)?.len() + fs::metadata(&stderr)?.len() > OUTPUT_LIMIT {
                    break Err(anyhow::anyhow!("tool output limit"));
                }
                thread::sleep(Duration::from_millis(20));
            })();
            // Kill descendants even when the leader has already exited. No pipe
            // readers can be kept alive past the deadline by inherited handles.
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
            let _ = child.wait();
            let status = outcome?;
            record["exit_code"] = json!(status.code());
            ensure!(
                fs::metadata(&stdout)?.len() + fs::metadata(&stderr)?.len() <= OUTPUT_LIMIT,
                "tool output limit"
            );
            if !status.success() {
                bail!(
                    "tool exited {status}: {}; see {}",
                    program.display(),
                    stderr.display()
                );
            }
            Ok(fs::read_to_string(&stdout)?)
        })();
        record["duration_ms"] = json!(started.elapsed().as_millis() as u64);
        record["error"] = result
            .as_ref()
            .err()
            .map_or(Value::Null, |e| json!(format!("{e:#}")));
        artifact::write(&prefix.with_extension("json"), &record)?;
        result
    }

    pub fn text(&mut self, program: &Path, args: &[&str]) -> Result<String> {
        self.run(
            program,
            &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            &BTreeMap::new(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nonzero_and_timeout_never_return_tool_output() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir(&project).unwrap();
        let mut runner = Runner::create(&dir.path().join("capture"), &project, 1).unwrap();
        assert!(runner
            .text(
                Path::new("/bin/sh"),
                &["-c", "echo forged-success; exit 17"]
            )
            .unwrap_err()
            .to_string()
            .contains("17"));
        let start = Instant::now();
        assert!(runner
            .text(Path::new("/bin/sh"), &["-c", "sleep 60 & wait"])
            .unwrap_err()
            .to_string()
            .contains("timeout"));
        assert!(start.elapsed() < Duration::from_secs(5));
        assert!(runner.text(Path::new("/nonexistent/tool"), &[]).is_err());
        assert!(runner.root.join("commands/0003.json").is_file());
    }
}
