//! Workflow composition over configured contracts. No ecosystem owns this path.
use crate::config::quality::{baseline, collectors, compiler, QualityConfig, ReportFormat};
use crate::process::adapter::{HostPolicy, TrustedKey};
use crate::{project::Project, scope::ScopeResult};
use anyhow::{ensure, Context, Result};
use harness_gate::quality::{evidence::ValidationContext, parse, policy, project_report};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeSet, fs, path::Path};

pub(super) struct Prepared {
    state: compiler::TrustedState,
    keys: Vec<TrustedKey>,
    baseline: Option<baseline::Request>,
    output: String,
    formats: BTreeSet<ReportFormat>,
}

#[derive(Debug, Serialize)]
pub struct QualityResult {
    pub schema: &'static str,
    pub status: &'static str,
    pub phase: &'static str,
    pub error: Option<String>,
    pub selection: Value,
    pub inputs: Value,
    pub evidence: Value,
    pub baseline: Value,
    pub project_report: Value,
    pub project_report_path: Option<String>,
    pub output: Option<String>,
    pub formats: BTreeSet<ReportFormat>,
    pub evaluation_time: Option<String>,
}

impl QualityResult {
    pub(super) fn passed(&self) -> bool {
        self.status == "pass"
    }

    fn pending() -> Self {
        Self {
            schema: "quality-verification/v1",
            status: "blocked",
            phase: "configuration",
            error: None,
            selection: Value::Null,
            inputs: Value::Null,
            evidence: Value::Null,
            baseline: Value::Null,
            project_report: Value::Null,
            project_report_path: None,
            output: None,
            formats: BTreeSet::new(),
            evaluation_time: None,
        }
    }
}

fn read<T: for<'de> Deserialize<'de>>(root: &Path, name: &str) -> Result<T> {
    let root = root
        .canonicalize()
        .context("resolve quality workflow root")?;
    let path =
        crate::project::resolve_repo_path(&root, Path::new(name), "quality workflow input", true)?;
    Ok(serde_json::from_value(parse(&fs::read_to_string(path)?)?)?)
}

/// Snapshot trust inputs before execution. Recompilation after collection detects
/// changes to pinned configuration, subjects, and artifacts made by any step.
pub(super) fn prepare(
    project: &Project,
    scope: &ScopeResult,
    profile: &str,
) -> Result<Option<Prepared>> {
    let root = &project.execution_root;
    let Some(config) = QualityConfig::load_optional(root, &project.config)? else {
        return Ok(None);
    };
    let participation = config
        .profiles
        .get(profile)
        .context("quality profile is not declared")?;
    let workflow = participation
        .workflow
        .as_ref()
        .context("quality profile requires workflow state and trusted_keys")?;
    let state: compiler::TrustedState = read(root, &workflow.state)?;
    ensure!(
        state.profile == profile,
        "quality state profile differs from verify profile"
    );
    compiler::compile(root, &state)?;
    validate_selection(&config, &state, scope)?;
    Ok(Some(Prepared {
        state,
        keys: read(root, &workflow.trusted_keys)?,
        baseline: workflow
            .baseline_request
            .as_ref()
            .map(|name| read(root, name))
            .transpose()?,
        output: config.reporting.output,
        formats: config.reporting.formats,
    }))
}

fn validate_selection(
    config: &QualityConfig,
    state: &compiler::TrustedState,
    scope: &ScopeResult,
) -> Result<()> {
    let changed = config
        .subjects
        .iter()
        .filter(|(alias, subject)| {
            let component = &config.components[&subject.component];
            !component.flow_components.is_disjoint(&scope.components)
                && (scope.mode == "all"
                    || scope.mode == "components"
                    || state.subjects[*alias]
                        .iter()
                        .any(|s| scope.changed_files.contains(&s.path)))
        })
        .map(|(alias, _)| alias.clone())
        .collect::<BTreeSet<_>>();
    // Signed collector requests include this selection. Never rewrite a signed
    // request to expand or narrow it after the host has authenticated it.
    ensure!(state.selection.as_ref().and_then(|s| s.get("changed_subject")) == Some(&changed),
        "quality changed-subject selection differs from verify scope; prepare fresh signed workflow inputs");
    Ok(())
}

pub(super) fn run(project: &Project, prepared: Result<Option<Prepared>>) -> Option<QualityResult> {
    let mut result = QualityResult::pending();
    let work = match prepared {
        Ok(None) => return None,
        Ok(Some(work)) => work,
        Err(error) => {
            result.error = Some(crate::utils::redaction::redact_text(&format!("{error:#}")));
            return Some(result);
        }
    };
    if let Err(error) = evaluate(project, work, &mut result) {
        result.status = "blocked";
        result.error = Some(crate::utils::redaction::redact_text(&format!("{error:#}")));
    }
    Some(result)
}

fn evaluate(project: &Project, work: Prepared, result: &mut QualityResult) -> Result<()> {
    let root = &project.execution_root;
    result.selection = serde_json::to_value(&work.state.selection)?;
    result.output = Some(work.output);
    result.formats = work.formats;
    result.phase = "baseline";
    let temporary = tempfile::tempdir()?;
    let base_root = temporary.path().join("baseline");
    baseline::resolve(root, &work.state, work.baseline.as_ref(), &base_root)?;
    result.baseline = read(&base_root, "resolution.json")?;
    let base_inputs: Option<Value> = if result.baseline["status"] == "available" {
        Some(read(&base_root, "inputs.json")?)
    } else {
        None
    };
    let base_records: Option<Value> = base_inputs
        .as_ref()
        .map(|_| read(&base_root, "evidence.json"))
        .transpose()?;
    result.phase = "collection";
    let collection = collectors::collect(
        root,
        &work.state,
        &HostPolicy {
            trusted_keys: work.keys,
            ..HostPolicy::default()
        },
    )?;
    result.inputs = serde_json::to_value(&collection.inputs)?;
    result.evidence = collection.evidence;
    result.phase = "evaluation";
    let inputs = collection.inputs;
    let head = ValidationContext {
        project: &inputs.project,
        expected: &inputs.expected,
        source_root: &inputs.source_root,
        artifact_root: &inputs.artifact_root,
    };
    let base = base_inputs.as_ref().map(|base| ValidationContext {
        project: &base["project"],
        expected: &base["expected"],
        source_root: Path::new(base["source_root"].as_str().expect("compiled source root")),
        artifact_root: Path::new(
            base["artifact_root"]
                .as_str()
                .expect("compiled artifact root"),
        ),
    });
    let now = chrono::Utc::now().to_rfc3339();
    result.evaluation_time = Some(now.clone());
    let evaluated = policy::evaluate(
        &inputs.policy,
        &result.evidence,
        &head,
        &policy::EvaluationOptions {
            selection: inputs.selection.as_ref(),
            base_records: base_records.as_ref(),
            base_context: base.as_ref(),
            mappings: inputs.mappings.as_ref(),
            exceptions: inputs.exceptions.as_ref(),
            now: Some(&now),
        },
    )?;
    result.project_report = project_report::report(&evaluated, &inputs.project, &inputs.policy)?;
    result.status = if result.project_report["aggregate"]["state"] == "pass" {
        "pass"
    } else {
        "fail"
    };
    result.phase = "complete";
    result.project_report_path = Some(
        project
            .reports
            .join("quality-project-report.json")
            .to_string_lossy()
            .into_owned(),
    );
    if base_inputs.is_some() {
        result.baseline["inputs"] = json!(base_inputs);
        result.baseline["evidence"] = json!(base_records);
        // Retain immutable base sources for direct evaluator replay. The report
        // names this host-owned directory so artifact retention can remove it.
        result.baseline["retained_directory"] = json!(temporary.keep());
    }
    Ok(())
}

pub(super) fn diagnostics(result: &QualityResult) -> String {
    let mut text = format!("Quality: {} ({})\n", result.status, result.phase);
    if let Some(error) = &result.error {
        text.push_str(&format!("{error}\n"));
    }
    if let Some(gates) = result.project_report["gates"].as_object() {
        for gate in gates.values().filter(|gate| gate["state"] != "pass") {
            let record = &gate["record"];
            text.push_str(&format!("{}: {} — {}; component={} subject={} metric={} base={} head={} threshold={} ratchet={} outcome={} evidence={} remediation={}\n",
                gate["policy"], gate["state"], gate["reason"], record["component"], record["subject"], record["metric"],
                record["base"], record["head"], record["policy"]["limit"], record["policy"]["ratchet"],
                record["ratchet"], record["evidence_links"], record["remediation_classes"]));
        }
    }
    crate::utils::redaction::redact_text(&text)
}

/// Configured copies carry the same invocation identity and combined decision.
pub(super) fn publish(report: &super::VerificationReport, project: &Project) -> Result<()> {
    let Some(quality) = &report.quality else {
        return Ok(());
    };
    let Some(output) = &quality.output else {
        return Ok(());
    };
    let mut files = Vec::new();
    if quality.formats.contains(&ReportFormat::Json) {
        files.push("test_result.json");
        if !quality.project_report.is_null() {
            files.push("quality-project-report.json");
        }
    }
    if quality.formats.contains(&ReportFormat::Human) {
        files.push("test_result.md");
    }
    for file in files {
        crate::utils::fs::confined_atomic_write(
            &project.root,
            &Path::new(output).join(&report.invocation_id).join(file),
            fs::read(Path::new(&report.report_directory).join(file))?,
            true,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::*;

    #[cfg(unix)]
    #[test]
    fn workflow_input_accepts_aliased_root_but_rejects_escape() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("root");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("resolution.json"), r#"{"status":"available"}"#).unwrap();
        let alias = directory.path().join("alias");
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        let value: Value = read(&alias, "resolution.json").unwrap();
        assert_eq!(value["status"], "available");
        fs::write(directory.path().join("outside.json"), "{}").unwrap();
        std::os::unix::fs::symlink(
            directory.path().join("outside.json"),
            root.join("escape.json"),
        )
        .unwrap();
        assert!(read::<Value>(&alias, "escape.json")
            .unwrap_err()
            .to_string()
            .contains("escapes the repository"));
    }
}
