use super::VerificationReport;
use crate::config::WebhookConfig;
use crate::project::Project;
use crate::service::ResourceLease;
use anyhow::{bail, Context, Result};
use serde::{ser::Serializer, Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static INVOCATION_COUNTER: AtomicU64 = AtomicU64::new(1);
pub(super) const MACHINE_RESULT_SCHEMA_VERSION: &str = "1";

#[derive(Debug, Serialize)]
struct MachineResult {
    schema_version: &'static str,
    invocation_id: String,
    executor_version: String,
    report_directory: String,
    timestamp: String,
    profile: String,
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
    runner: Option<crate::process::RunnerExecution>,
    attempts: Vec<MachineAttempt>,
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

#[derive(Debug, Serialize)]
struct MachineFailure {
    step_id: Option<String>,
    code: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct MachineArtifact {
    path: String,
    kind: &'static str,
}

#[derive(Debug, Serialize)]
struct MachineService {
    id: String,
    status: &'static str,
}

#[derive(Debug, Deserialize)]
struct MachineResultHeader {
    schema_version: String,
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
                    code: failure_code(step),
                    message: step
                        .detail
                        .clone()
                        .unwrap_or_else(|| "verification step failed".into()),
                });
            }
            if !step.log.is_empty() {
                match invocation_relative_path(&report.report_directory, &step.log) {
                    Some(path) => artifacts.push(MachineArtifact {
                        path,
                        kind: "step-log",
                    }),
                    None => {
                        evidence_complete = false;
                        failures.push(MachineFailure {
                            step_id: step.step_id.clone(),
                            code: "EVIDENCE_PATH_ESCAPE".into(),
                            message: format!(
                                "step log is outside invocation directory: {}",
                                step.log
                            ),
                        });
                    }
                }
            }
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
                detail: step.detail.clone(),
                runner: step.runner.clone(),
                attempts: vec![MachineAttempt {
                    attempt: step.attempt.unwrap_or(1),
                    status,
                    started_at: step.started_at.clone(),
                    finished_at: step.finished_at.clone(),
                    duration_ms: step.duration_ms,
                    timed_out: step.timed_out,
                    cancelled: step.cancelled,
                    log: step.log.clone(),
                    detail: step.detail.clone(),
                }],
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
            reason: step.reason.clone(),
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

fn ensure_supported_machine_result(raw: &[u8]) -> Result<()> {
    let header: MachineResultHeader =
        serde_json::from_slice(raw).context("parse machine-result schema header")?;
    if header.schema_version != MACHINE_RESULT_SCHEMA_VERSION {
        bail!(
            "unsupported machine-result schema version {:?}",
            header.schema_version
        );
    }
    Ok(())
}

fn task_status(step: &crate::process::TaskResult) -> &'static str {
    if step.cancelled {
        "CANCELLED"
    } else if step.passed {
        "PASS"
    } else {
        "FAIL"
    }
}

fn failure_code(step: &crate::process::TaskResult) -> String {
    if step.cancelled {
        "STEP_CANCELLED"
    } else if step.timed_out {
        "STEP_TIMEOUT"
    } else {
        "STEP_FAILED"
    }
    .into()
}

fn report_status(report: &VerificationReport) -> &'static str {
    if report.passed {
        "PASS"
    } else if report.steps.iter().any(|step| step.cancelled)
        || report
            .skipped_steps
            .iter()
            .any(|step| step.reason.contains("cancel"))
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
    executor_version: &'static str,
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
    profile: &str,
    staged: bool,
) -> Result<()> {
    let metadata = InvocationMetadata {
        invocation_id: &invocation.id,
        created_at: chrono::Utc::now().to_rfc3339(),
        profile,
        staged,
        executor_version: env!("CARGO_PKG_VERSION"),
    };
    let contents = serde_json::to_vec_pretty(&metadata).context("serialize invocation metadata")?;
    atomic_write(&invocation.root.join("invocation.json"), &contents, false)
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
            let target = report_target(legacy_project, &relative)?;
            atomic_write(&target, &contents, true)
                .with_context(|| format!("mirror legacy report {}", target.display()))?;
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
            let target = report_target(legacy_project, &relative)?;
            atomic_write(&target, &contents, true)
                .with_context(|| format!("mirror legacy log {}", target.display()))?;
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
    // Keep the stable JSON/Markdown artifacts independent from optional
    // renderers. A malformed template must not erase the machine-readable
    // result that CI and incident tooling depend on.
    let mut failures = Vec::new();
    match serde_json::to_string_pretty(report)
        .context("serialize verification report as JSON")
        .and_then(|json| {
            ensure_supported_machine_result(json.as_bytes())?;
            write_report_file(project, "test_result.json", json)
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
    if failures.is_empty() {
        return Ok(());
    }
    let details = failures
        .iter()
        .map(|error| format!("{error:#}"))
        .collect::<Vec<_>>()
        .join("; ");
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
    let target = report_target(project, relative)?;
    atomic_write(&target, contents.as_ref(), false)
        .with_context(|| format!("write report {}", target.display()))
}

fn atomic_write(target: &Path, contents: &[u8], replace_existing: bool) -> Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| anyhow::anyhow!("output has no parent directory"))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("create output directory {}", parent.display()))?;
    let counter = INVOCATION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("output has an invalid filename"))?;
    let temporary = parent.join(format!(".{file_name}.{counter}.tmp"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .with_context(|| format!("create temporary output {}", temporary.display()))?;
    let write_result = (|| -> Result<()> {
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        if replace_existing && fs::symlink_metadata(target).is_ok() {
            fs::remove_file(target)
                .with_context(|| format!("replace existing output {}", target.display()))?;
        }
        fs::rename(&temporary, target)
            .with_context(|| format!("publish output {}", target.display()))?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result
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
    let body = serde_json::to_vec(report)?;
    for webhook in &project.config.notifications.webhooks {
        if (report.passed && !webhook.on_success) || (!report.passed && !webhook.on_failure) {
            continue;
        }
        WebhookNotifier { config: webhook }.notify(&body)?;
    }
    Ok(())
}

trait Notifier {
    fn notify(&self, body: &[u8]) -> Result<()>;
}

struct WebhookNotifier<'a> {
    config: &'a WebhookConfig,
}

impl Notifier for WebhookNotifier<'_> {
    fn notify(&self, body: &[u8]) -> Result<()> {
        post_json(&self.config.url, body)
            .with_context(|| format!("send verification report to webhook {}", self.config.url))
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
        let status = if step.passed { "PASS" } else { "FAIL" };
        output.push_str(&format!(
            "- {status}: {} ({} ms)",
            step.label, step.duration_ms
        ));
        if let Some(detail) = &step.detail {
            output.push_str(&format!(" - {detail}"));
        }
        if !step.passed {
            output.push_str(&format!("; log: {}", step.log));
        }
        output.push('\n');
    }
    for step in &report.skipped_steps {
        output.push_str(&format!("- SKIPPED: {} - {}\n", step.label, step.reason));
    }
    output.push_str(&format!(
        "\nTEST_SUMMARY: {}\n",
        if report.passed { "PASS" } else { "FAIL" }
    ));
    output
}

#[cfg(test)]
fn render_html(template: &str, report: &VerificationReport) -> String {
    let summary = if report.passed { "PASS" } else { "FAIL" };
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
            escape(&step.label),
            step.duration_ms as f64 / 1000.0
        ));
        if !step.passed {
            output.push_str(&format!(
                "<failure message=\"{}\"/>",
                escape(step.detail.as_deref().unwrap_or("failed"))
            ));
        }
        output.push_str("</testcase>\n");
    }
    for step in &report.skipped_steps {
        output.push_str(&format!(
            "  <testcase name=\"{}\" time=\"0.000\"><skipped message=\"{}\"/></testcase>\n",
            escape(&step.label),
            escape(&step.reason)
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

#[cfg(test)]
mod tests {
    use super::{
        configured_html, ensure_supported_machine_result, junit, machine_result, markdown, notify,
        post_json, render_html, report_target,
    };
    use crate::config::WebhookConfig;
    use crate::process::TaskResult;
    use crate::scope::ScopeResult;
    use crate::verify::{SkippedStep, VerificationReport};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

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
                runner: None,
            }],
            skipped_steps: vec![],
            passed: false,
        }
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
    fn markdown_and_junit_expose_skipped_steps() {
        let mut report = report();
        report.skipped_steps.push(SkippedStep {
            id: "dependent.tests".into(),
            label: "dependent tests".into(),
            reason: "blocked by a failed prerequisite".into(),
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
                on_failure: true,
                on_success: false,
            },
            WebhookConfig {
                url: format!("http://{second_address}/second"),
                on_failure: true,
                on_success: false,
            },
        ];
        let error = notify(&report(), &project).expect_err("first webhook should fail");
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
            on_failure: true,
            on_success: false,
        }];
        let mut success = report();
        success.passed = true;
        notify(&success, &project).expect("disabled success notification is skipped");
    }
}
