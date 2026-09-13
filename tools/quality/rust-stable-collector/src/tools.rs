use crate::{
    artifact::{self, FileIdentity},
    process::Runner,
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    env,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    pub path: PathBuf,
    pub identity: FileIdentity,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tools {
    pub rustc: Tool,
    pub cargo: Tool,
    pub rustdoc: Tool,
    pub cargo_llvm_cov: Tool,
    pub llvm_cov: Tool,
    pub llvm_profdata: Tool,
    pub release: String,
    pub commit: String,
    pub host: String,
    pub llvm: String,
}

fn locate(name: &str, hint: &str) -> Result<PathBuf> {
    for directory in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        let path = directory.join(name);
        if path.is_file() {
            return Ok(path.canonicalize()?);
        }
    }
    anyhow::bail!("missing {name}; install explicitly: {hint}")
}

fn field(text: &str, key: &str) -> Result<String> {
    Ok(text
        .lines()
        .find_map(|line| line.strip_prefix(key))
        .context(format!("missing tool version field {key}"))?
        .trim()
        .to_owned())
}

fn probe(runner: &mut Runner, path: PathBuf, args: &[&str]) -> Result<Tool> {
    let before = artifact::identity(&path)?;
    let version = runner.text(&path, args)?;
    ensure!(
        before == artifact::identity(&path)?,
        "tool changed during probe"
    );
    Ok(Tool {
        path,
        identity: before,
        version: version.trim().into(),
    })
}

pub fn discover(runner: &mut Runner) -> Result<Tools> {
    ensure!(
        cfg!(all(target_os = "linux", target_arch = "x86_64")),
        "unsupported collector platform; Linux x86_64 candidate only"
    );
    // Invoke the rustup proxy by its name, not its canonical rustup filename.
    // Selection is performed in the project directory and respects its pin.
    let version = runner.text(Path::new("rustc"), &["-vV"])
        .context("Rust required; install the project's supported stable toolchain with rustup toolchain install VERSION --profile minimal")?;
    let release = field(&version, "release: ")?;
    ensure!(["1.97.1", "1.98.1"].contains(&release.as_str()), "unsupported Rust {release}; candidate interface matrix is 1.97.1 / 1.98.1; no toolchain was changed");
    let host = field(&version, "host: ")?;
    ensure!(
        host == "x86_64-unknown-linux-gnu",
        "unsupported target {host}"
    );
    let commit = field(&version, "commit-hash: ")?;
    let llvm = field(&version, "LLVM version: ")?;
    let sysroot = PathBuf::from(
        runner
            .text(Path::new("rustc"), &["--print", "sysroot"])?
            .trim(),
    );
    let rustc = probe(runner, sysroot.join("bin/rustc"), &["-vV"])?;
    ensure!(rustc.version == version.trim(), "rustc selection changed");
    let cargo = probe(runner, sysroot.join("bin/cargo"), &["--version"])?;
    let rustdoc = probe(runner, sysroot.join("bin/rustdoc"), &["--version"])?;
    let directory = sysroot.join("lib/rustlib").join(&host).join("bin");
    for name in ["llvm-cov", "llvm-profdata"] {
        ensure!(directory.join(name).is_file(), "missing {name} for Rust {release}; run: rustup component add llvm-tools-preview --toolchain {release}");
    }
    let llvm_cov = probe(runner, directory.join("llvm-cov"), &["--version"])?;
    let llvm_profdata = probe(runner, directory.join("llvm-profdata"), &["--version"])?;
    for tool in [&llvm_cov, &llvm_profdata] {
        let actual = tool
            .version
            .lines()
            .find_map(|l| l.trim().strip_prefix("LLVM version "))
            .context("unrecognized LLVM version output")?
            .trim();
        ensure!(actual == llvm || actual == format!("{llvm}-rust-{release}-stable"), "LLVM mismatch: Rust uses {llvm}, {} reports {actual}; run: rustup component add llvm-tools-preview --toolchain {release}", tool.path.display());
    }
    let cargo_llvm_cov = probe(
        runner,
        locate(
            "cargo-llvm-cov",
            "cargo install cargo-llvm-cov --version 0.9.0 --locked",
        )?,
        &["llvm-cov", "--version"],
    )?;
    ensure!(
        cargo_llvm_cov.version == "cargo-llvm-cov 0.9.0",
        "unsupported cargo-llvm-cov; run: cargo install cargo-llvm-cov --version 0.9.0 --locked"
    );
    Ok(Tools {
        rustc,
        cargo,
        rustdoc,
        cargo_llvm_cov,
        llvm_cov,
        llvm_profdata,
        release,
        commit,
        host,
        llvm,
    })
}
