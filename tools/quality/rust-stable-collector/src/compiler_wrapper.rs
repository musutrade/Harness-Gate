//! Cargo's public RUSTC_WRAPPER protocol, implemented by this same executable.
use crate::{artifact, tools::Tools};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

pub const CONTEXT: &str = "HARNESS_GATE_COMPILER_CAPTURE";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Invocation {
    rustc: PathBuf,
    cwd: PathBuf,
    args: Vec<String>,
    exit_code: Option<i32>,
}

// Only the observed stable Cargo invocation form is accepted. These are public
// rustc output options; neither Cargo fingerprint files nor compiler IR is read.
fn dep_info(args: &[String]) -> Result<Option<PathBuf>> {
    let mut values = BTreeMap::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        ensure!(
            !arg.starts_with("-Z") && !arg.starts_with('@'),
            "unsupported compiler argument"
        );
        let (key, value) = if ["--emit", "--out-dir", "--crate-name", "-C"].contains(&arg.as_str())
        {
            (
                arg.as_str(),
                iter.next()
                    .context("compiler option missing value")?
                    .as_str(),
            )
        } else if let Some(value) = arg.strip_prefix("--emit=") {
            ("--emit", value)
        } else {
            continue;
        };
        if key == "-C" {
            if let Some(value) = value.strip_prefix("extra-filename=") {
                ensure!(
                    values.insert("suffix", value).is_none(),
                    "duplicate compiler suffix"
                );
            }
        } else {
            ensure!(
                values.insert(key, value).is_none(),
                "duplicate compiler output option"
            );
        }
    }
    let Some(emit) = values.get("--emit") else {
        return Ok(None);
    };
    ensure!(
        !emit.contains('='),
        "explicit compiler output paths unsupported"
    );
    if !emit.split(',').any(|v| v == "dep-info") {
        return Ok(None);
    }
    let name = values
        .get("--crate-name")
        .context("compiler crate name missing")?;
    let suffix = values.get("suffix").copied().unwrap_or("");
    ensure!(
        !name.is_empty()
            && name
                .chars()
                .chain(suffix.chars())
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-')),
        "invalid compiler output name"
    );
    let directory = Path::new(
        values
            .get("--out-dir")
            .context("compiler output directory missing")?,
    );
    ensure!(
        directory.is_absolute(),
        "compiler output directory must be absolute"
    );
    Ok(Some(directory.join(format!("{name}{suffix}.d"))))
}

pub fn run(root: &Path) -> Result<i32> {
    let tools: Tools = serde_json::from_slice(&fs::read(root.join("tools.json"))?)?;
    let mut args = env::args().skip(1);
    let rustc = PathBuf::from(args.next().context("wrapper compiler missing")?);
    ensure!(
        rustc == tools.rustc.path && artifact::identity(&rustc)? == tools.rustc.identity,
        "wrapper compiler identity differs"
    );
    let args: Vec<_> = args.collect();
    dep_info(&args)?;
    let cwd = env::current_dir()?.canonicalize()?;
    let status = Command::new(&rustc).args(&args).status()?;
    let invocation = Invocation {
        rustc,
        cwd,
        args,
        exit_code: status.code(),
    };
    let mut file = tempfile::NamedTempFile::new_in(root.join("compiler-invocations"))?;
    file.write_all(&serde_json::to_vec(&invocation)?)?;
    file.as_file().sync_all()?;
    file.keep()?;
    Ok(status.code().unwrap_or(1))
}

pub fn directories(
    root: &Path,
    scratch: &Path,
    rustc: &Path,
) -> Result<BTreeMap<PathBuf, PathBuf>> {
    let mut result = BTreeMap::new();
    let files = artifact::inventory(&root.join("compiler-invocations"), false)?;
    ensure!(files.len() <= 10000, "too many compiler invocations");
    for (name, identity) in files {
        ensure!(
            identity.bytes <= 1024 * 1024,
            "compiler invocation too large"
        );
        let invocation: Invocation = serde_json::from_value(crate::strict_json::parse(
            &fs::read(root.join("compiler-invocations").join(name))?,
        )?)?;
        ensure!(
            invocation.rustc == rustc && invocation.exit_code == Some(0),
            "compiler invocation identity or exit differs"
        );
        ensure!(
            invocation.cwd.is_absolute() && invocation.cwd.canonicalize()? == invocation.cwd,
            "compiler cwd is not canonical"
        );
        if let Some(path) = dep_info(&invocation.args)? {
            ensure!(
                path.starts_with(scratch),
                "compiler dep-info outside scratch"
            );
            ensure!(
                result.insert(path, invocation.cwd).is_none(),
                "duplicate compiler dep-info producer"
            );
        }
    }
    ensure!(!result.is_empty(), "compiler dep-info producers missing");
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_output_options_identify_the_dep_info_file() {
        let args = [
            "--crate-name",
            "a",
            "src/lib.rs",
            "--emit=dep-info,link",
            "--out-dir",
            "/fresh/deps",
            "-C",
            "extra-filename=-abc",
        ]
        .map(str::to_owned);
        assert_eq!(
            dep_info(&args).unwrap(),
            Some(PathBuf::from("/fresh/deps/a-abc.d"))
        );
        for args in [
            vec!["-Zunpretty=mir"],
            vec!["@hidden"],
            vec!["--emit=dep-info=/elsewhere"],
        ] {
            assert!(dep_info(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
    }
}
