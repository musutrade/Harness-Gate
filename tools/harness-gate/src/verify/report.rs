use super::VerificationReport;
use crate::config::WebhookConfig;
use crate::failure::FailureCode;
use crate::net_policy::{is_local_only, normalize_host};
use crate::project::Project;
use crate::service::ResourceLease;
use crate::utils::redaction::{redact_text, REDACTION_TEXT_LIMIT};
use anyhow::{bail, Context, Result};
use serde::{ser::Serializer, Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::NextTimeout;

static INVOCATION_COUNTER: AtomicU64 = AtomicU64::new(1);
pub(super) const MACHINE_RESULT_SCHEMA_VERSION: &str = "1";
pub(super) const ARTIFACT_MANIFEST_SCHEMA_VERSION: &str = "1";
const ARTIFACT_REGISTRY_SCHEMA_VERSION: &str = "1";
const ARTIFACT_REGISTRY_FILE: &str = "artifact-registry.json";
const MANIFEST_FILE: &str = "manifest.json";
const MACHINE_RESULT_FILE: &str = "test_result.json";
const MAX_RETAINED_INVOCATIONS: usize = 50;
const MAX_INVOCATION_EVIDENCE_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Serialize)]
struct MachineResult {
    schema_version: &'static str,
    invocation_id: String,
    executor_version: String,
    report_directory: String,
    timestamp: String,
    profile: String,
    input_mode: String,
    project_identity: String,
    source_identity: String,
    execution_root: String,
    configuration_digest: String,
    scope: crate::scope::ScopeResult,
    services: Vec<MachineService>,
    steps: Vec<MachineStep>,
    skipped_steps: Vec<MachineSkippedStep>,
    warnings: Vec<MachineWarning>,
    failures: Vec<MachineFailure>,
    artifacts: Vec<MachineArtifact>,
    evidence_complete: bool,
    /// Kept for consumers of the pre-schema report while `status` is adopted.
    passed: bool,
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct MachineStep {
    step_id: Option<String>,
    invocation_id: Option<String>,
    label: String,
    passed: bool,
    status: &'static str,
    timed_out: bool,
    cancelled: bool,
    duration_ms: u128,
    log: String,
    detail: Option<String>,
    failure_code: Option<String>,
    waived: bool,
    waiver: Option<crate::process::WaiverEvidence>,
    runner: Option<crate::process::RunnerExecution>,
    attempts: Vec<MachineAttempt>,
    retry_count: u32,
    flaky: bool,
    retry_class: Option<String>,
    parser: Option<MachineParser>,
}

#[derive(Debug, Serialize)]
struct MachineParser {
    mode: String,
    version: u32,
    observed: usize,
    minimum: usize,
    complete: bool,
}

#[derive(Debug, Serialize)]
struct MachineAttempt {
    attempt: u32,
    status: &'static str,
    started_at: Option<String>,
    finished_at: Option<String>,
    duration_ms: u128,
    timed_out: bool,
    cancelled: bool,
    log: String,
    detail: Option<String>,
}

#[derive(Debug, Serialize)]
struct MachineSkippedStep {
    id: String,
    label: String,
    status: &'static str,
    reason: String,
}

#[derive(Debug, Serialize)]
struct MachineWarning {
    code: String,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct MachineFailure {
    step_id: Option<String>,
    code: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct MachineArtifact {
    path: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    invocation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    step_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha256: Option<String>,
}

#[derive(Debug, Serialize)]
struct MachineService {
    id: String,
    status: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ArtifactManifest {
    schema_version: String,
    invocation_id: String,
    generated_at: String,
    artifacts: Vec<ManifestArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ArtifactRegistry {
    schema_version: String,
    invocation_id: String,
    artifacts: Vec<ArtifactBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ArtifactBinding {
    invocation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    step_id: Option<String>,
    kind: String,
    path: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Debug, Clone)]
struct EvidenceAssessment {
    bindings: Vec<ArtifactBinding>,
    failures: Vec<MachineFailure>,
}

impl EvidenceAssessment {
    fn complete(&self) -> bool {
        self.failures.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ManifestArtifact {
    path: String,
    kind: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Debug, Deserialize)]
struct MachineResultHeader {
    schema_version: String,
}

#[derive(Debug, Deserialize)]
struct InvocationMetadataRecord {
    invocation_id: String,
    input_mode: String,
    project_identity: String,
    source_identity: String,
    execution_root: String,
    configuration_digest: String,
}

#[derive(Debug, Deserialize)]
struct MachineResultEvidence {
    schema_version: String,
    invocation_id: String,
    evidence_complete: bool,
    status: String,
    artifacts: Vec<MachineArtifactEvidence>,
}

#[derive(Debug, Deserialize)]
struct MachineArtifactEvidence {
    path: String,
    kind: String,
    invocation_id: Option<String>,
    step_id: Option<String>,
    size_bytes: Option<u64>,
    sha256: Option<String>,
}

impl Serialize for VerificationReport {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        machine_result(self).serialize(serializer)
    }
}

fn machine_result(report: &VerificationReport) -> MachineResult {
    let mut artifacts = Vec::new();
    let mut evidence_complete = true;
    let mut failures = Vec::new();
    let steps = report
        .steps
        .iter()
        .map(|step| {
            let status = task_status(step);
            if step.step_id.is_none() {
                evidence_complete = false;
            }
            if !step.passed {
                failures.push(MachineFailure {
                    step_id: step.step_id.clone(),
                    code: failure_code(step).to_string(),
                    message: step
                        .detail
                        .clone()
                        .map(|detail| redact_text(&detail))
                        .unwrap_or_else(|| "verification step failed".into()),
                });
            }
            if !step.log.is_empty() {
                match invocation_relative_path(&report.report_directory, &step.log) {
                    Some(path) => artifacts.push(MachineArtifact {
                        path,
                        kind: "step-log".into(),
                        invocation_id: None,
                        step_id: None,
                        size_bytes: None,
                        sha256: None,
                    }),
                    None => {
                        evidence_complete = false;
                        failures.push(MachineFailure {
                            step_id: step.step_id.clone(),
                            code: FailureCode::EvidencePathEscape.to_string(),
                            message: format!(
                                "step log is outside invocation directory: {}",
                                step.log
                            ),
                        });
                    }
                }
            }
            let attempts = if step.attempts.is_empty() {
                vec![MachineAttempt {
                    attempt: step.attempt.unwrap_or(1),
                    status,
                    started_at: step.started_at.clone(),
                    finished_at: step.finished_at.clone(),
                    duration_ms: step.duration_ms,
                    timed_out: step.timed_out,
                    cancelled: step.cancelled,
                    log: step.log.clone(),
                    detail: step.detail.as_deref().map(redact_text),
                }]
            } else {
                step.attempts
                    .iter()
                    .map(|attempt| MachineAttempt {
                        attempt: attempt.attempt,
                        status: if attempt.cancelled {
                            "CANCELLED"
                        } else if attempt.status == "PASS" {
                            "PASS"
                        } else {
                            "FAIL"
                        },
                        started_at: attempt.started_at.clone(),
                        finished_at: attempt.finished_at.clone(),
                        duration_ms: attempt.duration_ms,
                        timed_out: attempt.timed_out,
                        cancelled: attempt.cancelled,
                        log: attempt.log.clone(),
                        detail: attempt.detail.as_deref().map(redact_text),
                    })
                    .collect()
            };
            MachineStep {
                step_id: step.step_id.clone(),
                invocation_id: step.invocation_id.clone(),
                label: step.label.clone(),
                passed: step.passed,
                status,
                timed_out: step.timed_out,
                cancelled: step.cancelled,
                duration_ms: step.duration_ms,
                log: step.log.clone(),
                detail: step.detail.as_deref().map(redact_text),
                failure_code: step
                    .failure_code
                    .map(|code| code.to_string())
                    .or_else(|| (!step.passed).then(|| failure_code(step).to_string())),
                waived: step.waived,
                waiver: step.waiver.clone().map(redact_waiver),
                runner: step.runner.clone().map(redact_runner),
                attempts,
                retry_count: step.attempts.len().saturating_sub(1) as u32,
                flaky: step.flaky,
                retry_class: step.retry_class.map(|class| class.to_string()),
                parser: step.parser.as_ref().map(|parser| MachineParser {
                    mode: parser.mode.clone(),
                    version: parser.version,
                    observed: parser.observed,
                    minimum: parser.minimum,
                    complete: parser.complete,
                }),
            }
        })
        .collect::<Vec<_>>();
    let skipped_steps = report
        .skipped_steps
        .iter()
        .map(|step| MachineSkippedStep {
            id: step.id.clone(),
            label: step.label.clone(),
            status: "SKIPPED",
            reason: redact_text(&step.reason),
        })
        .collect::<Vec<_>>();
    let status = if !evidence_complete {
        "FAIL"
    } else {
        report_status(report)
    };
    if status == "CANCELLED" {
        failures.retain(|failure| failure.code != "STEP_CANCELLED");
    }
    artifacts.sort_by(|left, right| left.path.cmp(&right.path));
    artifacts.dedup_by(|left, right| left.path == right.path);
    MachineResult {
        schema_version: MACHINE_RESULT_SCHEMA_VERSION,
        invocation_id: report.invocation_id.clone(),
        executor_version: report.executor_version.clone(),
        report_directory: report.report_directory.clone(),
        timestamp: report.timestamp.clone(),
        profile: report.profile.clone(),
        input_mode: report.input_mode.clone(),
        project_identity: report.project_identity.clone(),
        source_identity: report.source_identity.clone(),
        execution_root: report.execution_root.clone(),
        configuration_digest: report.configuration_digest.clone(),
        scope: report.scope.clone(),
        services: report
            .services
            .iter()
            .map(|service| MachineService {
                id: service.id.clone(),
                status: match service.status.as_str() {
                    "READY" => "READY",
                    "FAILED" => "FAILED",
                    "LEAKED" => "LEAKED",
                    _ => "CLEANED",
                },
            })
            .collect(),
        steps,
        skipped_steps,
        warnings: Vec::new(),
        failures,
        artifacts,
        evidence_complete,
        passed: report.passed && evidence_complete,
        status,
    }
}

fn machine_result_with_assessment(
    report: &VerificationReport,
    assessment: &EvidenceAssessment,
) -> MachineResult {
    let mut result = machine_result(report);
    let baseline_complete = result.evidence_complete;
    result.artifacts = assessment
        .bindings
        .iter()
        .map(|binding| MachineArtifact {
            path: binding.path.clone(),
            kind: binding.kind.clone(),
            invocation_id: Some(binding.invocation_id.clone()),
            step_id: binding.step_id.clone(),
            size_bytes: Some(binding.size_bytes),
            sha256: Some(binding.sha256.clone()),
        })
        .collect();
    let complete = baseline_complete && assessment.complete();
    result.evidence_complete = complete;
    result.passed = report.passed && complete;
    result.status = if !complete {
        "FAIL"
    } else {
        report_status(report)
    };
    result
        .failures
        .extend(assessment.failures.iter().cloned().map(|mut failure| {
            failure.message = redact_text(&failure.message);
            failure
        }));
    result
}

fn ensure_supported_machine_result(raw: &[u8]) -> Result<()> {
    let value: Value = serde_json::from_slice(raw).context("parse machine-result schema")?;
    let header: MachineResultHeader =
        serde_json::from_value(value.clone()).context("parse machine-result schema header")?;
    if header.schema_version != MACHINE_RESULT_SCHEMA_VERSION {
        bail!(
            "unsupported machine-result schema version {:?}",
            header.schema_version
        );
    }
    if let Some(steps) = value.get("steps").and_then(Value::as_array) {
        for (index, step) in steps.iter().enumerate() {
            validate_wire_failure_code(
                step.get("failure_code"),
                &format!("steps[{index}].failure_code"),
            )?;
            validate_wire_retry_class(
                step.get("retry_class"),
                &format!("steps[{index}].retry_class"),
            )?;
        }
    }
    if let Some(failures) = value.get("failures").and_then(Value::as_array) {
        for (index, failure) in failures.iter().enumerate() {
            validate_wire_failure_code(failure.get("code"), &format!("failures[{index}].code"))?;
        }
    }
    Ok(())
}

fn validate_wire_failure_code(value: Option<&Value>, path: &str) -> Result<()> {
    match value {
        None | Some(Value::Null) => Ok(()),
        Some(Value::String(code)) if FailureCode::try_from(code.as_str()).is_ok() => Ok(()),
        Some(Value::String(code)) => bail!("unknown failure code at {path}: {code:?}"),
        Some(_) => bail!("failure code at {path} must be a string or null"),
    }
}

fn validate_wire_retry_class(value: Option<&Value>, path: &str) -> Result<()> {
    match value {
        None | Some(Value::Null) => Ok(()),
        Some(Value::String(class))
            if crate::failure::RetryClass::try_from(class.as_str()).is_ok() =>
        {
            Ok(())
        }
        Some(Value::String(class)) => bail!("unknown retry class at {path}: {class:?}"),
        Some(_) => bail!("retry class at {path} must be a string or null"),
    }
}

fn task_status(step: &crate::process::TaskResult) -> &'static str {
    if step.cancelled {
        "CANCELLED"
    } else if step.waived {
        "WAIVED"
    } else if step.passed {
        "PASS"
    } else {
        "FAIL"
    }
}

fn failure_code(step: &crate::process::TaskResult) -> FailureCode {
    if let Some(code) = step.failure_code {
        return code;
    }
    if step.cancelled {
        FailureCode::StepCancelled
    } else if step.timed_out {
        FailureCode::StepTimeout
    } else {
        FailureCode::StepFailed
    }
}

fn report_status(report: &VerificationReport) -> &'static str {
    if report.passed && report.steps.iter().any(|step| step.waived) {
        "WAIVED"
    } else if report.passed {
        "PASS"
    } else if report.steps.iter().any(|step| step.cancelled)
        || report.skipped_steps.iter().any(|step| step.cancelled)
    {
        "CANCELLED"
    } else {
        "FAIL"
    }
}

fn invocation_relative_path(report_directory: &str, path: &str) -> Option<String> {
    let report_root = Path::new(report_directory);
    let candidate = Path::new(path);
    let relative = if candidate.is_absolute() {
        candidate.strip_prefix(report_root).ok()?.to_path_buf()
    } else {
        candidate.to_path_buf()
    };
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return None;
    }
    Some(relative.to_string_lossy().replace('\\', "/"))
}

#[derive(Debug)]
pub(super) struct Invocation {
    pub(super) id: String,
    pub(super) root: PathBuf,
    pub(super) _lease: ResourceLease,
}

#[derive(Debug, Serialize)]
struct InvocationMetadata<'a> {
    invocation_id: &'a str,
    created_at: String,
    profile: &'a str,
    staged: bool,
    input_mode: &'a str,
    project_identity: &'a str,
    source_identity: &'a str,
    execution_root: String,
    configuration_digest: &'a str,
    executor_version: &'static str,
    commit: String,
    platform: String,
    toolchain: String,
    request_id: Option<String>,
}

pub(super) fn allocate_invocation(project: &Project) -> Result<Invocation> {
    let repository = project
        .root
        .canonicalize()
        .with_context(|| format!("resolve project root {}", project.root.display()))?;
    let reports = resolve_report_root(project, &repository)?;
    let invocations = reports.join("invocations");
    fs::create_dir_all(&invocations)
        .with_context(|| format!("create invocation directory {}", invocations.display()))?;
    let resolved_invocations = invocations
        .canonicalize()
        .with_context(|| format!("resolve invocation directory {}", invocations.display()))?;
    if !resolved_invocations.starts_with(&reports) {
        bail!("invocation directory escapes report directory");
    }

    for _ in 0..32 {
        let id = next_invocation_id();
        let root = invocations.join(&id);
        match fs::create_dir(&root) {
            Ok(()) => {
                fs::create_dir_all(root.join("logs"))
                    .with_context(|| format!("create invocation logs {}", root.display()))?;
                let lease = ResourceLease::acquire(
                    project,
                    format!("invocation:{id}"),
                    "report-directory",
                    id.clone(),
                    Some(root.to_string_lossy().into_owned()),
                    None,
                )
                .with_context(|| format!("acquire invocation lease {id:?}"))?;
                return Ok(Invocation {
                    id,
                    root,
                    _lease: lease,
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("create invocation root {}", root.display()));
            }
        }
    }
    bail!("could not allocate a collision-free invocation id")
}

pub(super) fn write_invocation_metadata(
    invocation: &Invocation,
    project: &Project,
    profile: &str,
    staged: bool,
) -> Result<()> {
    write_invocation_metadata_with_request(invocation, project, profile, staged, None)
}

pub(super) fn write_invocation_metadata_with_request(
    invocation: &Invocation,
    project: &Project,
    profile: &str,
    staged: bool,
    request_id: Option<&str>,
) -> Result<()> {
    let metadata = InvocationMetadata {
        invocation_id: &invocation.id,
        created_at: chrono::Utc::now().to_rfc3339(),
        profile,
        staged,
        input_mode: project.input().mode.as_str(),
        project_identity: &project.input().project_identity,
        source_identity: &project.input().source_identity,
        execution_root: project
            .input()
            .execution_root
            .to_string_lossy()
            .into_owned(),
        configuration_digest: &project.input().configuration_digest,
        executor_version: env!("CARGO_PKG_VERSION"),
        commit: current_commit(project.repository_root()),
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        toolchain: rustc_version(),
        request_id: request_id.map(str::to_string),
    };
    let contents = serde_json::to_vec_pretty(&metadata).context("serialize invocation metadata")?;
    crate::utils::fs::confined_atomic_write(
        &invocation.root,
        Path::new("invocation.json"),
        &contents,
        false,
    )
    .map(|_| ())
}

fn current_commit(repository_root: &Path) -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repository_root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

fn rustc_version() -> String {
    std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

pub(super) fn mirror_legacy_outputs(
    invocation_project: &Project,
    legacy_project: &Project,
) -> Result<()> {
    let mut paths = vec![
        "test_result.json".to_string(),
        "test_result.md".to_string(),
        "test_result.html".to_string(),
    ];
    if let Some(path) = &legacy_project.config.report_templates.junit {
        if !paths.iter().any(|candidate| candidate == path) {
            paths.push(path.clone());
        }
    }
    for relative in paths {
        let source = invocation_project.reports.join(&relative);
        if source.is_file() {
            let contents = fs::read(&source)
                .with_context(|| format!("read invocation report {}", source.display()))?;
            crate::utils::fs::confined_atomic_write(
                &legacy_project.reports,
                Path::new(&relative),
                &contents,
                true,
            )
            .with_context(|| format!("mirror legacy report {relative}"))?;
        }
    }
    let invocation_logs = invocation_project.reports.join("logs");
    if invocation_logs.is_dir() {
        for entry in fs::read_dir(&invocation_logs)
            .with_context(|| format!("read invocation logs {}", invocation_logs.display()))?
        {
            let entry = entry.with_context(|| "read invocation log entry")?;
            let path = entry.path();
            if !entry
                .file_type()
                .with_context(|| format!("inspect invocation log {}", path.display()))?
                .is_file()
            {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let relative = format!("logs/{name}");
            let contents = fs::read(&path)
                .with_context(|| format!("read invocation log {}", path.display()))?;
            crate::utils::fs::confined_atomic_write(
                &legacy_project.reports,
                Path::new(&relative),
                &contents,
                true,
            )
            .with_context(|| format!("mirror legacy log {relative}"))?;
        }
    }
    Ok(())
}

fn next_invocation_id() -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let counter = INVOCATION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "inv-{}-{:09}-{}-{}",
        timestamp.as_secs(),
        timestamp.subsec_nanos(),
        std::process::id(),
        counter
    )
}

/// Report output boundary. The verifier produces a report model; this module
/// owns serialization and optional result delivery.
pub(super) fn write(report: &VerificationReport, project: &Project) -> Result<()> {
    redact_invocation_files(project)?;
    // Publish an explicitly incomplete machine result first. Only the closed-
    // set validation below is allowed to replace it with a complete result.
    let preliminary = EvidenceAssessment {
        bindings: Vec::new(),
        failures: vec![MachineFailure {
            step_id: None,
            code: FailureCode::EvidencePending.to_string(),
            message: "invocation evidence has not been finalized".into(),
        }],
    };
    let mut failures = Vec::new();
    match serde_json::to_string_pretty(&machine_result_with_assessment(report, &preliminary))
        .context("serialize verification report as JSON")
        .and_then(|json| {
            ensure_supported_machine_result(json.as_bytes())?;
            write_report_file(project, MACHINE_RESULT_FILE, json)
        }) {
        Ok(()) => {}
        Err(error) => failures.push(error),
    }
    if let Err(error) = write_report_file(project, "test_result.md", markdown(report)) {
        failures.push(error);
    }

    match configured_html(report, project) {
        Ok(Some(rendered)) => {
            if let Err(error) = write_report_file(project, "test_result.html", rendered) {
                failures.push(error);
            }
        }
        Ok(None) => {}
        Err(error) => failures.push(error.context("render configured HTML report")),
    }
    let templates = &project.config.report_templates;
    if let Some(path) = &templates.junit {
        if let Err(error) = write_report_file(project, path, junit(report)) {
            failures.push(error);
        }
    }
    if !failures.is_empty() {
        return finish_incomplete_report(report, project, failures);
    }

    redact_invocation_files(project)?;
    let assessment = assess_evidence(report, project)?;
    if !assessment.complete() {
        let details = format_failures(&assessment.failures);
        write_machine_result(report, project, &assessment, true)?;
        if report.passed {
            bail!("evidence finalization failed: {details}");
        }
        return Ok(());
    }

    // The assessment has already validated the closed set. Publish the
    // registry and manifest before the final machine result. That ordering
    // leaves the preliminary result incomplete if publication is interrupted
    // between control files.
    if let Err(error) = write_registry(report, project, &assessment.bindings)
        .and_then(|()| write_manifest(report, project, &assessment.bindings))
        .and_then(|()| verify_manifest_without_result(project))
        .and_then(|()| write_machine_result(report, project, &assessment, true))
        .and_then(|()| verify_manifest(project))
    {
        let failure = EvidenceAssessment {
            bindings: assessment.bindings.clone(),
            failures: vec![MachineFailure {
                step_id: None,
                code: FailureCode::EvidenceFinalizationFailure.to_string(),
                message: redact_text(&format!("{error:#}")),
            }],
        };
        let _ = write_machine_result(report, project, &failure, true);
        return Err(error);
    }
    prune_old_invocations(project, &report.invocation_id)?;
    Ok(())
}

fn finish_incomplete_report(
    report: &VerificationReport,
    project: &Project,
    failures: Vec<anyhow::Error>,
) -> Result<()> {
    let details = failures
        .iter()
        .map(|error| redact_text(&format!("{error:#}")))
        .collect::<Vec<_>>()
        .join("; ");
    let assessment = EvidenceAssessment {
        bindings: Vec::new(),
        failures: vec![MachineFailure {
            step_id: None,
            code: FailureCode::EvidencePublicationFailure.to_string(),
            message: details.clone(),
        }],
    };
    write_machine_result(report, project, &assessment, true)?;
    bail!("one or more report outputs failed: {details}");
}

fn configured_html(report: &VerificationReport, project: &Project) -> Result<Option<String>> {
    let (Some(root), Some(template)) = (
        project.config.report_templates.root.as_deref(),
        project.config.report_templates.template.as_deref(),
    ) else {
        return Ok(None);
    };
    let repository = project
        .root
        .canonicalize()
        .with_context(|| format!("resolve project root {}", project.root.display()))?;
    let root_path = confined_input_path(&repository, root, "report template root")?;
    if !root_path.is_dir() {
        bail!(
            "report template root is not a directory: {}",
            root_path.display()
        );
    }
    let template_path = confined_input_path(&repository, template, "report template")?;
    if !template_path.is_file() || !template_path.starts_with(&root_path) {
        bail!(
            "report template must be a regular file below the configured template root: {}",
            template_path.display()
        );
    }
    let template_name = template_path
        .strip_prefix(&root_path)
        .with_context(|| format!("resolve report template name {}", template_path.display()))?
        .to_string_lossy()
        .replace('\\', "/");
    if template_name.is_empty() {
        bail!("report template name is empty: {}", template_path.display());
    }
    let tera = load_templates(&root_path)?;
    let mut context = tera::Context::from_serialize(report)
        .context("serialize verification report for HTML template")?;
    context.insert("report", report);
    context.insert("summary", if report.passed { "PASS" } else { "FAIL" });
    context.insert(
        "components",
        &report
            .scope
            .components
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(","),
    );
    tera.render(&template_name, &context)
        .with_context(|| format!("render report template {template_name:?}"))
        .map(Some)
}

/// Load every template below the validated root and register it under a root-
/// relative name. Canonical checks are repeated for entries so an include or
/// inheritance target cannot read through an external symlink.
fn load_templates(root: &Path) -> Result<tera::Tera> {
    let mut tera = tera::Tera::default();
    tera.autoescape_on(vec![".html", ".htm", ".tera"]);
    let mut files = Vec::new();
    collect_template_files(root, root, &mut files)?;
    if files.is_empty() {
        bail!(
            "report template root contains no regular files: {}",
            root.display()
        );
    }
    tera.add_template_files(
        files
            .iter()
            .map(|(path, name)| (path, Some(name)))
            .collect::<Vec<_>>(),
    )
    .context("load report templates")?;
    Ok(tera)
}

fn collect_template_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<(PathBuf, String)>,
) -> Result<()> {
    for entry in fs::read_dir(directory)
        .with_context(|| format!("read report template directory {}", directory.display()))?
    {
        let entry = entry
            .with_context(|| format!("read report template entry in {}", directory.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("inspect report template entry {}", path.display()))?;
        if file_type.is_dir() {
            let resolved = path
                .canonicalize()
                .with_context(|| format!("resolve report template directory {}", path.display()))?;
            if !resolved.starts_with(root) {
                bail!(
                    "report template path escapes configured root: {}",
                    path.display()
                );
            }
            collect_template_files(root, &path, files)?;
            continue;
        }
        if !file_type.is_file() && !file_type.is_symlink() {
            continue;
        }
        let resolved = path
            .canonicalize()
            .with_context(|| format!("resolve report template {}", path.display()))?;
        if !resolved.starts_with(root) {
            bail!(
                "report template path escapes configured root: {}",
                path.display()
            );
        }
        if !resolved.is_file() {
            continue;
        }
        let name = path
            .strip_prefix(root)
            .with_context(|| format!("resolve report template name {}", path.display()))?
            .to_string_lossy()
            .replace('\\', "/");
        // Tera only needs text templates. Ignore unrelated files (images,
        // editor backups, etc.) so a template directory can contain assets
        // without making report generation fail on invalid UTF-8.
        let supported = Path::new(&name)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| matches!(extension, "html" | "htm" | "tera"));
        if supported && !name.is_empty() {
            files.push((path, name));
        }
    }
    Ok(())
}

fn confined_input_path(repository: &Path, relative: &str, label: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    if unsafe_relative_path(relative)
        || path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        bail!("{label} must be a repository-relative path: {relative:?}");
    }
    let candidate = repository.join(path);
    let resolved = candidate
        .canonicalize()
        .with_context(|| format!("resolve {label} {}", candidate.display()))?;
    if !resolved.starts_with(repository) {
        bail!("{label} escapes the project root: {}", candidate.display());
    }
    Ok(resolved)
}

fn unsafe_relative_path(value: &str) -> bool {
    value.contains('\0')
        || value.starts_with("\\\\")
        || value.starts_with("//")
        || value
            .as_bytes()
            .get(1)
            .is_some_and(|character| *character == b':')
        || value.split(['/', '\\']).any(|component| component == "..")
}

/// Resolve a report-relative output while protecting the report root from
/// traversal and pre-existing symlink escapes. Configuration validation
/// rejects traversal lexically, but this runtime check also covers callers
/// that construct a `Project` directly in tests or libraries.
fn report_target(project: &Project, relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    if unsafe_relative_path(relative)
        || path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        bail!("report output path must be a report-relative path: {relative:?}");
    }

    let repository = project
        .root
        .canonicalize()
        .with_context(|| format!("resolve project root {}", project.root.display()))?;
    // Validate the nearest existing report ancestor before creating anything.
    // This prevents a missing report path from being created through a
    // symlink that points outside the repository.
    let reports = if fs::symlink_metadata(&project.reports).is_ok() {
        project
            .reports
            .canonicalize()
            .with_context(|| format!("resolve report directory {}", project.reports.display()))?
    } else {
        let mut ancestor = project.reports.as_path();
        while fs::symlink_metadata(ancestor).is_err() {
            ancestor = ancestor
                .parent()
                .ok_or_else(|| anyhow::anyhow!("report directory has no resolvable parent"))?;
        }
        let resolved_ancestor = ancestor
            .canonicalize()
            .with_context(|| format!("resolve report directory {}", ancestor.display()))?;
        if !resolved_ancestor.starts_with(&repository) {
            bail!("report directory escapes project root");
        }
        fs::create_dir_all(&project.reports)
            .with_context(|| format!("create report directory {}", project.reports.display()))?;
        project
            .reports
            .canonicalize()
            .with_context(|| format!("resolve report directory {}", project.reports.display()))?
    };
    if !reports.starts_with(&repository) {
        bail!("report directory escapes project root");
    }
    let target = project.reports.join(path);
    let parent = target
        .parent()
        .ok_or_else(|| anyhow::anyhow!("report output has no parent directory"))?;
    let mut existing = parent;
    while fs::symlink_metadata(existing).is_err() {
        existing = existing
            .parent()
            .ok_or_else(|| anyhow::anyhow!("report output has no resolvable parent"))?;
    }
    let resolved_existing = existing
        .canonicalize()
        .with_context(|| format!("resolve report directory {}", existing.display()))?;
    if !resolved_existing.starts_with(&reports) {
        bail!("report output path escapes report directory: {relative:?}");
    }
    fs::create_dir_all(parent)
        .with_context(|| format!("create report directory {}", parent.display()))?;
    let resolved_parent = parent
        .canonicalize()
        .with_context(|| format!("resolve report directory {}", parent.display()))?;
    if !resolved_parent.starts_with(&reports) {
        bail!("report output path escapes report directory: {relative:?}");
    }
    if fs::symlink_metadata(&target).is_ok() {
        let resolved_target = target
            .canonicalize()
            .with_context(|| format!("resolve report output {}", target.display()))?;
        if !resolved_target.starts_with(&reports) {
            bail!("report output path escapes report directory: {relative:?}");
        }
    }
    Ok(target)
}

fn write_report_file(project: &Project, relative: &str, contents: impl AsRef<[u8]>) -> Result<()> {
    report_target(project, relative)?;
    crate::utils::fs::confined_atomic_write(
        &project.reports,
        Path::new(relative),
        contents.as_ref(),
        true,
    )
    .map(|_| ())
    .with_context(|| format!("write report {relative}"))
}

fn write_machine_result(
    report: &VerificationReport,
    project: &Project,
    assessment: &EvidenceAssessment,
    _final: bool,
) -> Result<()> {
    let value = machine_result_with_assessment(report, assessment);
    let contents = serde_json::to_vec_pretty(&value).context("serialize machine result")?;
    ensure_supported_machine_result(&contents)?;
    write_report_file(project, MACHINE_RESULT_FILE, contents)
}

fn write_registry(
    report: &VerificationReport,
    project: &Project,
    bindings: &[ArtifactBinding],
) -> Result<()> {
    let registry = ArtifactRegistry {
        schema_version: ARTIFACT_REGISTRY_SCHEMA_VERSION.into(),
        invocation_id: report.invocation_id.clone(),
        artifacts: bindings.to_vec(),
    };
    let contents = serde_json::to_vec_pretty(&registry).context("serialize artifact registry")?;
    write_report_file(project, ARTIFACT_REGISTRY_FILE, contents)
}

fn write_manifest(
    report: &VerificationReport,
    project: &Project,
    bindings: &[ArtifactBinding],
) -> Result<()> {
    let artifacts = bindings
        .iter()
        .map(|binding| ManifestArtifact {
            path: binding.path.clone(),
            kind: binding.kind.clone(),
            size_bytes: binding.size_bytes,
            sha256: binding.sha256.clone(),
        })
        .collect();
    let manifest = ArtifactManifest {
        schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION.into(),
        invocation_id: report.invocation_id.clone(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        artifacts,
    };
    let contents = serde_json::to_vec_pretty(&manifest).context("serialize artifact manifest")?;
    write_report_file(project, MANIFEST_FILE, contents)
}

fn manifest_kind(path: &str) -> String {
    if path.starts_with("logs/") {
        "step-log".into()
    } else if path == "invocation.json" {
        "invocation-metadata".into()
    } else if path.ends_with(".json")
        || path.ends_with(".md")
        || path.ends_with(".html")
        || path.ends_with(".xml")
    {
        "report".into()
    } else {
        "artifact".into()
    }
}

fn declare_artifact(
    root: &Path,
    declarations: &mut BTreeMap<String, (String, Option<String>, bool)>,
    failures: &mut Vec<MachineFailure>,
    path: &str,
    kind: &str,
    step_id: Option<String>,
    required: bool,
) {
    match invocation_relative_path(&root.to_string_lossy(), path) {
        Some(relative) => {
            if let Some((old_kind, old_step, old_required)) = declarations.get(&relative) {
                if old_kind != kind && old_kind != "step-log" && kind != "step-log" {
                    failures.push(MachineFailure {
                        step_id: step_id.clone(),
                        code: FailureCode::EvidenceDuplicatePath.to_string(),
                        message: format!("artifact path has conflicting declarations: {relative}"),
                    });
                } else if kind == "step-log" && old_kind != "step-log" {
                    declarations
                        .insert(relative, (kind.into(), step_id, required || *old_required));
                } else if old_step.is_none() && step_id.is_some() {
                    declarations.insert(
                        relative,
                        (old_kind.clone(), step_id, required || *old_required),
                    );
                } else if old_step.is_some() && step_id.is_some() && old_step != &step_id {
                    failures.push(MachineFailure {
                        step_id: step_id.clone(),
                        code: FailureCode::EvidenceDuplicatePath.to_string(),
                        message: format!("artifact path has conflicting step bindings: {relative}"),
                    });
                }
            } else {
                declarations.insert(relative, (kind.into(), step_id, required));
            }
        }
        None => failures.push(MachineFailure {
            step_id,
            code: FailureCode::EvidencePathEscape.to_string(),
            message: format!("artifact path is outside invocation directory: {path}"),
        }),
    }
}

fn assess_evidence(report: &VerificationReport, project: &Project) -> Result<EvidenceAssessment> {
    let root = project.reports.canonicalize().with_context(|| {
        format!(
            "resolve invocation report root {}",
            project.reports.display()
        )
    })?;
    if !root.is_dir() {
        bail!(
            "invocation report root is not a directory: {}",
            root.display()
        );
    }

    let mut declarations = BTreeMap::<String, (String, Option<String>, bool)>::new();
    let mut failures = Vec::new();

    validate_invocation_metadata(&root, report, &mut failures);

    for step in &report.steps {
        let Some(step_id) = step.step_id.clone() else {
            failures.push(MachineFailure {
                step_id: None,
                code: FailureCode::EvidenceStepUnbound.to_string(),
                message: format!("step {:?} has no invocation-bound step id", step.label),
            });
            continue;
        };
        if step.invocation_id.as_deref() != Some(report.invocation_id.as_str()) {
            failures.push(MachineFailure {
                step_id: Some(step_id.clone()),
                code: FailureCode::EvidenceInvocationMismatch.to_string(),
                message: format!(
                    "step {:?} is not bound to invocation {}",
                    step.label, report.invocation_id
                ),
            });
        }
        if !step.log.is_empty() {
            declare_artifact(
                &root,
                &mut declarations,
                &mut failures,
                &step.log,
                "step-log",
                Some(step_id.clone()),
                true,
            );
        }
        for attempt in &step.attempts {
            if !attempt.log.is_empty() {
                declare_artifact(
                    &root,
                    &mut declarations,
                    &mut failures,
                    &attempt.log,
                    "step-log",
                    Some(step_id.clone()),
                    true,
                );
            }
        }
    }

    // These are deterministic outputs owned by the invocation coordinator.
    // They are declarations, not an implicit directory listing: an unrelated
    // file under the report root remains an evidence failure.
    declare_artifact(
        &root,
        &mut declarations,
        &mut failures,
        "invocation.json",
        "invocation-metadata",
        None,
        true,
    );
    declare_artifact(
        &root,
        &mut declarations,
        &mut failures,
        "test_result.md",
        "report",
        None,
        true,
    );
    let mut optional_outputs = vec![
        "changed_files.txt".to_string(),
        "scope.json".to_string(),
        "secret_scan.json".to_string(),
        "review_context.json".to_string(),
        "review_context.md".to_string(),
        "test_result.html".to_string(),
    ];
    optional_outputs.extend(audit_report_names(project));
    optional_outputs.sort();
    optional_outputs.dedup();
    for relative in optional_outputs {
        if root.join(&relative).exists() {
            let kind = manifest_kind(&relative);
            declare_artifact(
                &root,
                &mut declarations,
                &mut failures,
                &relative,
                &kind,
                None,
                false,
            );
        }
    }
    if let Some(junit) = &project.config.report_templates.junit {
        if root.join(junit).exists() {
            declare_artifact(
                &root,
                &mut declarations,
                &mut failures,
                junit,
                "report",
                None,
                false,
            );
        }
    }
    let disk = collect_publishable_files(&root, &root, &mut failures)?;
    let declared_paths = declarations.keys().cloned().collect::<BTreeSet<_>>();
    for path in declared_paths.difference(&disk) {
        if declarations
            .get(path)
            .is_some_and(|(_, _, required)| *required)
        {
            failures.push(MachineFailure {
                step_id: declarations.get(path).and_then(|(_, step, _)| step.clone()),
                code: FailureCode::EvidenceMissing.to_string(),
                message: format!("declared artifact is missing: {path}"),
            });
        }
    }
    for path in disk.difference(&declared_paths) {
        failures.push(MachineFailure {
            step_id: None,
            code: FailureCode::EvidenceUndeclaredFile.to_string(),
            message: format!("publishable file has no declaration: {path}"),
        });
    }

    let mut bindings = Vec::new();
    for (path, (kind, step_id, _required)) in declarations {
        if !disk.contains(&path) {
            continue;
        }
        match binding_for(&root, &path, &kind, step_id.clone(), &report.invocation_id) {
            Ok(binding) => bindings.push(binding),
            Err(failure) => failures.push(failure),
        }
    }
    bindings.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(EvidenceAssessment { bindings, failures })
}

fn audit_report_names(project: &Project) -> Vec<String> {
    let Ok(source) = fs::read_to_string(&project.audit_config) else {
        return Vec::new();
    };
    let Ok(value) = toml::from_str::<toml::Value>(&source) else {
        return Vec::new();
    };
    let Some(engine) = value.get("engine").and_then(toml::Value::as_table) else {
        return Vec::new();
    };
    ["json_report_filename", "markdown_report_filename"]
        .into_iter()
        .filter_map(|field| engine.get(field).and_then(toml::Value::as_str))
        .map(str::to_string)
        .collect()
}

fn validate_invocation_metadata(
    root: &Path,
    report: &VerificationReport,
    failures: &mut Vec<MachineFailure>,
) {
    let path = root.join("invocation.json");
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            failures.push(MachineFailure {
                step_id: None,
                code: FailureCode::EvidenceSymlink.to_string(),
                message: "invocation metadata is a symbolic link".into(),
            });
            return;
        }
        Ok(metadata) if !metadata.is_file() => {
            failures.push(MachineFailure {
                step_id: None,
                code: FailureCode::EvidenceInvalidType.to_string(),
                message: "invocation metadata is not a regular file".into(),
            });
            return;
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            failures.push(MachineFailure {
                step_id: None,
                code: FailureCode::EvidenceReadFailure.to_string(),
                message: format!("inspect invocation metadata: {error}"),
            });
            return;
        }
    };
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            failures.push(MachineFailure {
                step_id: None,
                code: FailureCode::EvidenceReadFailure.to_string(),
                message: format!("read invocation metadata: {error}"),
            });
            return;
        }
    };
    let record: InvocationMetadataRecord = match serde_json::from_slice(&bytes) {
        Ok(record) => record,
        Err(error) => {
            failures.push(MachineFailure {
                step_id: None,
                code: FailureCode::EvidenceInvalidMetadata.to_string(),
                message: format!("parse invocation metadata: {error}"),
            });
            return;
        }
    };
    let expected = [
        (
            "invocation_id",
            record.invocation_id.as_str(),
            report.invocation_id.as_str(),
        ),
        (
            "input_mode",
            record.input_mode.as_str(),
            report.input_mode.as_str(),
        ),
        (
            "project_identity",
            record.project_identity.as_str(),
            report.project_identity.as_str(),
        ),
        (
            "source_identity",
            record.source_identity.as_str(),
            report.source_identity.as_str(),
        ),
        (
            "execution_root",
            record.execution_root.as_str(),
            report.execution_root.as_str(),
        ),
        (
            "configuration_digest",
            record.configuration_digest.as_str(),
            report.configuration_digest.as_str(),
        ),
    ];
    for (field, actual, expected) in expected {
        if actual != expected {
            failures.push(MachineFailure {
                step_id: None,
                code: FailureCode::EvidenceInvocationMismatch.to_string(),
                message: format!("invocation metadata field {field} does not match the report"),
            });
        }
    }
}

fn collect_publishable_files(
    root: &Path,
    directory: &Path,
    failures: &mut Vec<MachineFailure>,
) -> Result<BTreeSet<String>> {
    let mut files = BTreeSet::new();
    for entry in fs::read_dir(directory)
        .with_context(|| format!("read invocation evidence directory {}", directory.display()))?
    {
        let entry = entry.context("read invocation evidence entry")?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("inspect invocation evidence entry {}", path.display()))?;
        if metadata.file_type().is_symlink() {
            failures.push(MachineFailure {
                step_id: None,
                code: FailureCode::EvidenceSymlink.to_string(),
                message: format!(
                    "invocation evidence contains a symbolic link: {}",
                    path.display()
                ),
            });
            continue;
        }
        if metadata.is_dir() {
            let relative = path
                .strip_prefix(root)
                .with_context(|| format!("resolve invocation evidence path {}", path.display()))?
                .to_string_lossy()
                .replace('\\', "/");
            if is_internal_directory(&relative) {
                continue;
            }
            collect_publishable_files(root, &path, failures)?
                .into_iter()
                .for_each(|file| {
                    files.insert(file);
                });
            continue;
        }
        if !metadata.is_file() {
            failures.push(MachineFailure {
                step_id: None,
                code: FailureCode::EvidenceInvalidType.to_string(),
                message: format!(
                    "invocation evidence is not a regular file: {}",
                    path.display()
                ),
            });
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .with_context(|| format!("resolve invocation evidence path {}", path.display()))?
            .to_string_lossy()
            .replace('\\', "/");
        // Control files are finalized separately. A leftover temporary is
        // deliberately *not* ignored: it indicates an incomplete publication
        // and must be reported as undeclared evidence.
        if is_control_file(&relative) {
            continue;
        }
        files.insert(relative);
    }
    Ok(files)
}

fn is_control_file(relative: &str) -> bool {
    matches!(
        relative,
        MACHINE_RESULT_FILE | ARTIFACT_REGISTRY_FILE | MANIFEST_FILE
    )
}

fn is_internal_directory(relative: &str) -> bool {
    relative == "isolation"
}

fn binding_for(
    root: &Path,
    relative: &str,
    kind: &str,
    step_id: Option<String>,
    invocation_id: &str,
) -> std::result::Result<ArtifactBinding, MachineFailure> {
    let target = root.join(relative);
    let components = relative
        .split('/')
        .filter(|component| !component.is_empty());
    let mut current = root.to_path_buf();
    for component in components {
        current.push(component);
        let metadata = fs::symlink_metadata(&current).map_err(|error| MachineFailure {
            step_id: step_id.clone(),
            code: FailureCode::EvidenceMissing.to_string(),
            message: format!("inspect declared artifact {relative}: {error}"),
        })?;
        if metadata.file_type().is_symlink() {
            return Err(MachineFailure {
                step_id: step_id.clone(),
                code: FailureCode::EvidenceSymlink.to_string(),
                message: format!("declared artifact crosses a symbolic link: {relative}"),
            });
        }
        if current != target && !metadata.is_dir() {
            return Err(MachineFailure {
                step_id: step_id.clone(),
                code: FailureCode::EvidenceInvalidType.to_string(),
                message: format!("declared artifact parent is not a directory: {relative}"),
            });
        }
        if current == target && !metadata.is_file() {
            return Err(MachineFailure {
                step_id: step_id.clone(),
                code: FailureCode::EvidenceInvalidType.to_string(),
                message: format!("declared artifact is not a regular file: {relative}"),
            });
        }
    }
    let metadata = fs::metadata(&target).map_err(|error| MachineFailure {
        step_id: step_id.clone(),
        code: FailureCode::EvidenceMissing.to_string(),
        message: format!("inspect declared artifact {relative}: {error}"),
    })?;
    let sha256 = sha256_file(&target).map_err(|error| MachineFailure {
        step_id: step_id.clone(),
        code: FailureCode::EvidenceReadFailure.to_string(),
        message: format!("digest declared artifact {relative}: {error:#}"),
    })?;
    Ok(ArtifactBinding {
        invocation_id: invocation_id.into(),
        step_id,
        kind: kind.into(),
        path: relative.into(),
        size_bytes: metadata.len(),
        sha256,
    })
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file =
        fs::File::open(path).with_context(|| format!("open artifact {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("read artifact {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn verify_manifest_without_result(project: &Project) -> Result<()> {
    let path = project.reports.join(MANIFEST_FILE);
    let bytes =
        fs::read(&path).with_context(|| format!("read artifact manifest {}", path.display()))?;
    let manifest: ArtifactManifest =
        serde_json::from_slice(&bytes).context("parse artifact manifest")?;
    if manifest.schema_version != ARTIFACT_MANIFEST_SCHEMA_VERSION {
        bail!(
            "unsupported artifact manifest schema version {:?}",
            manifest.schema_version
        );
    }
    let registry_path = project.reports.join(ARTIFACT_REGISTRY_FILE);
    let registry: ArtifactRegistry = serde_json::from_slice(
        &fs::read(&registry_path)
            .with_context(|| format!("read artifact registry {}", registry_path.display()))?,
    )
    .context("parse artifact registry")?;
    if registry.schema_version != ARTIFACT_REGISTRY_SCHEMA_VERSION {
        bail!(
            "unsupported artifact registry schema version {:?}",
            registry.schema_version
        );
    }
    if registry.invocation_id != manifest.invocation_id {
        bail!("artifact registry and manifest invocation IDs differ");
    }
    if manifest.artifacts.is_empty() {
        bail!("artifact manifest contains no invocation artifacts");
    }
    let registry_manifest = registry
        .artifacts
        .iter()
        .map(|binding| ManifestArtifact {
            path: binding.path.clone(),
            kind: binding.kind.clone(),
            size_bytes: binding.size_bytes,
            sha256: binding.sha256.clone(),
        })
        .collect::<Vec<_>>();
    if registry_manifest != manifest.artifacts {
        bail!("artifact registry and manifest entries differ");
    }
    let report_root = project
        .reports
        .canonicalize()
        .with_context(|| format!("resolve invocation directory {}", project.reports.display()))?;
    let mut failures = Vec::new();
    let disk = collect_publishable_files(&report_root, &report_root, &mut failures)?;
    if !failures.is_empty() {
        bail!(
            "manifest evidence validation failed: {}",
            format_failures(&failures)
        );
    }
    let manifest_paths = manifest
        .artifacts
        .iter()
        .map(|artifact| artifact.path.clone())
        .collect::<BTreeSet<_>>();
    if manifest_paths != disk {
        bail!("manifest and publishable disk artifact sets differ");
    }
    for artifact in &manifest.artifacts {
        let binding = registry
            .artifacts
            .iter()
            .find(|binding| binding.path == artifact.path)
            .ok_or_else(|| {
                anyhow::anyhow!("artifact registry entry is missing: {}", artifact.path)
            })?;
        if binding.invocation_id != manifest.invocation_id {
            bail!("artifact invocation binding differs: {}", artifact.path);
        }
        let actual = binding_for(
            &report_root,
            &artifact.path,
            &artifact.kind,
            binding.step_id.clone(),
            &manifest.invocation_id,
        )
        .map_err(|failure| anyhow::anyhow!("{}: {}", failure.code, failure.message))?;
        if actual.size_bytes != artifact.size_bytes || actual.sha256 != artifact.sha256 {
            bail!("artifact digest or size changed: {}", artifact.path);
        }
    }
    Ok(())
}

fn verify_manifest(project: &Project) -> Result<()> {
    verify_manifest_without_result(project)?;
    let result_path = project.reports.join(MACHINE_RESULT_FILE);
    let result_bytes = fs::read(&result_path)
        .with_context(|| format!("read machine result {}", result_path.display()))?;
    let result: MachineResultEvidence =
        serde_json::from_slice(&result_bytes).context("parse final machine result")?;
    if result.schema_version != MACHINE_RESULT_SCHEMA_VERSION {
        bail!(
            "unsupported final machine-result schema version {:?}",
            result.schema_version
        );
    }
    let manifest: ArtifactManifest = serde_json::from_slice(
        &fs::read(project.reports.join(MANIFEST_FILE)).context("read final artifact manifest")?,
    )
    .context("parse final artifact manifest")?;
    if result.invocation_id != manifest.invocation_id {
        bail!("machine result and manifest invocation IDs differ");
    }
    if !result.evidence_complete {
        bail!("final machine result does not claim complete evidence");
    }
    let result_artifacts = result
        .artifacts
        .iter()
        .map(|artifact| {
            let invocation_id = artifact.invocation_id.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "machine artifact has no invocation binding: {}",
                    artifact.path
                )
            })?;
            let size_bytes = artifact.size_bytes.ok_or_else(|| {
                anyhow::anyhow!("machine artifact has no size: {}", artifact.path)
            })?;
            let sha256 = artifact.sha256.as_deref().ok_or_else(|| {
                anyhow::anyhow!("machine artifact has no digest: {}", artifact.path)
            })?;
            if invocation_id != result.invocation_id {
                bail!(
                    "machine artifact invocation binding differs: {}",
                    artifact.path
                );
            }
            let _ = &artifact.step_id;
            Ok(ManifestArtifact {
                path: artifact.path.clone(),
                kind: artifact.kind.clone(),
                size_bytes,
                sha256: sha256.to_string(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    if result_artifacts != manifest.artifacts {
        bail!("machine result and manifest artifact entries differ");
    }
    if matches!(result.status.as_str(), "PASS" | "WAIVED") && !result.evidence_complete {
        bail!("successful machine result cannot omit complete evidence");
    }
    Ok(())
}

fn format_failures(failures: &[MachineFailure]) -> String {
    failures
        .iter()
        .map(|failure| format!("{}: {}", failure.code, redact_text(&failure.message)))
        .collect::<Vec<_>>()
        .join("; ")
}

fn redact_invocation_files(project: &Project) -> Result<()> {
    if !project.reports.exists() {
        return Ok(());
    }

    let repository = project
        .root
        .canonicalize()
        .with_context(|| format!("resolve project root {}", project.root.display()))?;
    let reports = project
        .reports
        .canonicalize()
        .with_context(|| format!("resolve report directory {}", project.reports.display()))?;
    if !reports.starts_with(&repository) {
        bail!("report directory escapes project root");
    }
    let mut total_bytes = 0_u64;
    redact_directory(&reports, &reports, &mut total_bytes)
}

fn redact_directory(root: &Path, directory: &Path, total_bytes: &mut u64) -> Result<()> {
    if !directory.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)
        .with_context(|| format!("read evidence directory {}", directory.display()))?
    {
        let entry = entry.with_context(|| "read evidence entry")?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("inspect evidence entry {}", path.display()))?;
        if file_type.is_dir() {
            redact_directory(root, &path, total_bytes)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        // Check the declared size before allocating a buffer. Report files
        // are untrusted evidence and must not turn redaction into a memory
        // amplification path.
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("inspect evidence {}", path.display()))?;
        if metadata.len() > REDACTION_TEXT_LIMIT as u64 {
            bail!("text evidence exceeds redaction limit: {}", path.display());
        }
        *total_bytes = total_bytes
            .checked_add(metadata.len())
            .ok_or_else(|| anyhow::anyhow!("invocation evidence byte budget overflow"))?;
        if *total_bytes > MAX_INVOCATION_EVIDENCE_BYTES {
            bail!(
                "invocation evidence exceeds {} bytes (observed {})",
                MAX_INVOCATION_EVIDENCE_BYTES,
                *total_bytes
            );
        }
        if path.file_name().and_then(|name| name.to_str()) == Some("manifest.json")
            || path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".tmp"))
        {
            continue;
        }
        let bytes = fs::read(&path).with_context(|| format!("read evidence {}", path.display()))?;
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        let redacted = redact_text(text);
        if redacted.as_bytes() != bytes {
            let relative = path
                .strip_prefix(root)
                .with_context(|| format!("resolve evidence path {}", path.display()))?;
            crate::utils::fs::confined_atomic_write(root, relative, redacted.as_bytes(), true)
                .with_context(|| format!("publish redacted evidence {}", path.display()))?;
        }
    }
    Ok(())
}

fn redact_runner(mut runner: crate::process::RunnerExecution) -> crate::process::RunnerExecution {
    runner.environment = runner
        .environment
        .into_iter()
        .map(|(key, value)| (key, redact_text(&value)))
        .collect();
    runner
}

fn redact_waiver(mut waiver: crate::process::WaiverEvidence) -> crate::process::WaiverEvidence {
    waiver.id = redact_text(&waiver.id);
    waiver.risk = redact_text(&waiver.risk);
    waiver.owner = redact_text(&waiver.owner);
    waiver.approved_by = redact_text(&waiver.approved_by);
    waiver.created_at = redact_text(&waiver.created_at);
    waiver.expires_at = redact_text(&waiver.expires_at);
    waiver.compensating_control = redact_text(&waiver.compensating_control);
    waiver
}

fn prune_old_invocations(project: &Project, current_id: &str) -> Result<()> {
    let Some(invocations) = project.reports.parent() else {
        return Ok(());
    };
    if invocations.file_name().and_then(|name| name.to_str()) != Some("invocations") {
        return Ok(());
    }
    let mut directories = Vec::new();
    for entry in fs::read_dir(invocations).with_context(|| {
        format!(
            "read invocation retention directory {}",
            invocations.display()
        )
    })? {
        let entry = entry.context("read invocation retention entry")?;
        let path = entry.path();
        if !entry.file_type()?.is_dir()
            || path.file_name().and_then(|name| name.to_str()) == Some(current_id)
        {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        directories.push((modified, path));
    }
    if directories.len() <= MAX_RETAINED_INVOCATIONS {
        return Ok(());
    }
    directories.sort_by_key(|(modified, _)| *modified);
    let cutoff = std::time::SystemTime::now()
        .checked_sub(Duration::from_secs(15 * 60))
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    let remove_count = directories.len() - MAX_RETAINED_INVOCATIONS;
    for (modified, path) in directories.into_iter().take(remove_count) {
        if modified < cutoff {
            fs::remove_dir_all(&path)
                .with_context(|| format!("remove expired invocation {}", path.display()))?;
        }
    }
    Ok(())
}

fn resolve_report_root(project: &Project, repository: &Path) -> Result<PathBuf> {
    let reports = if fs::symlink_metadata(&project.reports).is_ok() {
        project
            .reports
            .canonicalize()
            .with_context(|| format!("resolve report directory {}", project.reports.display()))?
    } else {
        let mut ancestor = project.reports.as_path();
        while fs::symlink_metadata(ancestor).is_err() {
            ancestor = ancestor
                .parent()
                .ok_or_else(|| anyhow::anyhow!("report directory has no resolvable parent"))?;
        }
        let resolved_ancestor = ancestor
            .canonicalize()
            .with_context(|| format!("resolve report directory {}", ancestor.display()))?;
        if !resolved_ancestor.starts_with(repository) {
            bail!("report directory escapes project root");
        }
        fs::create_dir_all(&project.reports)
            .with_context(|| format!("create report directory {}", project.reports.display()))?;
        project
            .reports
            .canonicalize()
            .with_context(|| format!("resolve report directory {}", project.reports.display()))?
    };
    if !reports.starts_with(repository) {
        bail!("report directory escapes project root");
    }
    Ok(reports)
}

pub(super) fn notify(report: &VerificationReport, project: &Project) -> Result<()> {
    notify_with(report, project, &PolicyWebhookTransport)
}

trait WebhookTransport {
    fn send(&self, config: &WebhookConfig, body: &[u8]) -> Result<()>;
}

struct PolicyWebhookTransport;

impl WebhookTransport for PolicyWebhookTransport {
    fn send(&self, config: &WebhookConfig, body: &[u8]) -> Result<()> {
        post_json_with_policy(&config.url, &config.allowed_hosts, body).with_context(|| {
            format!(
                "send verification report to {}",
                destination_summary(config)
            )
        })
    }
}

fn notify_with<T: WebhookTransport>(
    report: &VerificationReport,
    project: &Project,
    transport: &T,
) -> Result<()> {
    let body = redact_text(&serde_json::to_string(report)?).into_bytes();
    for webhook in &project.config.notifications.webhooks {
        if (report.passed && !webhook.on_success) || (!report.passed && !webhook.on_failure) {
            continue;
        }
        transport.send(webhook, &body)?;
    }
    Ok(())
}

fn destination_summary(config: &WebhookConfig) -> String {
    url::Url::parse(&config.url)
        .ok()
        .and_then(|url| {
            url.host_str()
                .map(|host| format!("{}://{}", url.scheme(), normalize_host(host)))
        })
        .unwrap_or_else(|| "webhook destination".into())
}

#[derive(Debug)]
struct WebhookPolicyError {
    code: FailureCode,
    host: String,
    reason: &'static str,
}

impl std::fmt::Display for WebhookPolicyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} (host {:?}: {})",
            self.code, self.host, self.reason
        )
    }
}

impl std::error::Error for WebhookPolicyError {}

#[derive(Debug)]
struct PolicyResolver {
    allowed_hosts: Vec<String>,
}

impl PolicyResolver {
    fn allows_host(&self, host: &str) -> bool {
        let host = normalize_host(host);
        self.allowed_hosts
            .iter()
            .map(|allowed| normalize_host(allowed))
            .any(|allowed| allowed == host)
    }

    fn check_addresses(
        &self,
        host: &str,
        addresses: &ResolvedSocketAddrs,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        if !self.allows_host(host) {
            return Err(ureq::Error::Other(Box::new(WebhookPolicyError {
                code: FailureCode::WebhookDestinationDenied,
                host: normalize_host(host),
                reason: "host is not in the explicit allowlist",
            })));
        }
        let mut permitted = <Self as Resolver>::empty(self);
        for address in addresses {
            if is_local_only(address.ip()) {
                return Err(ureq::Error::Other(Box::new(WebhookPolicyError {
                    code: FailureCode::WebhookDestinationDenied,
                    host: normalize_host(host),
                    reason: "resolved address is local-only",
                })));
            }
            permitted.push(*address);
        }
        if permitted.is_empty() {
            return Err(ureq::Error::HostNotFound);
        }
        Ok(permitted)
    }
}

impl Resolver for PolicyResolver {
    fn resolve(
        &self,
        uri: &ureq::http::Uri,
        config: &ureq::config::Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let host = uri.host().ok_or_else(|| {
            ureq::Error::Other(Box::new(WebhookPolicyError {
                code: FailureCode::WebhookDestinationDenied,
                host: "<missing>".into(),
                reason: "URL has no host",
            }))
        })?;
        let resolved = DefaultResolver::default().resolve(uri, config, timeout)?;
        self.check_addresses(host, &resolved)
    }
}

fn markdown(report: &VerificationReport) -> String {
    let mut output = String::from("=== Verification report ===\n");
    output.push_str(&format!("Timestamp: {}\n", report.timestamp));
    output.push_str(&format!("Profile: {}\n", report.profile));
    output.push_str(&format!(
        "Components: {}\n\n",
        report
            .scope
            .components
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(",")
    ));
    for step in &report.steps {
        let status = if step.waived {
            "WAIVED"
        } else if step.passed {
            "PASS"
        } else {
            "FAIL"
        };
        output.push_str(&format!(
            "- {status}: {} ({} ms)",
            redact_text(&step.label),
            step.duration_ms
        ));
        if let Some(detail) = &step.detail {
            output.push_str(&format!(" - {}", redact_text(detail)));
        }
        if !step.passed && !step.waived {
            output.push_str(&format!("; log: {}", redact_text(&step.log)));
        }
        output.push('\n');
    }
    for step in &report.skipped_steps {
        output.push_str(&format!(
            "- SKIPPED: {} - {}\n",
            redact_text(&step.label),
            redact_text(&step.reason)
        ));
    }
    output.push_str(&format!(
        "\nTEST_SUMMARY: {}\n",
        if report.passed && report.steps.iter().any(|step| step.waived) {
            "WAIVED"
        } else if report.passed {
            "PASS"
        } else {
            "FAIL"
        }
    ));
    output
}

#[cfg(test)]
fn render_html(template: &str, report: &VerificationReport) -> String {
    let summary = if report.passed && report.steps.iter().any(|step| step.waived) {
        "WAIVED"
    } else if report.passed {
        "PASS"
    } else {
        "FAIL"
    };
    template
        .replace("{{ timestamp }}", &escape(&report.timestamp))
        .replace("{{ profile }}", &escape(&report.profile))
        .replace("{{ summary }}", summary)
        .replace(
            "{{ components }}",
            &escape(
                &report
                    .scope
                    .components
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(","),
            ),
        )
}

fn junit(report: &VerificationReport) -> String {
    let failures = report.steps.iter().filter(|step| !step.passed).count();
    let total = report.steps.len() + report.skipped_steps.len();
    let mut output = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuite name=\"harness-gate\" tests=\"{}\" failures=\"{}\">\n", total, failures);
    for step in &report.steps {
        output.push_str(&format!(
            "  <testcase name=\"{}\" time=\"{:.3}\">",
            escape(&redact_text(&step.label)),
            step.duration_ms as f64 / 1000.0
        ));
        if !step.passed {
            output.push_str(&format!(
                "<failure message=\"{}\"/>",
                escape(&redact_text(step.detail.as_deref().unwrap_or("failed")))
            ));
        } else if step.waived {
            output.push_str("<skipped message=\"WAIVED\"/>");
        }
        output.push_str("</testcase>\n");
    }
    for step in &report.skipped_steps {
        output.push_str(&format!(
            "  <testcase name=\"{}\" time=\"0.000\"><skipped message=\"{}\"/></testcase>\n",
            escape(&redact_text(&step.label)),
            escape(&redact_text(&step.reason))
        ));
    }
    output.push_str("</testsuite>\n");
    output
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
fn post_json(raw_url: &str, body: &[u8]) -> Result<()> {
    let response = ureq::post(raw_url)
        .config()
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .content_type("application/json")
        .send(body)
        .context("send webhook request")?;
    if !response.status().is_success() {
        bail!("webhook returned HTTP status {}", response.status());
    }
    Ok(())
}

fn post_json_with_policy(raw_url: &str, allowed_hosts: &[String], body: &[u8]) -> Result<()> {
    let parsed = url::Url::parse(raw_url).context("parse webhook URL")?;
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("WEBHOOK_DESTINATION_DENIED: webhook URL has no host"))?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        bail!("WEBHOOK_DESTINATION_DENIED: webhook URL must be credential-free HTTP(S)");
    }
    let resolver = PolicyResolver {
        allowed_hosts: allowed_hosts.to_vec(),
    };
    if !resolver.allows_host(host) {
        return Err(anyhow::Error::new(WebhookPolicyError {
            code: FailureCode::WebhookDestinationDenied,
            host: normalize_host(host),
            reason: "host is not in the explicit allowlist",
        }));
    }
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .timeout_resolve(Some(Duration::from_secs(5)))
        .max_redirects(0)
        .proxy(None)
        .build();
    let agent = ureq::Agent::with_parts(
        config,
        ureq::unversioned::transport::DefaultConnector::default(),
        resolver,
    );
    let response = agent
        .post(raw_url)
        .content_type("application/json")
        .send(body)
        .context("send webhook request")?;
    validate_webhook_status(response.status(), host)
}

fn validate_webhook_status(status: ureq::http::StatusCode, host: &str) -> Result<()> {
    if status.is_redirection() {
        return Err(anyhow::Error::new(WebhookPolicyError {
            code: FailureCode::WebhookRedirectDenied,
            host: normalize_host(host),
            reason: "redirects are disabled for webhook delivery",
        }));
    }
    if !status.is_success() {
        bail!("webhook returned HTTP status {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        allocate_invocation, configured_html, ensure_supported_machine_result, junit,
        machine_result, markdown, notify, notify_with, post_json, redact_invocation_files,
        redact_text, render_html, report_target, validate_webhook_status, verify_manifest, write,
        write_invocation_metadata, PolicyResolver, ARTIFACT_REGISTRY_FILE, MACHINE_RESULT_FILE,
        MANIFEST_FILE, REDACTION_TEXT_LIMIT,
    };
    use crate::config::WebhookConfig;
    use crate::process::TaskResult;
    use crate::scope::ScopeResult;
    use crate::verify::{SkippedStep, VerificationReport};
    use std::io::{Read, Write};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener};
    use std::thread;
    use std::time::Duration;
    use ureq::unversioned::resolver::Resolver;

    struct LocalWebhookTransport;

    impl super::WebhookTransport for LocalWebhookTransport {
        fn send(&self, config: &WebhookConfig, body: &[u8]) -> anyhow::Result<()> {
            super::post_json(&config.url, body)
        }
    }

    fn read_http_request(stream: &mut std::net::TcpStream) -> Vec<u8> {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set request read timeout");
        let mut request = Vec::new();
        let mut chunk = [0_u8; 1024];
        loop {
            let size = stream.read(&mut chunk).expect("read webhook request");
            if size == 0 {
                break;
            }
            request.extend_from_slice(&chunk[..size]);
            let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n")
            else {
                continue;
            };
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers
                .lines()
                .filter_map(|line| line.split_once(':'))
                .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                .and_then(|(_, value)| value.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if request.len() >= header_end + 4 + content_length {
                break;
            }
        }
        request
    }

    fn report() -> VerificationReport {
        VerificationReport {
            invocation_id: "inv-test".into(),
            executor_version: "0.3.3".into(),
            report_directory: "reports/invocations/inv-test".into(),
            timestamp: "2026-01-01T00:00:00Z".into(),
            profile: "full".into(),
            input_mode: "working-tree".into(),
            project_identity: "/repository".into(),
            source_identity: "working-tree:fixture".into(),
            execution_root: "/repository".into(),
            configuration_digest: format!("sha256:{}", "0".repeat(64)),
            scope: ScopeResult {
                mode: "all".into(),
                changed_files: vec![],
                components: ["api".to_string()].into_iter().collect(),
                unmatched_files: vec![],
            },
            services: vec![],
            steps: vec![TaskResult {
                step_id: None,
                invocation_id: None,
                attempt: None,
                started_at: None,
                finished_at: None,
                label: "unit tests".into(),
                passed: false,
                timed_out: false,
                cancelled: false,
                duration_ms: 42,
                log: "logs/unit.log".into(),
                detail: Some("exit code 1".into()),
                failure_code: None,
                attempts: Vec::new(),
                flaky: false,
                retry_class: None,
                parser: None,
                waived: false,
                waiver: None,
                runner: None,
            }],
            skipped_steps: vec![],
            passed: false,
        }
    }

    fn complete_invocation_fixture(
        name: &str,
    ) -> (
        crate::test_support::TestWorkspace,
        crate::project::Project,
        super::Invocation,
        crate::project::Project,
        VerificationReport,
    ) {
        let workspace = crate::test_support::TestWorkspace::new(name);
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        let project = crate::project::Project::discover(Some(workspace.root.clone()), None)
            .expect("discover fixture");
        let invocation = allocate_invocation(&project).expect("allocate invocation");
        write_invocation_metadata(&invocation, &project, "full", false)
            .expect("write invocation metadata");
        let mut invocation_project = project.clone();
        invocation_project.reports = invocation.root.clone();
        let mut current = report();
        current.invocation_id = invocation.id.clone();
        current.report_directory = invocation.root.to_string_lossy().into_owned();
        current.input_mode = project.input().mode.as_str().into();
        current.project_identity = project.input().project_identity.clone();
        current.source_identity = project.input().source_identity.clone();
        current.execution_root = project
            .input()
            .execution_root
            .to_string_lossy()
            .into_owned();
        current.configuration_digest = project.input().configuration_digest.clone();
        current.passed = true;
        current.steps[0].passed = true;
        current.steps[0].detail = None;
        current.steps[0].step_id = Some("unit.tests".into());
        current.steps[0].invocation_id = Some(invocation.id.clone());
        std::fs::write(invocation.root.join("logs/unit.log"), b"complete\n")
            .expect("write fixture log");
        (workspace, project, invocation, invocation_project, current)
    }

    fn assert_incomplete_result(project: &crate::project::Project) {
        let bytes = std::fs::read(project.reports.join(MACHINE_RESULT_FILE))
            .expect("read incomplete machine result");
        let value: serde_json::Value = serde_json::from_slice(&bytes).expect("parse result");
        assert_eq!(value["evidence_complete"], false);
        assert_eq!(value["passed"], false);
    }

    #[test]
    fn markdown_keeps_legacy_summary_shape() {
        let output = markdown(&report());
        assert!(output.contains("=== Verification report ==="));
        assert!(output.contains("TEST_SUMMARY: FAIL"));
        assert!(output.contains("logs/unit.log"));
    }

    #[test]
    fn machine_result_uses_versioned_status_attempts_and_artifact_references() {
        let value = serde_json::to_value(machine_result(&report())).expect("machine result JSON");
        assert_eq!(value["schema_version"], "1");
        assert_eq!(value["status"], "FAIL");
        assert!(value["services"].as_array().is_some());
        assert_eq!(value["steps"][0]["status"], "FAIL");
        assert_eq!(value["steps"][0]["attempts"][0]["attempt"], 1);
        assert_eq!(value["artifacts"][0]["path"], "logs/unit.log");
        assert_eq!(value["artifacts"][0]["kind"], "step-log");
        assert_eq!(value["evidence_complete"], false);
        assert_eq!(value["failures"][0]["code"], "STEP_FAILED");
    }

    #[test]
    fn machine_result_keeps_cancellation_distinct_from_failure() {
        let mut report = report();
        report.steps[0].passed = false;
        report.steps[0].cancelled = true;
        report.steps[0].step_id = Some("unit.tests".into());
        report.skipped_steps.push(SkippedStep {
            id: "dependent.tests".into(),
            label: "dependent tests".into(),
            reason: "verification cancelled before dispatch".into(),
            cancelled: true,
        });
        let value = serde_json::to_value(machine_result(&report)).expect("machine result JSON");
        assert_eq!(value["status"], "CANCELLED");
        assert_eq!(value["steps"][0]["status"], "CANCELLED");
        assert_eq!(value["skipped_steps"][0]["status"], "SKIPPED");
        assert!(value["failures"]
            .as_array()
            .expect("failures array")
            .is_empty());
    }

    #[test]
    fn machine_status_does_not_infer_cancellation_from_skip_wording() {
        let mut report = report();
        report.skipped_steps.push(SkippedStep {
            id: "dependent.tests".into(),
            label: "dependent tests".into(),
            reason: "wording mentions cancellation but was blocked by a failed prerequisite".into(),
            cancelled: false,
        });
        let value = serde_json::to_value(machine_result(&report)).expect("machine result JSON");
        assert_eq!(value["status"], "FAIL");
    }

    #[test]
    fn machine_result_rejects_artifacts_outside_the_invocation() {
        let mut report = report();
        report.passed = true;
        report.steps[0].passed = true;
        report.steps[0].step_id = Some("unit.tests".into());
        report.steps[0].log = "/tmp/outside-invocation.log".into();
        let value = serde_json::to_value(machine_result(&report)).expect("machine result JSON");
        assert_eq!(value["status"], "FAIL");
        assert_eq!(value["passed"], false);
        assert_eq!(value["evidence_complete"], false);
        assert_eq!(value["failures"][0]["code"], "EVIDENCE_PATH_ESCAPE");
        assert!(value["artifacts"]
            .as_array()
            .expect("artifacts array")
            .is_empty());
    }

    #[test]
    fn unsupported_machine_result_versions_fail_closed() {
        let error = ensure_supported_machine_result(br#"{"schema_version":"2"}"#)
            .expect_err("unsupported schema version must fail");
        assert!(error
            .to_string()
            .contains("unsupported machine-result schema"));
    }

    #[test]
    fn unknown_machine_failure_contracts_fail_closed() {
        let error = ensure_supported_machine_result(
            br#"{"schema_version":"1","steps":[{"failure_code":"FUTURE_CODE","retry_class":"timeout"}]}"#,
        )
        .expect_err("unknown failure code must fail");
        assert!(error.to_string().contains("unknown failure code"));

        let error = ensure_supported_machine_result(
            br#"{"schema_version":"1","steps":[{"failure_code":null,"retry_class":"future"}]}"#,
        )
        .expect_err("unknown retry class must fail");
        assert!(error.to_string().contains("unknown retry class"));
    }

    #[test]
    fn redaction_removes_credentials_from_exported_text() {
        let auth_header = ["Author", "ization"].concat();
        let bearer = ["Bear", "er"].concat();
        let cookie_header = ["cook", "ie"].concat();
        let db_scheme = ["post", "gres"].concat();
        let private_key = ["PRIVATE", " KEY"].concat();
        let password_key = ["pass", "word"].concat();
        let text = format!(
            "{auth_header}: {bearer} opaque-value\n{cookie_header}: session=opaque-cookie\nDATABASE_URL={db_scheme}://user:opaque-pass@db.example.test/app\n{password_key}=opaque-value\n{{\"api_key\":\"opaque-json\"}}\n-----BEGIN {private_key}-----\nopaque-material\n-----END {private_key}-----"
        );
        let redacted = redact_text(&text);
        for secret in [
            "opaque-value",
            "opaque-cookie",
            "opaque-pass@db.example.test",
            "opaque-json",
            "opaque-material",
        ] {
            assert!(!redacted.contains(secret), "secret leaked: {secret}");
        }
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn redaction_rejects_oversized_evidence_before_reading_it() {
        let workspace = crate::test_support::TestWorkspace::new("report-size-limit");
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        let project = crate::project::Project::discover(Some(workspace.root.clone()), None)
            .expect("discover fixture");
        let invocation = allocate_invocation(&project).expect("allocate invocation");
        let oversized = invocation.root.join("large-evidence.bin");
        std::fs::File::create(&oversized)
            .expect("create sparse evidence")
            .set_len(REDACTION_TEXT_LIMIT as u64 + 1)
            .expect("set sparse evidence size");
        let mut invocation_project = project;
        invocation_project.reports = invocation.root.clone();

        let error = redact_invocation_files(&invocation_project)
            .expect_err("oversized evidence must fail before reading");
        assert!(error.to_string().contains("redaction limit"));
    }

    #[test]
    fn manifest_records_digests_and_detects_tampering() {
        let workspace = crate::test_support::TestWorkspace::new("report-manifest");
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        let project = crate::project::Project::discover(Some(workspace.root.clone()), None)
            .expect("discover fixture");
        let invocation = allocate_invocation(&project).expect("allocate invocation");
        write_invocation_metadata(&invocation, &project, "full", false).expect("write metadata");
        let password_key = ["pass", "word"].concat();
        std::fs::write(
            invocation.root.join("logs/unit.log"),
            format!("{password_key}=opaque-value\n"),
        )
        .expect("write log");
        let mut invocation_project = project.clone();
        invocation_project.reports = invocation.root.clone();
        let mut current = report();
        current.invocation_id = invocation.id.clone();
        current.report_directory = invocation.root.to_string_lossy().into_owned();
        current.input_mode = project.input().mode.as_str().into();
        current.project_identity = project.input().project_identity.clone();
        current.source_identity = project.input().source_identity.clone();
        current.execution_root = project
            .input()
            .execution_root
            .to_string_lossy()
            .into_owned();
        current.configuration_digest = project.input().configuration_digest.clone();
        current.steps[0].step_id = Some("unit.tests".into());
        current.steps[0].invocation_id = Some(invocation.id.clone());
        write(&current, &invocation_project).expect("write reports and manifest");

        let manifest_path = invocation.root.join("manifest.json");
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest_path).expect("read manifest"))
                .expect("parse manifest");
        let log = manifest["artifacts"]
            .as_array()
            .expect("manifest artifacts")
            .iter()
            .find(|artifact| artifact["path"] == "logs/unit.log")
            .expect("log manifest entry");
        assert_eq!(log["size_bytes"], 11);
        assert_eq!(log["sha256"].as_str().unwrap().len(), 64);
        assert_eq!(
            std::fs::read_to_string(invocation.root.join("logs/unit.log"))
                .expect("read redacted log"),
            "[REDACTED]\n"
        );
        verify_manifest(&invocation_project).expect("manifest should verify");
        std::fs::write(invocation.root.join("logs/unit.log"), "tampered\n").expect("tamper log");
        assert!(verify_manifest(&invocation_project).is_err());
        drop(invocation);
    }

    #[test]
    fn missing_required_log_blocks_complete_evidence() {
        let (_workspace, _project, invocation, invocation_project, current) =
            complete_invocation_fixture("report-missing-log");
        std::fs::remove_file(invocation.root.join("logs/unit.log")).expect("remove log");

        let error = write(&current, &invocation_project).expect_err("missing log must fail");
        assert!(error.to_string().contains("EVIDENCE_MISSING"));
        assert_incomplete_result(&invocation_project);
        assert!(!invocation.root.join(MANIFEST_FILE).exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_required_log_blocks_publication_without_touching_target() {
        use std::os::unix::fs::symlink;

        let (_workspace, _project, invocation, invocation_project, current) =
            complete_invocation_fixture("report-symlink-log");
        let outside = invocation
            .root
            .parent()
            .expect("invocation root has a parent")
            .join("outside-log.txt");
        std::fs::write(&outside, b"outside\n").expect("write outside fixture");
        let log = invocation.root.join("logs/unit.log");
        std::fs::remove_file(&log).expect("remove regular log");
        symlink(&outside, &log).expect("create log symlink");

        let error = write(&current, &invocation_project).expect_err("symlink log must fail");
        assert!(error.to_string().contains("EVIDENCE_SYMLINK"));
        assert_eq!(std::fs::read(&outside).expect("read outside"), b"outside\n");
        assert_incomplete_result(&invocation_project);
        let _ = std::fs::remove_file(outside);
    }

    #[test]
    fn stale_invocation_metadata_blocks_publication() {
        let (_workspace, _project, invocation, invocation_project, current) =
            complete_invocation_fixture("report-stale-invocation");
        std::fs::write(
            invocation.root.join("invocation.json"),
            serde_json::json!({
                "invocation_id": "inv-stale",
                "input_mode": current.input_mode,
                "project_identity": current.project_identity,
                "source_identity": current.source_identity,
                "execution_root": current.execution_root,
                "configuration_digest": current.configuration_digest,
            })
            .to_string(),
        )
        .expect("forge stale metadata");

        let error = write(&current, &invocation_project).expect_err("stale metadata must fail");
        assert!(error.to_string().contains("EVIDENCE_INVOCATION_MISMATCH"));
        assert_incomplete_result(&invocation_project);
        assert!(!invocation.root.join(MANIFEST_FILE).exists());
    }

    #[test]
    fn undeclared_file_blocks_publication() {
        let (_workspace, _project, invocation, invocation_project, current) =
            complete_invocation_fixture("report-undeclared-file");
        std::fs::write(invocation.root.join("unexpected.txt"), b"unexpected\n")
            .expect("write undeclared file");

        let error = write(&current, &invocation_project).expect_err("undeclared file must fail");
        assert!(error.to_string().contains("EVIDENCE_UNDECLARED_FILE"));
        assert_incomplete_result(&invocation_project);
        assert!(!invocation.root.join(MANIFEST_FILE).exists());
    }

    #[cfg(unix)]
    #[test]
    fn unlink_open_log_blocks_publication() {
        let (_workspace, _project, invocation, invocation_project, current) =
            complete_invocation_fixture("report-unlink-open");
        let log = invocation.root.join("logs/unit.log");
        let handle = std::fs::File::open(&log).expect("open log");
        std::fs::remove_file(&log).expect("unlink open log");

        let error = write(&current, &invocation_project).expect_err("unlinked log must fail");
        assert!(error.to_string().contains("EVIDENCE_MISSING"));
        assert_incomplete_result(&invocation_project);
        drop(handle);
    }

    #[test]
    fn registry_and_machine_result_bindings_are_verified() {
        let (_workspace, _project, invocation, invocation_project, current) =
            complete_invocation_fixture("report-binding-mismatch");
        write(&current, &invocation_project).expect("publish complete evidence");

        let registry_path = invocation.root.join(ARTIFACT_REGISTRY_FILE);
        let mut registry: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&registry_path).expect("read registry"))
                .expect("parse registry");
        registry["artifacts"][0]["invocation_id"] = serde_json::Value::String("other".into());
        std::fs::write(
            &registry_path,
            serde_json::to_vec_pretty(&registry).expect("serialize registry"),
        )
        .expect("tamper registry");
        assert!(verify_manifest(&invocation_project).is_err());

        write(&current, &invocation_project).expect("republish complete evidence");
        let result_path = invocation.root.join(MACHINE_RESULT_FILE);
        let mut result: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&result_path).expect("read result"))
                .expect("parse result");
        result["artifacts"][0]["invocation_id"] = serde_json::Value::String("other".into());
        std::fs::write(
            &result_path,
            serde_json::to_vec_pretty(&result).expect("serialize result"),
        )
        .expect("tamper result");
        assert!(verify_manifest(&invocation_project).is_err());
    }

    #[test]
    fn markdown_and_junit_expose_skipped_steps() {
        let mut report = report();
        report.skipped_steps.push(SkippedStep {
            id: "dependent.tests".into(),
            label: "dependent tests".into(),
            reason: "blocked by a failed prerequisite".into(),
            cancelled: false,
        });
        let markdown_output = markdown(&report);
        assert!(markdown_output.contains("SKIPPED: dependent tests"));
        let junit_output = junit(&report);
        assert!(junit_output.contains("tests=\"2\" failures=\"1\""));
        assert!(junit_output.contains("<skipped message=\"blocked by a failed prerequisite\"/>"));
    }

    #[test]
    fn junit_escapes_values_and_records_failures() {
        let mut report = report();
        report.steps[0].label = "unit <tests>".into();
        let output = junit(&report);
        assert!(output.contains("tests=\"1\" failures=\"1\""));
        assert!(output.contains("unit &lt;tests&gt;"));
        assert!(output.contains("<failure message=\"exit code 1\"/>"));
    }

    #[test]
    fn html_template_replaces_only_supported_fields() {
        let output = render_html(
            "{{ profile }} {{ summary }} {{ components }} {{ unknown }}",
            &report(),
        );
        assert_eq!(output, "full FAIL api {{ unknown }}");
    }

    #[test]
    fn configured_html_supports_include_and_inheritance() {
        let workspace = crate::test_support::TestWorkspace::new("report-template-tera");
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        let template_root = workspace.root.join("templates");
        std::fs::create_dir_all(&template_root).expect("create template root");
        std::fs::write(
            template_root.join("base.html"),
            "<html><body>{% block content %}{% endblock %}</body></html>",
        )
        .expect("write base template");
        std::fs::write(
            template_root.join("partial.tera"),
            "{{ summary }} / {{ components }}",
        )
        .expect("write partial template");
        std::fs::write(
            template_root.join("report.html"),
            "{% extends \"base.html\" %}{% block content %}{{ profile }} - {% include \"partial.tera\" %}{% endblock %}",
        )
        .expect("write report template");

        let mut project = crate::project::Project::discover(Some(workspace.root.clone()), None)
            .expect("discover fixture");
        project.config.report_templates.root = Some("templates".into());
        project.config.report_templates.template = Some("templates/report.html".into());

        let output = configured_html(&report(), &project)
            .expect("render configured template")
            .expect("configured HTML");
        assert_eq!(output, "<html><body>full - FAIL / api</body></html>");
    }

    #[test]
    fn configured_html_ignores_unrelated_binary_assets() {
        let workspace = crate::test_support::TestWorkspace::new("report-template-assets");
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        let template_root = workspace.root.join("templates");
        std::fs::create_dir_all(&template_root).expect("create template root");
        std::fs::write(template_root.join("report.html"), "{{ summary }}")
            .expect("write report template");
        std::fs::write(template_root.join("logo.bin"), [0_u8, 159, 146, 150])
            .expect("write binary asset");
        std::fs::write(template_root.join("editor.swp"), [0_u8, 1, 2, 3])
            .expect("write editor artifact");

        let mut project = crate::project::Project::discover(Some(workspace.root.clone()), None)
            .expect("discover fixture");
        project.config.report_templates.root = Some("templates".into());
        project.config.report_templates.template = Some("templates/report.html".into());

        let output = configured_html(&report(), &project)
            .expect("render configured template")
            .expect("configured HTML");
        assert_eq!(output, "FAIL");
    }

    #[test]
    fn optional_html_failure_preserves_base_reports() {
        let workspace = crate::test_support::TestWorkspace::new("report-base-on-html-failure");
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        let template_root = workspace.root.join("templates");
        std::fs::create_dir_all(&template_root).expect("create template root");
        std::fs::write(template_root.join("report.html"), "{{")
            .expect("write malformed report template");

        let mut project = crate::project::Project::discover(Some(workspace.root.clone()), None)
            .expect("discover fixture");
        project.config.report_templates.root = Some("templates".into());
        project.config.report_templates.template = Some("templates/report.html".into());

        let error = super::write(&report(), &project).expect_err("malformed template");
        assert!(format!("{error:#}").contains("HTML"));
        assert!(project.reports.join("test_result.json").is_file());
        assert!(project.reports.join("test_result.md").is_file());
        assert!(!project.reports.join("test_result.html").exists());
    }

    #[test]
    fn configured_html_rejects_template_outside_root() {
        let workspace = crate::test_support::TestWorkspace::new("report-template-boundary");
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        std::fs::create_dir_all(workspace.root.join("templates")).expect("create template root");
        std::fs::write(workspace.root.join("outside.html"), "outside")
            .expect("write outside template");
        let mut project = crate::project::Project::discover(Some(workspace.root.clone()), None)
            .expect("discover fixture");
        project.config.report_templates.root = Some("templates".into());
        project.config.report_templates.template = Some("outside.html".into());

        let error = configured_html(&report(), &project).expect_err("template boundary");
        assert!(error
            .to_string()
            .contains("below the configured template root"));
    }

    #[cfg(unix)]
    #[test]
    fn configured_html_rejects_external_symlink_templates() {
        let workspace = crate::test_support::TestWorkspace::new("report-template-symlink");
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        let template_root = workspace.root.join("templates");
        std::fs::create_dir_all(&template_root).expect("create template root");
        let outside = std::env::temp_dir().join(format!(
            "harness-gate-template-outside-{}",
            std::process::id()
        ));
        std::fs::write(&outside, "outside").expect("write outside template");
        std::os::unix::fs::symlink(&outside, template_root.join("outside.html"))
            .expect("create external template symlink");
        std::fs::write(template_root.join("report.html"), "report").expect("write report template");
        let mut project = crate::project::Project::discover(Some(workspace.root.clone()), None)
            .expect("discover fixture");
        project.config.report_templates.root = Some("templates".into());
        project.config.report_templates.template = Some("templates/report.html".into());

        let error = configured_html(&report(), &project).expect_err("external symlink");
        assert!(error.to_string().contains("escapes configured root"));
        let _ = std::fs::remove_file(outside);
    }

    #[test]
    fn report_target_rejects_existing_symlink_escape() {
        let workspace = crate::test_support::TestWorkspace::new("report-target");
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        let project = crate::project::Project::discover(Some(workspace.root.clone()), None)
            .expect("discover fixture");
        let reports = project.reports.clone();
        let outside = workspace.root.join("outside.txt");
        std::fs::create_dir_all(&reports).expect("create reports");
        std::fs::write(&outside, "outside").expect("create outside file");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, reports.join("junit.xml"))
            .expect("create report symlink");
        #[cfg(unix)]
        assert!(report_target(&project, "junit.xml").is_err());
    }

    #[test]
    fn report_target_rejects_windows_style_traversal() {
        let workspace = crate::test_support::TestWorkspace::new("report-target-windows");
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        let project = crate::project::Project::discover(Some(workspace.root.clone()), None)
            .expect("discover fixture");
        for path in [
            "..\\outside.xml",
            "C:\\outside.xml",
            "\\\\server\\share.xml",
        ] {
            assert!(
                report_target(&project, path).is_err(),
                "unsafe path: {path}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn report_target_rejects_missing_directory_through_symlink_escape() {
        let workspace = crate::test_support::TestWorkspace::new("report-target-missing");
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        let mut project = crate::project::Project::discover(Some(workspace.root.clone()), None)
            .expect("discover fixture");
        let outside = std::env::temp_dir().join(format!(
            "harness-gate-report-target-outside-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&outside).expect("create outside directory");
        let link = workspace.root.join("report-link");
        std::os::unix::fs::symlink(&outside, &link).expect("create report symlink");
        project.reports = link.join("new-reports");

        let error = report_target(&project, "junit.xml").expect_err("symlink escape");
        assert!(error.to_string().contains("report directory escapes"));
        assert!(!outside.join("new-reports").exists());
        let _ = std::fs::remove_dir_all(outside);
    }

    #[test]
    fn webhook_accepts_success_response() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
        let address = listener.local_addr().expect("listener address");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept webhook");
            let request = read_http_request(&mut stream);
            assert!(String::from_utf8_lossy(&request).contains("POST /notify"));
            stream
                .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n")
                .expect("write response");
        });
        post_json(&format!("http://{address}/notify"), br#"{"passed":true}"#)
            .expect("webhook should succeed");
        handle.join().expect("webhook thread");
    }

    #[test]
    fn webhook_policy_rejects_local_address_before_connecting() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
        listener
            .set_nonblocking(true)
            .expect("set listener nonblocking");
        let address = listener.local_addr().expect("listener address");
        let error = super::post_json_with_policy(
            &format!("http://{address}/notify"),
            &[address.ip().to_string()],
            br#"{"passed":true}"#,
        )
        .expect_err("loopback webhook must be denied");
        assert!(format!("{error:#}").contains("WEBHOOK_DESTINATION_DENIED"));
        assert!(matches!(
            listener.accept(),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
        ));
    }

    #[test]
    fn webhook_policy_rechecks_every_resolution_and_fails_closed_on_rebinding() {
        let resolver = PolicyResolver {
            allowed_hosts: vec!["hooks.example.test".into()],
        };
        let mut public = <PolicyResolver as Resolver>::empty(&resolver);
        public.push(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)),
            443,
        ));
        public.push(SocketAddr::new(
            "2001:4860:4860::8888"
                .parse::<IpAddr>()
                .expect("public IPv6 fixture"),
            443,
        ));
        let permitted = resolver
            .check_addresses("hooks.example.test", &public)
            .expect("public resolution is permitted");
        assert_eq!(permitted.iter().count(), 2);

        for address in [
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1)),
            IpAddr::V4(Ipv4Addr::new(224, 0, 0, 1)),
            IpAddr::V6(Ipv6Addr::UNSPECIFIED),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            IpAddr::V6("fc00::1".parse().expect("unique-local fixture")),
            IpAddr::V6("fe80::1".parse().expect("link-local fixture")),
            IpAddr::V6("ff00::1".parse().expect("multicast fixture")),
        ] {
            let mut rebound = <PolicyResolver as Resolver>::empty(&resolver);
            rebound.push(SocketAddr::new(address, 443));
            let error = resolver
                .check_addresses("hooks.example.test", &rebound)
                .expect_err("rebound local address must be denied");
            assert!(
                error.to_string().contains("WEBHOOK_DESTINATION_DENIED"),
                "unexpected error for {address}: {error}"
            );
        }
    }

    #[test]
    fn webhook_policy_rejects_unlisted_hosts_without_exposing_secrets() {
        let resolver = PolicyResolver {
            allowed_hosts: vec!["hooks.example.test".into()],
        };
        let mut addresses = <PolicyResolver as Resolver>::empty(&resolver);
        addresses.push(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)),
            443,
        ));
        let error = resolver
            .check_addresses("other.example.test", &addresses)
            .expect_err("unlisted host must be denied");
        assert!(error.to_string().contains("WEBHOOK_DESTINATION_DENIED"));

        let body_secret = "body-secret-must-not-appear";
        let error = super::post_json_with_policy(
            "https://operator:url-secret@example.test/hook?token=query-secret",
            &["example.test".into()],
            body_secret.as_bytes(),
        )
        .expect_err("credential-bearing URL must be denied");
        let rendered = format!("{error:#}");
        for secret in ["operator", "url-secret", "query-secret", body_secret] {
            assert!(!rendered.contains(secret), "policy error leaked {secret}");
        }
    }

    #[test]
    fn webhook_redirects_are_a_typed_policy_failure() {
        let error = validate_webhook_status(ureq::http::StatusCode::FOUND, "hooks.example.test")
            .expect_err("redirect must be denied");
        assert!(format!("{error:#}").contains("WEBHOOK_REDIRECT_DENIED"));
    }

    #[test]
    fn webhook_redacts_sensitive_report_details() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
        let address = listener.local_addr().expect("listener address");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept webhook");
            let request = read_http_request(&mut stream);
            let body = String::from_utf8_lossy(&request);
            assert!(!body.contains("webhook-secret"));
            assert!(body.contains("[REDACTED]"));
            stream
                .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n")
                .expect("write response");
        });
        let workspace = crate::test_support::TestWorkspace::new("webhook-redaction");
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        let mut project = crate::project::Project::discover(Some(workspace.root.clone()), None)
            .expect("discover fixture");
        project.config.notifications.webhooks = vec![WebhookConfig {
            url: format!("http://{address}/notify"),
            allowed_hosts: vec![address.ip().to_string()],
            on_failure: true,
            on_success: false,
        }];
        let mut sensitive = report();
        let token_key = ["tok", "en"].concat();
        sensitive.steps[0].detail = Some(format!("{token_key}=opaque-value"));
        notify_with(&sensitive, &project, &LocalWebhookTransport)
            .expect("redacted webhook should succeed");
        handle.join().expect("webhook thread");
    }

    #[test]
    fn webhook_rejects_non_success_response() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
        let address = listener.local_addr().expect("listener address");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept webhook");
            let _ = read_http_request(&mut stream);
            stream
                .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n")
                .expect("write response");
        });
        let error = post_json(&format!("http://{address}/notify"), br#"{"passed":false}"#)
            .expect_err("webhook failure should be reported");
        assert!(format!("{error:#}").contains("503"), "{error:#}");
        handle.join().expect("webhook thread");
    }

    #[test]
    fn webhook_reports_connection_failure() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
        let address = listener.local_addr().expect("listener address");
        drop(listener);
        let workspace = crate::test_support::TestWorkspace::new("webhook-connection-failure");
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        let mut project = crate::project::Project::discover(Some(workspace.root.clone()), None)
            .expect("discover fixture");
        project.config.notifications.webhooks = vec![WebhookConfig {
            url: format!("http://{address}/notify"),
            allowed_hosts: vec![address.ip().to_string()],
            on_failure: true,
            on_success: false,
        }];
        let error = notify(&report(), &project).expect_err("connection failure should be reported");
        assert!(format!("{error:#}").contains("webhook"), "{error:#}");
    }

    #[test]
    fn webhook_stops_after_the_first_failure_in_configured_order() {
        let first = TcpListener::bind(("127.0.0.1", 0)).expect("bind first listener");
        let first_address = first.local_addr().expect("first listener address");
        let second = TcpListener::bind(("127.0.0.1", 0)).expect("bind second listener");
        second
            .set_nonblocking(true)
            .expect("set second listener nonblocking");
        let second_address = second.local_addr().expect("second listener address");
        let first_handle = thread::spawn(move || {
            let (mut stream, _) = first.accept().expect("accept first webhook");
            let _ = read_http_request(&mut stream);
            stream
                .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n")
                .expect("write first response");
        });

        let workspace = crate::test_support::TestWorkspace::new("webhook-order");
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        let mut project = crate::project::Project::discover(Some(workspace.root.clone()), None)
            .expect("discover fixture");
        project.config.notifications.webhooks = vec![
            WebhookConfig {
                url: format!("http://{first_address}/first"),
                allowed_hosts: vec![first_address.ip().to_string()],
                on_failure: true,
                on_success: false,
            },
            WebhookConfig {
                url: format!("http://{second_address}/second"),
                allowed_hosts: vec![second_address.ip().to_string()],
                on_failure: true,
                on_success: false,
            },
        ];
        let error = notify_with(&report(), &project, &LocalWebhookTransport)
            .expect_err("first webhook should fail");
        assert!(format!("{error:#}").contains("503"), "{error:#}");
        first_handle.join().expect("first webhook thread");
        assert!(
            matches!(second.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
        );
    }

    #[test]
    fn webhook_filters_success_notifications_without_contacting_the_endpoint() {
        let workspace = crate::test_support::TestWorkspace::new("webhook-filter");
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        let mut project = crate::project::Project::discover(Some(workspace.root.clone()), None)
            .expect("discover fixture");
        project.config.notifications.webhooks = vec![WebhookConfig {
            url: "http://127.0.0.1:1/should-not-be-called".into(),
            allowed_hosts: vec!["127.0.0.1".into()],
            on_failure: true,
            on_success: false,
        }];
        let mut success = report();
        success.passed = true;
        notify(&success, &project).expect("disabled success notification is skipped");
    }
}
