//! Stable-interface collection candidate. No gate policy or migration authority.
mod adapter;
mod artifact;
mod collect;
mod process;
mod release;
mod source;
mod strict_json;
mod support;
mod tools;

use anyhow::{bail, Result};
use serde_json::json;
use std::{env, path::Path, process::ExitCode};

fn run() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["--version"] => println!("harness-gate-rust-stable-collector {}", env!("CARGO_PKG_VERSION")),
        ["doctor", project, output] => {
            let project = Path::new(project).canonicalize()?;
            let mut runner = process::Runner::create(Path::new(output), &project, 30)?;
            let tools = tools::discover(&mut runner)?;
            artifact::write(&runner.root.join("doctor.json"), &tools)?;
            println!("{}", serde_json::to_string(&tools)?);
        }
        ["release-verify", bundle, trust, digest, log] => println!("{}", release::verify(Path::new(bundle), Path::new(trust), digest, Path::new(log))?),
        ["install", bundle, trust, digest, root, log] => println!("{}", release::install(Path::new(bundle), Path::new(trust), digest, Path::new(root), Path::new(log))?),
        ["rollback", root, version, trust, digest, log] => println!("{}", release::rollback(Path::new(root), version, Path::new(trust), digest, Path::new(log))?),
        ["adapter", "--binding", path, "--binding-sha256", digest] => println!("{}", adapter::run(Path::new(path), digest)?),
        ["describe", root, anchor, request_digest] => println!("{}", adapter::describe(Path::new(root), anchor, request_digest)?),
        ["collect", request] => println!("{}", collect::collect(Path::new(request))?),
        ["prepare", project, output, doctor] => println!("{}", collect::prepare(Path::new(project), Path::new(output), Path::new(doctor))?),
        ["verify", directory, anchor, request_digest] => {
            collect::verify(Path::new(directory), anchor, request_digest)?;
            println!("{}", json!({"schema":"rust-stable-verification/v1", "integrity":"verified", "core_acceptance":"pending"}));
        }
        _ => bail!("usage: harness-gate-rust-stable-collector --version | doctor PROJECT NEW_OUTPUT | prepare PROJECT NEW_OUTPUT DOCTOR.json | collect REQUEST.json | verify OUTPUT MANIFEST_SHA256 REQUEST_SHA256 | describe OUTPUT MANIFEST_SHA256 REQUEST_SHA256 | adapter --binding FILE --binding-sha256 SHA256 | release-verify BUNDLE TRUST TRUST_SHA256 NEW_LOG | install BUNDLE TRUST TRUST_SHA256 ROOT NEW_LOG | rollback ROOT INVENTORY_SHA256 TRUST TRUST_SHA256 NEW_LOG"),
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!(
                "{}",
                json!({"schema":"rust-stable-error/v1", "state":"measurement_error", "message":format!("{error:#}")})
            );
            ExitCode::FAILURE
        }
    }
}
