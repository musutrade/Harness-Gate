//! Stable-interface collection candidate. No gate policy or migration authority.
mod artifact;
mod collect;
mod process;
mod source;
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
        ["collect", request] => println!("{}", collect::collect(Path::new(request))?),
        ["prepare", project, output, doctor] => println!("{}", collect::prepare(Path::new(project), Path::new(output), Path::new(doctor))?),
        ["verify", directory, anchor, request_digest] => {
            collect::verify(Path::new(directory), anchor, request_digest)?;
            println!("{}", json!({"schema":"rust-stable-verification/v1", "integrity":"verified", "core_acceptance":"pending"}));
        }
        _ => bail!("usage: harness-gate-rust-stable-collector --version | doctor PROJECT NEW_OUTPUT | prepare PROJECT NEW_OUTPUT DOCTOR.json | collect REQUEST.json | verify OUTPUT MANIFEST_SHA256 REQUEST_SHA256"),
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
