use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use harness_gate::quality::{evidence::ValidationContext, parse, policy, project_report};
use serde_json::Value;
use std::{fs, path::PathBuf};

#[derive(Debug, Subcommand)]
pub(crate) enum QualityAction {
    /// Evaluate generic evidence and produce the authoritative project decision in Rust.
    Evaluate(Box<EvaluateArgs>),
}

#[derive(Debug, Args)]
pub(crate) struct EvaluateArgs {
    /// Trusted harness-project/v1 model.
    #[arg(long)]
    project: PathBuf,
    /// Trusted harness-policy/v1 rules.
    #[arg(long)]
    policy: PathBuf,
    /// Collected harness-evidence/v1 records (JSON array).
    #[arg(long)]
    evidence: PathBuf,
    /// Trusted expected source, collector and measurement-series context.
    #[arg(long)]
    expected: PathBuf,
    #[arg(long)]
    source_root: PathBuf,
    #[arg(long)]
    artifact_root: PathBuf,
    /// Optional trusted selection, subject mappings and exception metadata.
    #[arg(long)]
    selection: Option<PathBuf>,
    #[arg(long)]
    mappings: Option<PathBuf>,
    #[arg(long)]
    exceptions: Option<PathBuf>,
    /// All five base arguments must be supplied together for baseline comparison.
    #[arg(long, requires_all = ["base_project", "base_expected", "base_source_root", "base_artifact_root"])]
    base_evidence: Option<PathBuf>,
    #[arg(long, requires = "base_evidence")]
    base_project: Option<PathBuf>,
    #[arg(long, requires = "base_evidence")]
    base_expected: Option<PathBuf>,
    #[arg(long, requires = "base_evidence")]
    base_source_root: Option<PathBuf>,
    #[arg(long, requires = "base_evidence")]
    base_artifact_root: Option<PathBuf>,
    /// Trusted RFC3339 evaluation clock; defaults to current UTC time.
    #[arg(long)]
    now: Option<String>,
    /// Destination for the existing harness-project-report/v1 contract.
    #[arg(long)]
    output: PathBuf,
}

fn read(path: &PathBuf) -> Result<Value> {
    parse(&fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?)
        .with_context(|| format!("parse {}", path.display()))
}

pub(crate) fn run(action: &QualityAction) -> Result<bool> {
    let QualityAction::Evaluate(args) = action;
    // Clear an earlier report before reading inputs: errors must not leave a stale pass.
    // Refuse aliases of input documents before touching the destination.
    let paths = [
        Some(&args.project),
        Some(&args.policy),
        Some(&args.evidence),
        Some(&args.expected),
        args.selection.as_ref(),
        args.mappings.as_ref(),
        args.exceptions.as_ref(),
        args.base_evidence.as_ref(),
        args.base_project.as_ref(),
        args.base_expected.as_ref(),
    ];
    if args.output.exists() {
        let destination = args.output.canonicalize()?;
        for input in paths.into_iter().flatten() {
            anyhow::ensure!(
                !input.canonicalize().is_ok_and(|path| path == destination),
                "output aliases an input"
            );
        }
        fs::remove_file(&args.output).context("remove stale generic report")?;
    }
    let project = read(&args.project)?;
    let rules = read(&args.policy)?;
    let records = read(&args.evidence)?;
    let expected = read(&args.expected)?;
    let selection = args.selection.as_ref().map(read).transpose()?;
    let mappings = args.mappings.as_ref().map(read).transpose()?;
    let exceptions = args.exceptions.as_ref().map(read).transpose()?;
    let base_records = args.base_evidence.as_ref().map(read).transpose()?;
    let base_project = args.base_project.as_ref().map(read).transpose()?;
    let base_expected = args.base_expected.as_ref().map(read).transpose()?;
    let head = ValidationContext {
        project: &project,
        expected: &expected,
        source_root: &args.source_root,
        artifact_root: &args.artifact_root,
    };
    let base = base_project.as_ref().map(|project| ValidationContext {
        project,
        expected: base_expected
            .as_ref()
            .expect("clap requires complete base context"),
        source_root: args
            .base_source_root
            .as_ref()
            .expect("clap requires base source root"),
        artifact_root: args
            .base_artifact_root
            .as_ref()
            .expect("clap requires base artifact root"),
    });
    let now = args
        .now
        .clone()
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    chrono::DateTime::parse_from_rfc3339(&now).context("invalid evaluation clock")?;
    let result = policy::evaluate(
        &rules,
        &records,
        &head,
        &policy::EvaluationOptions {
            selection: selection.as_ref(),
            base_records: base_records.as_ref(),
            base_context: base.as_ref(),
            mappings: mappings.as_ref(),
            exceptions: exceptions.as_ref(),
            now: Some(&now),
        },
    )?;
    let report = project_report::report(&result, &project, &rules)?;
    crate::utils::fs::atomic_write(
        &args.output,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
        true,
    )?;
    Ok(report["aggregate"]["state"] == "pass")
}
