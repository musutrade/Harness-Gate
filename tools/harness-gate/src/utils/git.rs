use crate::process::CapturedOutput;
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::time::Duration;

const GIT_TIMEOUT: Duration = Duration::from_secs(30);

/// Run Git in a project root with the standard Harness-Gate timeout.
pub fn capture<I, S>(project_root: &Path, args: I) -> Result<CapturedOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args = command_args(args);
    crate::process::capture("git", &args, project_root, GIT_TIMEOUT)
        .with_context(|| format!("run git {}", args.join(" ")))
}

/// Return NUL-delimited Git path output as UTF-8 repository-relative paths.
pub fn null_terminated_paths<I, S>(project_root: &Path, args: I) -> Result<Vec<String>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args = command_args(args);
    let output = capture(project_root, &args)?;
    if !output.status.success() {
        bail!("git {} failed", args.join(" "));
    }
    output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            String::from_utf8(entry.to_vec()).context("Git returned a non-UTF-8 file path")
        })
        .collect()
}

/// Read a path from the staged Git snapshot when it exists.
pub fn staged_file(project_root: &Path, file: &str) -> Result<Option<Vec<u8>>> {
    let args = vec!["show".to_string(), format!(":{file}")];
    let output = capture(project_root, args).with_context(|| format!("read staged file {file}"))?;
    Ok(output.status.success().then_some(output.stdout))
}

fn command_args<I, S>(args: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter()
        .map(|arg| arg.as_ref().to_string())
        .collect()
}
