use super::command::{isolate_process_tree, terminate};
use super::signal::cancelled;
use anyhow::{bail, Context, Result};
use std::io::Read;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct CapturedOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub fn capture(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
) -> Result<CapturedOutput> {
    capture_command(program, args, cwd, timeout, true)
}

pub fn capture_cleanup(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
) -> Result<CapturedOutput> {
    capture_command(program, args, cwd, timeout, false)
}

fn capture_command(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
    observe_cancel: bool,
) -> Result<CapturedOutput> {
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    isolate_process_tree(&mut command);
    let mut child = command
        .spawn()
        .with_context(|| format!("start internal command {program}"))?;
    let stdout = child
        .stdout
        .take()
        .context("internal command stdout was not captured")?;
    let stderr = child
        .stderr
        .take()
        .context("internal command stderr was not captured")?;
    let stdout_reader = thread::spawn(move || read_all(stdout));
    let stderr_reader = thread::spawn(move || read_all(stderr));
    let started = Instant::now();

    let (status, timed_out, was_cancelled) = loop {
        if let Some(status) = child.try_wait()? {
            break (status, false, false);
        }
        if observe_cancel && cancelled() {
            break (terminate(&mut child)?, false, true);
        }
        if started.elapsed() >= timeout {
            break (terminate(&mut child)?, true, false);
        }
        thread::sleep(Duration::from_millis(50));
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| anyhow::anyhow!("internal command stdout reader panicked"))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| anyhow::anyhow!("internal command stderr reader panicked"))??;

    if was_cancelled {
        bail!("internal command {program} was cancelled");
    }
    if timed_out {
        bail!(
            "internal command {program} timed out after {} ms",
            timeout.as_millis()
        );
    }
    Ok(CapturedOutput {
        status,
        stdout,
        stderr,
    })
}

fn read_all(mut reader: impl Read) -> std::io::Result<Vec<u8>> {
    let mut output = Vec::new();
    reader.read_to_end(&mut output)?;
    Ok(output)
}
