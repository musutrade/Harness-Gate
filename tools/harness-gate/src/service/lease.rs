use super::report_directory::ReportDirectoryGuard;
use crate::config::ContainerRuntimeKind;
use crate::project::Project;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(crate) const LEASE_SCHEMA_VERSION: u32 = 2;
/// Cleanup evidence has its own public schema.  Lease records may evolve
/// independently without invalidating consumers of `cleanup.json`.
pub(crate) const CLEANUP_REPORT_SCHEMA_VERSION: u32 = 1;
pub(crate) const OWNER_MARKER: &str = "harness-gate";
pub(crate) const LABEL_OWNER: &str = "harness-gate.owner";
pub(crate) const LABEL_SCHEMA: &str = "harness-gate.schema";
pub(crate) const LABEL_PROJECT: &str = "harness-gate.project";
pub(crate) const LABEL_RESOURCE: &str = "harness-gate.resource";
pub(crate) const LABEL_KIND: &str = "harness-gate.kind";
pub(crate) const LABEL_INVOCATION: &str = "harness-gate.invocation";
const LEASE_TTL: Duration = Duration::from_secs(15 * 60);
const RENEW_AFTER: Duration = Duration::from_secs(30);
#[cfg(not(test))]
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
#[cfg(test)]
const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(250);
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

trait RuntimeOperations {
    fn inspect(
        &self,
        runtime: ContainerRuntimeKind,
        project: &Project,
        name: &str,
        timeout: Duration,
    ) -> Result<super::inspection::RuntimeInspection>;

    fn stop(
        &self,
        runtime: ContainerRuntimeKind,
        cwd: &Path,
        name: &str,
        timeout: Duration,
    ) -> Result<()>;
}

struct CliRuntimeOperations;

impl RuntimeOperations for CliRuntimeOperations {
    fn inspect(
        &self,
        runtime: ContainerRuntimeKind,
        project: &Project,
        name: &str,
        timeout: Duration,
    ) -> Result<super::inspection::RuntimeInspection> {
        super::runtime::inspect_owned_container(runtime, project, name, timeout)
    }

    fn stop(
        &self,
        runtime: ContainerRuntimeKind,
        cwd: &Path,
        name: &str,
        timeout: Duration,
    ) -> Result<()> {
        super::runtime::stop_owned_container(runtime, cwd, name, timeout)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LeaseRecord {
    pub(crate) owner_marker: String,
    pub(crate) schema_version: u32,
    pub(crate) project_identity: String,
    pub(crate) resource_id: String,
    pub(crate) resource_kind: String,
    pub(crate) invocation_id: String,
    pub(crate) pid: u32,
    pub(crate) process_start_identity: String,
    pub(crate) created_at: u64,
    pub(crate) heartbeat_at: u64,
    pub(crate) expires_at: u64,
    #[serde(default)]
    pub(crate) resource_name: Option<String>,
    #[serde(default)]
    pub(crate) runtime: Option<String>,
    pub(crate) runtime_labels: BTreeMap<String, String>,
    pub(crate) runtime_object_id: Option<String>,
}

#[derive(Debug)]
pub(crate) struct ResourceLease {
    path: PathBuf,
    record: Arc<Mutex<LeaseRecord>>,
    last_renewed: Arc<Mutex<Instant>>,
    heartbeat_error: Arc<Mutex<Option<String>>>,
    stop_heartbeat: Option<mpsc::Sender<()>>,
    heartbeat: Option<JoinHandle<()>>,
    release_on_drop: bool,
    // Closed only after Drop stops heartbeat and attempts the checked release.
    report_directory_guard: Option<ReportDirectoryGuard>,
}

impl ResourceLease {
    pub(crate) fn acquire(
        project: &Project,
        resource_id: impl Into<String>,
        resource_kind: impl Into<String>,
        invocation_id: impl Into<String>,
        resource_name: Option<String>,
        runtime: Option<ContainerRuntimeKind>,
    ) -> Result<Self> {
        let resource_id = resource_id.into();
        let resource_kind = resource_kind.into();
        let invocation_id = invocation_id.into();
        let directory = lease_directory(project)?;
        fs::create_dir_all(&directory)
            .with_context(|| format!("create lease directory {}", directory.display()))?;
        let path = directory.join(format!("{}.json", resource_key(&resource_id)));
        let now = epoch_seconds();
        let project_identity = project.input().project_identity.clone();
        let runtime_labels = ownership_labels(
            &project_identity,
            &resource_id,
            &resource_kind,
            &invocation_id,
        );
        let record = LeaseRecord {
            owner_marker: OWNER_MARKER.into(),
            schema_version: LEASE_SCHEMA_VERSION,
            project_identity,
            resource_id: resource_id.clone(),
            resource_kind,
            invocation_id,
            pid: std::process::id(),
            process_start_identity: process_start_identity(std::process::id()),
            created_at: now,
            heartbeat_at: now,
            expires_at: now.saturating_add(LEASE_TTL.as_secs()),
            resource_name,
            runtime: runtime.map(|kind| kind.executable().to_string()),
            runtime_labels,
            runtime_object_id: None,
        };
        if !identity_is_proven(&record) {
            bail!(
                "LEASE_OWNERSHIP_UNCERTAIN: platform process identity is unavailable; resource allocation rejected"
            );
        }

        let report_directory_guard = if resource_kind_is_report(&record) {
            let guard = ReportDirectoryGuard::try_acquire(project, &record.invocation_id)?
                .ok_or_else(|| {
                    anyhow::anyhow!("report-directory lease conflict: resource is in use")
                })?;
            guard.validate_record(&record)?;
            // Invalidate any earlier release before creating/renewing a marker.
            // A failed allocation or failed release never certifies completion.
            guard.invalidate_release()?;
            Some(guard)
        } else {
            None
        };

        loop {
            match create_record(&path, &record) {
                Ok(()) => {
                    let record = Arc::new(Mutex::new(record));
                    let last_renewed = Arc::new(Mutex::new(Instant::now()));
                    let heartbeat_error = Arc::new(Mutex::new(None));
                    let (stop_heartbeat, receiver) = mpsc::channel();
                    let heartbeat_path = path.clone();
                    let heartbeat_record = Arc::clone(&record);
                    let heartbeat_last = Arc::clone(&last_renewed);
                    let heartbeat_failure = Arc::clone(&heartbeat_error);
                    let heartbeat = thread::spawn(move || loop {
                        match receiver.recv_timeout(HEARTBEAT_INTERVAL) {
                            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                if let Err(error) = renew_parts(
                                    &heartbeat_path,
                                    &heartbeat_record,
                                    &heartbeat_last,
                                    true,
                                ) {
                                    if let Ok(mut failure) = heartbeat_failure.lock() {
                                        if failure.is_none() {
                                            *failure = Some(format!("{error:#}"));
                                        }
                                    }
                                }
                            }
                        }
                    });
                    return Ok(Self {
                        path,
                        record,
                        last_renewed,
                        heartbeat_error,
                        stop_heartbeat: Some(stop_heartbeat),
                        heartbeat: Some(heartbeat),
                        release_on_drop: true,
                        report_directory_guard,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    let existing = read_record(&path).with_context(|| {
                        format!("inspect existing lease for resource {resource_id:?}")
                    })?;
                    validate_record(&existing, &resource_id, &path, &directory, project)?;
                    if resource_kind_is_report(&record) {
                        // Retained report markers must not be converted into
                        // fresh ownership through the generic expiry fallback.
                        bail!("report-directory marker is retained; explicit proven cleanup is required");
                    }
                    if !is_stale(&existing, epoch_seconds()) {
                        bail!(
                            "resource lease conflict for {resource_id:?}: invocation {} (pid {}) owns it",
                            existing.invocation_id,
                            existing.pid
                        );
                    }
                    if !identity_is_proven(&existing) {
                        bail!(
                            "LEASE_OWNERSHIP_UNCERTAIN: stale lease identity cannot be proven; resource retained"
                        );
                    }
                    reclaim_resource(project, &path, &existing)?;
                }
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("create lease for resource {resource_id:?}"));
                }
            }
        }
    }

    pub(crate) fn renew(&self) -> Result<()> {
        renew_parts(&self.path, &self.record, &self.last_renewed, false)
    }

    /// Stop renewal while retaining the marker for operator cleanup. This is
    /// used when an external resource may have been created but its ownership
    /// or identity could not be proved.
    pub(crate) fn retain(mut self) {
        self.stop_heartbeat();
        self.release_on_drop = false;
    }

    /// Stop the heartbeat before attempting an explicit release. If release
    /// cannot prove current ownership, dropping this value keeps the marker so
    /// an operator can inspect it instead of silently deleting it.
    pub(crate) fn release_checked(mut self) -> Result<()> {
        self.finish_release()
    }

    fn finish_release(&mut self) -> Result<()> {
        self.stop_heartbeat();
        let result = self.release();
        self.release_on_drop = false;
        result
    }

    #[cfg(test)]
    pub(crate) fn release_before_guard_close(&mut self) -> Result<()> {
        // Exercise the real checked-release transition while a child process
        // pauses before dropping the same guard that production Drop closes.
        self.finish_release()
    }

    /// Bind a newly created runtime object to this lease. The object ID is
    /// immutable for the lifetime of the object and is never accepted from
    /// repository-controlled configuration.
    pub(crate) fn bind_runtime_identity(
        &self,
        project: &Project,
        inspection: &super::inspection::RuntimeInspection,
    ) -> Result<()> {
        let mut record = self
            .record
            .lock()
            .map_err(|_| anyhow::anyhow!("lease record lock was poisoned"))?;
        let current = read_record(&self.path)
            .with_context(|| format!("read lease {}", self.path.display()))?;
        ensure_owner(&current, &record)?;
        validate_runtime_ownership(project, &self.path, &current, inspection)?;
        if inspection.object_id.trim().is_empty() {
            bail!("runtime inspection returned an empty immutable object ID");
        }
        record.runtime_object_id = Some(inspection.object_id.clone());
        record.runtime_labels = inspection.labels.clone();
        write_record(&self.path, &record)?;
        Ok(())
    }

    pub(crate) fn verify_runtime_ownership(&self, project: &Project) -> Result<()> {
        self.ensure_heartbeat_healthy()?;
        let record = self
            .record
            .lock()
            .map_err(|_| anyhow::anyhow!("lease record lock was poisoned"))?
            .clone();
        verify_runtime_record(&CliRuntimeOperations, project, &self.path, &record)
    }

    pub(crate) fn release(&self) -> Result<()> {
        self.ensure_heartbeat_healthy()?;
        if self.report_directory_guard.is_some() && self.heartbeat.is_some() {
            bail!("report-directory release requires stopped heartbeat");
        }
        let record = self
            .record
            .lock()
            .map_err(|_| anyhow::anyhow!("lease record lock was poisoned"))?;
        let contents = match fs::read(&self.path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if self.report_directory_guard.is_some() {
                    bail!("report-directory marker disappeared before checked release");
                }
                return Ok(());
            }
            Err(error) => return Err(error).with_context(|| "read lease before release"),
        };
        let current: LeaseRecord =
            serde_json::from_slice(&contents).context("parse lease before release")?;
        ensure_owner(&current, &record)?;
        // Persist the certificate while the JSON marker still protects the
        // resource. A certificate write/sync failure leaves that marker intact;
        // nothing fallible follows a successful marker removal.
        if let Some(guard) = &self.report_directory_guard {
            guard.mark_released(&record)?;
        }
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    && self.report_directory_guard.is_some() =>
            {
                if let Some(guard) = &self.report_directory_guard {
                    let _ = guard.invalidate_release();
                }
                Err(error).context("report-directory marker disappeared during release")
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => {
                Err(error).with_context(|| format!("release lease {}", self.path.display()))
            }
        }
    }

    fn ensure_heartbeat_healthy(&self) -> Result<()> {
        if let Some(error) = self
            .heartbeat_error
            .lock()
            .map_err(|_| anyhow::anyhow!("lease heartbeat error lock was poisoned"))?
            .as_ref()
            .cloned()
        {
            bail!("LEASE_OWNERSHIP_UNCERTAIN: lease heartbeat failed: {error}");
        }
        Ok(())
    }
}

impl Drop for ResourceLease {
    fn drop(&mut self) {
        self.stop_heartbeat();
        if self.release_on_drop {
            let _ = self.release();
        }
    }
}

impl ResourceLease {
    fn stop_heartbeat(&mut self) {
        if let Some(stop) = self.stop_heartbeat.take() {
            let _ = stop.send(());
        }
        if let Some(heartbeat) = self.heartbeat.take() {
            let _ = heartbeat.join();
        }
    }
}

fn renew_parts(
    path: &Path,
    record: &Mutex<LeaseRecord>,
    last_renewed: &Mutex<Instant>,
    force: bool,
) -> Result<()> {
    let mut last = last_renewed
        .lock()
        .map_err(|_| anyhow::anyhow!("lease renewal lock was poisoned"))?;
    if !force && last.elapsed() < RENEW_AFTER {
        return Ok(());
    }
    let mut record = record
        .lock()
        .map_err(|_| anyhow::anyhow!("lease record lock was poisoned"))?;
    let current = read_record(path).with_context(|| format!("read lease {}", path.display()))?;
    ensure_owner(&current, &record)?;
    let now = epoch_seconds();
    record.heartbeat_at = now;
    record.expires_at = now.saturating_add(LEASE_TTL.as_secs());
    write_record(path, &record)?;
    *last = Instant::now();
    Ok(())
}

#[derive(Debug, Serialize)]
pub(crate) struct CleanupReport {
    pub(crate) schema_version: u32,
    pub(crate) owner_marker: &'static str,
    pub(crate) dry_run: bool,
    pub(crate) scanned: usize,
    pub(crate) active: usize,
    pub(crate) stale: usize,
    pub(crate) reclaimed: usize,
    pub(crate) resources: Vec<CleanupResource>,
    pub(crate) failures: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CleanupResource {
    pub(crate) resource_id: String,
    pub(crate) resource_kind: String,
    pub(crate) invocation_id: String,
    pub(crate) state: String,
    pub(crate) action: String,
    pub(crate) lease_file: String,
}

pub(crate) fn cleanup(project: &Project, dry_run: bool) -> Result<CleanupReport> {
    cleanup_with_runtime(project, dry_run, &CliRuntimeOperations)
}

fn cleanup_with_runtime<O: RuntimeOperations + ?Sized>(
    project: &Project,
    dry_run: bool,
    runtime: &O,
) -> Result<CleanupReport> {
    let directory = lease_directory(project)?;
    let mut report = CleanupReport {
        schema_version: CLEANUP_REPORT_SCHEMA_VERSION,
        owner_marker: OWNER_MARKER,
        dry_run,
        scanned: 0,
        active: 0,
        stale: 0,
        reclaimed: 0,
        resources: Vec::new(),
        failures: Vec::new(),
    };
    if !directory.is_dir() {
        return Ok(report);
    }
    for entry in fs::read_dir(&directory)
        .with_context(|| format!("read lease directory {}", directory.display()))?
    {
        let entry = entry.context("read lease entry")?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        report.scanned += 1;
        let record = match read_record(&path) {
            Ok(record) => record,
            Err(error) => {
                report.failures.push(format!("{}: {error}", path.display()));
                continue;
            }
        };
        if let Err(error) =
            validate_record(&record, &record.resource_id, &path, &directory, project)
        {
            // Unknown or malformed markers are intentionally never reclaimed,
            // but the failure is retained as structured cleanup evidence.
            report.failures.push(format!(
                "{}: ownership validation failed: {error:#}",
                path.display()
            ));
            continue;
        }
        // Report retention, allocation, release and operator cleanup share this
        // guard. A heartbeat's JSON replacement cannot look like a released lease.
        let report_guard = if resource_kind_is_report(&record) {
            match ReportDirectoryGuard::try_acquire(project, &record.invocation_id) {
                Ok(Some(guard)) => {
                    let checked = read_record(&path).and_then(|current| {
                        ensure_owner(&current, &record)?;
                        guard.validate_record(&current)
                    });
                    if let Err(error) = checked {
                        report
                            .failures
                            .push(format!("{}: {error:#}", path.display()));
                        continue;
                    }
                    Some(guard)
                }
                Ok(None) => {
                    report.active += 1;
                    report.resources.push(CleanupResource {
                        resource_id: record.resource_id,
                        resource_kind: record.resource_kind,
                        invocation_id: record.invocation_id,
                        state: "active".into(),
                        action: "retained".into(),
                        lease_file: path.display().to_string(),
                    });
                    continue;
                }
                Err(error) => {
                    report
                        .failures
                        .push(format!("{}: {error:#}", path.display()));
                    continue;
                }
            }
        } else {
            None
        };
        let stale = if report_guard.is_some() {
            // Expiry is not proof of death when process observation is unknown.
            match report_owner_has_ended(&record) {
                Some(ended) => ended,
                None => {
                    report.failures.push(format!(
                        "{}: LEASE_OWNERSHIP_UNCERTAIN: report owner state is unknown; retained",
                        path.display()
                    ));
                    continue;
                }
            }
        } else {
            is_stale(&record, epoch_seconds())
        };
        let lease_file = path.display().to_string();
        if !identity_is_proven(&record) {
            if stale {
                report.stale += 1;
            } else {
                report.active += 1;
            }
            report.failures.push(format!(
                "{}: LEASE_OWNERSHIP_UNCERTAIN: platform process identity is unavailable; resource retained",
                record.resource_id
            ));
            report.resources.push(CleanupResource {
                resource_id: record.resource_id,
                resource_kind: record.resource_kind,
                invocation_id: record.invocation_id,
                state: "ownership-uncertain".into(),
                action: "retained".into(),
                lease_file,
            });
            continue;
        }
        if !stale {
            report.active += 1;
            report.resources.push(CleanupResource {
                resource_id: record.resource_id,
                resource_kind: record.resource_kind,
                invocation_id: record.invocation_id,
                state: "active".into(),
                action: "保留".into(),
                lease_file,
            });
            continue;
        }
        report.stale += 1;
        let mut action = if dry_run {
            "would-reclaim"
        } else {
            "reclaimed"
        };
        if !dry_run {
            if let Err(error) = reclaim_resource_with_runtime(project, &path, &record, runtime) {
                action = "failed";
                report.failures.push(format!(
                    "reclaim {} ({}) failed: {error:#}",
                    record.resource_id, record.resource_kind
                ));
            } else {
                report.reclaimed += 1;
            }
        }
        report.resources.push(CleanupResource {
            resource_id: record.resource_id,
            resource_kind: record.resource_kind,
            invocation_id: record.invocation_id,
            state: "stale".into(),
            action: action.into(),
            lease_file,
        });
    }
    Ok(report)
}

fn reclaim_resource(project: &Project, path: &Path, record: &LeaseRecord) -> Result<()> {
    reclaim_resource_with_runtime(project, path, record, &CliRuntimeOperations)
}

fn reclaim_resource_with_runtime<O: RuntimeOperations + ?Sized>(
    project: &Project,
    path: &Path,
    record: &LeaseRecord,
    runtime_operations: &O,
) -> Result<()> {
    if record.resource_kind == "container" {
        let name = record
            .resource_name
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("container lease has no resource name"))?;
        if !name.starts_with("harness-gate-") {
            bail!("container lease resource name is not Harness-Gate managed");
        }
        let runtime = match record.runtime.as_deref() {
            Some("docker") => ContainerRuntimeKind::Docker,
            Some("podman") => ContainerRuntimeKind::Podman,
            Some(value) => bail!("unsupported container runtime {value:?}"),
            None => bail!("container lease has no runtime"),
        };
        verify_runtime_record(runtime_operations, project, path, record)?;
        runtime_operations
            .stop(runtime, &project.root, name, Duration::from_secs(5))
            .with_context(|| format!("stop owned container {name:?}"))?;
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("remove lease {}", path.display())),
    }
}

fn create_record(path: &Path, record: &LeaseRecord) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let contents = serde_json::to_vec_pretty(record)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    file.write_all(&contents)?;
    file.write_all(b"\n")?;
    file.sync_all()
}

fn read_record(path: &Path) -> Result<LeaseRecord> {
    let contents = fs::read(path).with_context(|| format!("read lease {}", path.display()))?;
    serde_json::from_slice(&contents).with_context(|| format!("parse lease {}", path.display()))
}

fn write_record(path: &Path, record: &LeaseRecord) -> Result<()> {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temporary = path.with_file_name(format!(".lease-{counter}.tmp"));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .with_context(|| format!("create temporary lease {}", temporary.display()))?;
        let contents = serde_json::to_vec_pretty(record).context("serialize lease")?;
        file.write_all(&contents)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        publish_replacement(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn publish_replacement(temporary: &Path, target: &Path) -> Result<()> {
    #[cfg(windows)]
    if fs::symlink_metadata(target).is_ok() {
        fs::remove_file(target)
            .with_context(|| format!("replace existing lease {}", target.display()))?;
    }
    fs::rename(temporary, target).with_context(|| format!("publish lease {}", target.display()))?;
    Ok(())
}

fn validate_record(
    record: &LeaseRecord,
    resource_id: &str,
    path: &Path,
    lease_directory: &Path,
    project: &Project,
) -> Result<()> {
    if record.owner_marker != OWNER_MARKER {
        bail!("lease owner marker is not Harness-Gate");
    }
    if record.schema_version != LEASE_SCHEMA_VERSION {
        bail!("unsupported lease schema version {}", record.schema_version);
    }
    if record.resource_id != resource_id {
        bail!("lease resource identity mismatch");
    }
    let expected_name = format!("{}.json", resource_key(&record.resource_id));
    if path.file_name().and_then(|name| name.to_str()) != Some(expected_name.as_str()) {
        bail!("lease filename does not match the deterministic resource key");
    }
    if path.parent() != Some(lease_directory) {
        bail!("lease is not directly inside the project lease directory");
    }
    if record.project_identity != project.input().project_identity {
        bail!("lease project identity does not match the current project");
    }
    if record.resource_id.trim().is_empty()
        || record.resource_kind.trim().is_empty()
        || record.invocation_id.trim().is_empty()
    {
        bail!("lease ownership fields must be non-empty");
    }
    if record.resource_kind == "container" {
        if record
            .resource_name
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            bail!("container lease has no resource name");
        }
        if record.runtime.is_none() {
            bail!("container lease has no runtime");
        }
        if record
            .runtime_object_id
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            bail!("container lease has no immutable runtime object ID");
        }
        let expected = ownership_labels(
            &record.project_identity,
            &record.resource_id,
            &record.resource_kind,
            &record.invocation_id,
        );
        for (key, value) in expected {
            if record.runtime_labels.get(&key) != Some(&value) {
                bail!("container lease is missing expected runtime label {key:?}");
            }
        }
    }
    Ok(())
}

fn ensure_owner(current: &LeaseRecord, expected: &LeaseRecord) -> Result<()> {
    if current.owner_marker != OWNER_MARKER
        || current.schema_version != LEASE_SCHEMA_VERSION
        || current.resource_id != expected.resource_id
        || current.project_identity != expected.project_identity
        || current.resource_kind != expected.resource_kind
    {
        bail!("lease ownership changed while operating on the resource");
    }
    if current.invocation_id != expected.invocation_id
        || current.pid != expected.pid
        || current.process_start_identity != expected.process_start_identity
    {
        bail!("lease ownership changed while operating on the resource");
    }
    Ok(())
}

fn is_stale(record: &LeaseRecord, now: u64) -> bool {
    match process_alive(record.pid) {
        Some(true) => match process_start_identity_checked(record.pid) {
            Some(identity) if identity != record.process_start_identity => true,
            Some(_) => false,
            None => now > record.expires_at,
        },
        Some(false) => true,
        None => now > record.expires_at,
    }
}

fn identity_is_proven(record: &LeaseRecord) -> bool {
    #[cfg(target_os = "linux")]
    {
        record
            .process_start_identity
            .strip_prefix("linux:")
            .and_then(|value| value.parse::<u64>().ok())
            .is_some_and(|value| value > 0)
    }
    #[cfg(target_os = "macos")]
    {
        let mut fields = record.process_start_identity.split(':');
        let prefix = fields.next();
        let seconds = fields.next().and_then(|value| value.parse::<u64>().ok());
        let micros = fields.next().and_then(|value| value.parse::<u64>().ok());
        matches!(fields.next(), None)
            && matches!(prefix, Some("macos"))
            && seconds
                .zip(micros)
                .is_some_and(|(seconds, micros)| seconds > 0 || micros > 0)
    }
    #[cfg(target_os = "windows")]
    {
        record
            .process_start_identity
            .strip_prefix("windows:")
            .and_then(|value| value.parse::<u64>().ok())
            .is_some_and(|value| value > 0)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

fn resource_kind_is_report(record: &LeaseRecord) -> bool {
    record.resource_kind == "report-directory"
}

fn report_owner_has_ended(record: &LeaseRecord) -> Option<bool> {
    if !identity_is_proven(record) {
        return None;
    }
    if process_alive(record.pid) == Some(false) {
        return Some(true);
    }
    process_start_identity_checked(record.pid)
        .map(|identity| identity != record.process_start_identity)
}

pub(super) fn resource_key(resource_id: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(resource_id.as_bytes());
    let encoded = format!("{:x}", digest.finalize());
    encoded[..16].to_string()
}

pub(crate) fn ownership_labels(
    project_identity: &str,
    resource_id: &str,
    resource_kind: &str,
    invocation_id: &str,
) -> BTreeMap<String, String> {
    BTreeMap::from([
        (LABEL_OWNER.into(), OWNER_MARKER.into()),
        (LABEL_SCHEMA.into(), LEASE_SCHEMA_VERSION.to_string()),
        (LABEL_PROJECT.into(), project_identity.into()),
        (LABEL_RESOURCE.into(), resource_id.into()),
        (LABEL_KIND.into(), resource_kind.into()),
        (LABEL_INVOCATION.into(), invocation_id.into()),
    ])
}

fn validate_runtime_ownership(
    project: &Project,
    path: &Path,
    record: &LeaseRecord,
    inspection: &super::inspection::RuntimeInspection,
) -> Result<()> {
    let expected_name = record.resource_name.as_deref().unwrap_or_default();
    if expected_name.trim().is_empty() {
        bail!("container lease has no resource name");
    }
    if inspection.name != expected_name.trim_start_matches('/') {
        bail!("runtime object name does not match the lease resource name");
    }
    let expected = ownership_labels(
        &record.project_identity,
        &record.resource_id,
        &record.resource_kind,
        &record.invocation_id,
    );
    for (key, value) in expected {
        if inspection.labels.get(&key) != Some(&value) {
            bail!("runtime object is missing or mismatches ownership label {key:?}");
        }
    }
    if record.project_identity != project.input().project_identity {
        bail!("lease project identity does not match the current project");
    }
    if path.file_name().and_then(|name| name.to_str())
        != Some(format!("{}.json", resource_key(&record.resource_id)).as_str())
    {
        bail!("lease filename does not match the deterministic resource key");
    }
    Ok(())
}

fn verify_runtime_record<O: RuntimeOperations + ?Sized>(
    runtime_operations: &O,
    project: &Project,
    path: &Path,
    record: &LeaseRecord,
) -> Result<()> {
    let runtime = match record.runtime.as_deref() {
        Some("docker") => ContainerRuntimeKind::Docker,
        Some("podman") => ContainerRuntimeKind::Podman,
        Some(value) => bail!("unsupported container runtime {value:?}"),
        None => bail!("container lease has no runtime"),
    };
    let name = record
        .resource_name
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("container lease has no resource name"))?;
    let inspection = runtime_operations.inspect(runtime, project, name, Duration::from_secs(5))?;
    validate_runtime_ownership(project, path, record, &inspection)?;
    let object_id = record
        .runtime_object_id
        .as_deref()
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("container lease has no immutable runtime object ID"))?;
    if inspection.object_id != object_id {
        bail!("runtime object identity changed since the lease was recorded");
    }
    for (key, value) in &record.runtime_labels {
        if inspection.labels.get(key) != Some(value) {
            bail!("runtime ownership label {key:?} changed since lease creation");
        }
    }
    Ok(())
}

pub(super) fn lease_directory(project: &Project) -> Result<PathBuf> {
    let repository = project
        .root
        .canonicalize()
        .with_context(|| format!("resolve project root {}", project.root.display()))?;
    let path = &project.resource_leases;
    let resolved = if fs::symlink_metadata(path).is_ok() {
        path.canonicalize()
            .with_context(|| format!("resolve lease directory {}", path.display()))?
    } else {
        let mut ancestor = path.as_path();
        while fs::symlink_metadata(ancestor).is_err() {
            ancestor = ancestor
                .parent()
                .ok_or_else(|| anyhow::anyhow!("lease directory has no resolvable parent"))?;
        }
        let resolved_ancestor = ancestor
            .canonicalize()
            .with_context(|| format!("resolve lease directory parent {}", ancestor.display()))?;
        if !resolved_ancestor.starts_with(&repository) {
            bail!("lease directory escapes project root");
        }
        fs::create_dir_all(path)
            .with_context(|| format!("create lease directory {}", path.display()))?;
        path.canonicalize()
            .with_context(|| format!("resolve lease directory {}", path.display()))?
    };
    if !resolved.starts_with(&repository) {
        bail!("lease directory escapes project root");
    }
    Ok(resolved)
}

fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn process_start_identity(pid: u32) -> String {
    process_start_identity_checked(pid).unwrap_or_else(|| format!("unavailable:{pid}"))
}

#[cfg(target_os = "linux")]
fn process_start_identity_checked(pid: u32) -> Option<String> {
    let contents = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, rest) = contents.rsplit_once(") ")?;
    let start_time = rest.split_whitespace().nth(19)?;
    let start_time = start_time.parse::<u64>().ok()?;
    (start_time > 0).then(|| format!("linux:{start_time}"))
}

#[cfg(target_os = "macos")]
fn process_start_identity_checked(pid: u32) -> Option<String> {
    let mut info = unsafe { std::mem::zeroed::<libc::proc_bsdinfo>() };
    let expected_size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    let observed = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            expected_size,
        )
    };
    if observed != expected_size {
        return None;
    }
    (info.pbi_start_tvsec > 0 || info.pbi_start_tvusec > 0)
        .then(|| format!("macos:{}:{}", info.pbi_start_tvsec, info.pbi_start_tvusec))
}

#[cfg(target_os = "windows")]
fn process_start_identity_checked(pid: u32) -> Option<String> {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    if pid == 0 {
        return None;
    }
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let mut creation = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut exit = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut kernel = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut user = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let available =
        unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) != 0 };
    unsafe {
        CloseHandle(handle);
    }
    if !available {
        return None;
    }
    let ticks = (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime);
    (ticks > 0).then(|| format!("windows:{ticks}"))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn process_start_identity_checked(_pid: u32) -> Option<String> {
    None
}

#[cfg(unix)]
fn process_alive(pid: u32) -> Option<bool> {
    if pid == 0 {
        return Some(false);
    }
    // kill(pid, 0) checks existence without sending a signal. EPERM means the
    // process exists but is not inspectable by the current user.
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if result == 0 {
        Some(true)
    } else {
        match std::io::Error::last_os_error().raw_os_error() {
            Some(libc::ESRCH) => Some(false),
            Some(libc::EPERM) => Some(true),
            _ => None,
        }
    }
}

#[cfg(not(unix))]
fn process_alive(pid: u32) -> Option<bool> {
    if pid == std::process::id() {
        Some(true)
    } else {
        // Unknown Windows process state is treated conservatively; expiry is
        // still required before a lease can be reclaimed.
        None
    }
}

#[cfg(test)]
mod tests {
    use super::process_start_identity_checked;
    use super::{
        cleanup_with_runtime, is_stale, ownership_labels, read_record, resource_key, write_record,
        LeaseRecord, RuntimeOperations, LEASE_SCHEMA_VERSION, OWNER_MARKER,
    };
    use crate::config::ContainerRuntimeKind;
    use crate::project::Project;
    use crate::service::inspection::RuntimeInspection;
    use crate::test_support::TestWorkspace;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    #[derive(Clone)]
    struct FakeRuntime {
        expected_kind: ContainerRuntimeKind,
        inspection: Option<RuntimeInspection>,
        remove_calls: Arc<AtomicUsize>,
        stop_fails: bool,
    }

    impl RuntimeOperations for FakeRuntime {
        fn inspect(
            &self,
            runtime: ContainerRuntimeKind,
            _project: &Project,
            _name: &str,
            _timeout: Duration,
        ) -> anyhow::Result<RuntimeInspection> {
            if runtime != self.expected_kind {
                anyhow::bail!("fake runtime kind mismatch");
            }
            self.inspection
                .clone()
                .ok_or_else(|| anyhow::anyhow!("fake inspection failed"))
        }

        fn stop(
            &self,
            runtime: ContainerRuntimeKind,
            _cwd: &Path,
            _name: &str,
            _timeout: Duration,
        ) -> anyhow::Result<()> {
            if runtime != self.expected_kind {
                anyhow::bail!("fake runtime kind mismatch");
            }
            self.remove_calls.fetch_add(1, Ordering::SeqCst);
            if self.stop_fails {
                anyhow::bail!("injected runtime removal failure");
            }
            Ok(())
        }
    }

    fn runtime_project(name: &str) -> (TestWorkspace, Project) {
        let workspace = TestWorkspace::new(name);
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        let project =
            Project::discover(Some(workspace.root.clone()), None).expect("discover fixture");
        (workspace, project)
    }

    fn container_lease(
        project: &Project,
        runtime: ContainerRuntimeKind,
        stale: bool,
    ) -> (PathBuf, LeaseRecord, RuntimeInspection) {
        let resource_id = "service:database";
        let invocation_id = "invocation-runtime-fixture";
        let resource_name = "harness-gate-fixture-container";
        let mut lease = super::ResourceLease::acquire(
            project,
            resource_id,
            "container",
            invocation_id,
            Some(resource_name.into()),
            Some(runtime),
        )
        .expect("acquire container lease");
        let path = lease.path.clone();
        // Freeze the owner before writing the synthetic runtime identity and
        // stale/active state so heartbeat writes cannot overwrite the fixture.
        lease.stop_heartbeat();
        let mut record = read_record(&path).expect("read fixture lease");
        let inspection = RuntimeInspection {
            object_id: "runtime-object-1".into(),
            name: resource_name.into(),
            labels: ownership_labels(
                &project.input().project_identity,
                resource_id,
                "container",
                invocation_id,
            ),
        };
        record.runtime_object_id = Some(inspection.object_id.clone());
        record.runtime_labels = inspection.labels.clone();
        if stale {
            record.pid = 0;
            record.process_start_identity = proven_identity_fixture();
            record.expires_at = 0;
        } else {
            record.expires_at = record.expires_at.saturating_add(3600);
        }
        write_record(&path, &record).expect("write fixture lease");
        // The fixture intentionally leaves the marker for cleanup to inspect.
        lease.retain();
        (path, record, inspection)
    }

    fn proven_identity_fixture() -> String {
        #[cfg(target_os = "linux")]
        {
            "linux:1".into()
        }
        #[cfg(target_os = "macos")]
        {
            "macos:1:0".into()
        }
        #[cfg(target_os = "windows")]
        {
            "windows:1".into()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            "unavailable:test".into()
        }
    }

    fn fake_runtime(
        kind: ContainerRuntimeKind,
        inspection: Option<RuntimeInspection>,
    ) -> (FakeRuntime, Arc<AtomicUsize>) {
        let remove_calls = Arc::new(AtomicUsize::new(0));
        (
            FakeRuntime {
                expected_kind: kind,
                inspection,
                remove_calls: Arc::clone(&remove_calls),
                stop_fails: false,
            },
            remove_calls,
        )
    }

    fn assert_failed_without_remove(
        project: &Project,
        path: &Path,
        runtime: &FakeRuntime,
        expected_code: &str,
    ) {
        let report = cleanup_with_runtime(project, false, runtime).expect("cleanup report");
        assert_eq!(runtime.remove_calls.load(Ordering::SeqCst), 0);
        assert!(path.exists(), "ambiguous lease must remain available");
        assert!(
            report
                .failures
                .iter()
                .any(|failure| failure.contains(expected_code)),
            "expected {expected_code} in {:?}",
            report.failures
        );
    }

    #[test]
    fn resource_keys_are_stable_and_path_safe() {
        let key = resource_key("service:database");
        assert_eq!(key.len(), 16);
        assert!(key.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(key, resource_key("service:database"));
    }

    #[test]
    fn current_process_identity_is_available_on_supported_platforms() {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        assert!(process_start_identity_checked(std::process::id()).is_some());
    }

    #[test]
    fn expired_process_lease_is_stale() {
        let record = LeaseRecord {
            owner_marker: OWNER_MARKER.into(),
            schema_version: LEASE_SCHEMA_VERSION,
            project_identity: "fixture-project".into(),
            resource_id: "fixture".into(),
            resource_kind: "workspace".into(),
            invocation_id: "invocation".into(),
            pid: 0,
            process_start_identity: "unknown".into(),
            created_at: 1,
            heartbeat_at: 1,
            expires_at: 1,
            resource_name: None,
            runtime: None,
            runtime_labels: BTreeMap::new(),
            runtime_object_id: None,
        };
        assert!(is_stale(&record, 2));
    }

    #[test]
    fn lease_heartbeat_renews_marker_during_a_long_step() {
        let (_workspace, project) = runtime_project("lease-heartbeat");
        let lease = super::ResourceLease::acquire(
            &project,
            "step:long-running",
            "workspace",
            "invocation-heartbeat",
            None,
            None,
        )
        .expect("acquire heartbeat lease");
        let path = lease.path.clone();
        let mut stale = read_record(&path).expect("read heartbeat lease");
        stale.heartbeat_at = 0;
        stale.expires_at = 0;
        write_record(&path, &stale).expect("write stale heartbeat fixture");

        let deadline = Instant::now() + Duration::from_secs(15);
        let renewed = loop {
            match read_record(&path) {
                Ok(record)
                    if record.heartbeat_at > 0 && record.expires_at > record.heartbeat_at =>
                {
                    break record;
                }
                Ok(_) | Err(_) => {
                    // `write_record` replaces the marker by removing and
                    // renaming on Windows, so a concurrent read can briefly
                    // observe a missing file between the two operations.
                    assert!(
                        Instant::now() < deadline,
                        "heartbeat did not renew the lease marker before the deadline"
                    );
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        };
        assert!(renewed.heartbeat_at > 0);
        assert!(renewed.expires_at > renewed.heartbeat_at);

        drop(lease);
        // Windows can briefly delay marker removal when the heartbeat thread
        // just closed the same file (antivirus/indexer scanning or handle
        // release timing). Poll instead of asserting an immediate delete so a
        // loaded CI runner does not turn a transient deletion delay into a
        // flaky failure.
        let cleanup_deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if !path.exists() {
                break;
            }
            assert!(
                Instant::now() < cleanup_deadline,
                "drop must stop heartbeat and release marker before the cleanup deadline"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    #[test]
    fn heartbeat_failure_blocks_release_and_retains_the_marker() {
        let (_workspace, project) = runtime_project("lease-heartbeat-failure");
        let mut lease = super::ResourceLease::acquire(
            &project,
            "step:heartbeat-failure",
            "workspace",
            "invocation-heartbeat-failure",
            None,
            None,
        )
        .expect("acquire heartbeat lease");
        let path = lease.path.clone();
        // Make the real heartbeat's filesystem read fail, without racing an
        // in-flight renewal or replacing its error field with a mock result.
        let original = {
            let _record = lease.record.lock().expect("lock renewal");
            let original = std::fs::read(&path).expect("read ownership evidence");
            std::fs::remove_file(&path).expect("remove marker");
            std::fs::create_dir(&path).expect("obstruct marker read");
            original
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while lease
            .heartbeat_error
            .lock()
            .expect("heartbeat error lock")
            .is_none()
        {
            assert!(
                Instant::now() < deadline,
                "heartbeat failed to observe filesystem fault"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        // Join the heartbeat while reads are still obstructed. Otherwise a
        // later renewal can rewrite the restored marker before release_checked
        // stops the thread, racing the byte-for-byte retention assertion.
        lease.stop_heartbeat();
        std::fs::remove_dir(&path).expect("remove obstruction");
        std::fs::write(&path, &original).expect("restore ownership evidence");

        let error = lease
            .release_checked()
            .expect_err("uncertain ownership must block release");
        assert!(format!("{error:#}").contains("LEASE_OWNERSHIP_UNCERTAIN"));
        assert!(path.exists(), "failed release must retain its marker");
        assert_eq!(std::fs::read(path).unwrap(), original);
    }

    fn retained_report_project(name: &str) -> (Option<tempfile::TempDir>, Project) {
        let (temporary, root) = match std::env::var_os("GH286_RETENTION_EVIDENCE") {
            Some(parent) => {
                std::fs::create_dir_all(&parent).unwrap();
                let temporary = tempfile::Builder::new()
                    .prefix(name)
                    .tempdir_in(parent)
                    .unwrap();
                (None, temporary.keep())
            }
            None => {
                let temporary = tempfile::Builder::new().prefix(name).tempdir().unwrap();
                let root = temporary.path().to_path_buf();
                (Some(temporary), root)
            }
        };
        crate::preset::init(&root, "generic", false).unwrap();
        let output = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(output.status.success());
        let project = Project::discover(Some(root), None).unwrap();
        (temporary, project)
    }

    fn report_lease(project: &Project, id: &str) -> (super::ResourceLease, PathBuf) {
        let root = project.reports.join("invocations").join(id);
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let lease = super::ResourceLease::acquire(
            project,
            format!("invocation:{id}"),
            "report-directory",
            id,
            Some(root.to_string_lossy().into_owned()),
            None,
        )
        .unwrap();
        (lease, root)
    }

    #[test]
    fn report_cleanup_shares_live_guard_and_preserves_expired_unknown_ownership() {
        let (_workspace, project) = retained_report_project("gh286-report-cleanup-");
        let (lease, root) = report_lease(&project, "inv-cleanup-guard");
        let path = lease.path.clone();
        let (runtime, stop_calls) = fake_runtime(ContainerRuntimeKind::Docker, None);
        let report = cleanup_with_runtime(&project, false, &runtime).unwrap();
        std::fs::write(
            project.root.join("live-cleanup.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        assert_eq!(report.active, 1);
        assert_eq!(report.reclaimed, 0);
        assert!(path.is_file());
        assert!(
            super::ReportDirectoryGuard::try_acquire(&project, "inv-cleanup-guard")
                .unwrap()
                .is_none()
        );
        lease.retain();
        let mut unknown = read_record(&path).unwrap();
        unknown.pid = 0;
        unknown.process_start_identity = "unavailable:expired-fixture".into();
        unknown.heartbeat_at = 0;
        unknown.expires_at = 0;
        write_record(&path, &unknown).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let report = cleanup_with_runtime(&project, false, &runtime).unwrap();
        std::fs::write(
            project.root.join("unknown-cleanup.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        assert_eq!(report.reclaimed, 0);
        assert!(report
            .failures
            .iter()
            .any(|failure| failure.contains("LEASE_OWNERSHIP_UNCERTAIN")));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(stop_calls.load(Ordering::SeqCst), 0);
        let guard = super::ReportDirectoryGuard::try_acquire(&project, "inv-cleanup-guard")
            .unwrap()
            .unwrap();
        assert!(!guard.release_is_proven(&root).unwrap());
    }

    #[test]
    fn report_real_heartbeat_failure_cannot_certify_a_release_even_without_marker() {
        let (_workspace, project) = retained_report_project("gh286-report-heartbeat-fault-");
        let (mut lease, root) = report_lease(&project, "inv-heartbeat-fault");
        let path = lease.path.clone();
        let original = {
            let _record = lease.record.lock().unwrap();
            let original = std::fs::read(&path).unwrap();
            std::fs::remove_file(&path).unwrap();
            std::fs::create_dir(&path).unwrap();
            original
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while lease.heartbeat_error.lock().unwrap().is_none() {
            assert!(
                Instant::now() < deadline,
                "real heartbeat did not observe the read fault"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        std::fs::write(
            project.root.join("heartbeat-error.txt"),
            lease.heartbeat_error.lock().unwrap().as_deref().unwrap(),
        )
        .unwrap();
        lease.stop_heartbeat();
        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, &original).unwrap();
        let error = lease.release_checked().unwrap_err();
        std::fs::write(project.root.join("release-error.txt"), format!("{error:#}")).unwrap();
        assert!(format!("{error:#}").contains("lease heartbeat failed"));
        assert_eq!(std::fs::read(&path).unwrap(), original);
        // Missing JSON cannot bypass the failed-release state in the stable,
        // locked certificate. No PID or TTL inference is used by retention.
        std::fs::remove_file(&path).unwrap();
        let guard = super::ReportDirectoryGuard::try_acquire(&project, "inv-heartbeat-fault")
            .unwrap()
            .unwrap();
        assert!(!guard.release_is_proven(&root).unwrap());
    }

    #[test]
    fn runtime_removal_failure_retains_evidence_and_allows_a_proven_retry() {
        for kind in [ContainerRuntimeKind::Docker, ContainerRuntimeKind::Podman] {
            let (_workspace, project) = runtime_project("lease-removal-fault");
            let (path, _, inspection) = container_lease(&project, kind, true);
            let original = std::fs::read(&path).unwrap();
            let (mut runtime, calls) = fake_runtime(kind, Some(inspection));
            runtime.stop_fails = true;
            let report = cleanup_with_runtime(&project, false, &runtime).unwrap();
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_eq!(report.reclaimed, 0);
            assert!(report
                .failures
                .iter()
                .any(|failure| failure.contains("injected runtime removal failure")));
            assert_eq!(std::fs::read(&path).unwrap(), original);
            // Loss of inspection certainty must not trigger a second remove.
            let inspection = runtime.inspection.take();
            assert_failed_without_remove(
                &project,
                &path,
                &fake_runtime(kind, None).0,
                "fake inspection failed",
            );
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            runtime.inspection = inspection;
            runtime.stop_fails = false;
            let report = cleanup_with_runtime(&project, false, &runtime).unwrap();
            assert_eq!(calls.load(Ordering::SeqCst), 2);
            assert_eq!(report.reclaimed, 1);
            assert!(report.failures.is_empty());
            assert!(!path.exists());
        }
    }

    #[test]
    fn poisoned_lease_lock_fails_closed_and_retains_the_marker() {
        let (_workspace, project) = runtime_project("lease-lock-poison");
        let mut lease = super::ResourceLease::acquire(
            &project,
            "step:poisoned-lock",
            "workspace",
            "invocation-poisoned-lock",
            None,
            None,
        )
        .expect("acquire lease");
        let path = lease.path.clone();
        lease.stop_heartbeat();
        let record = std::sync::Arc::clone(&lease.record);
        let poisoned = std::thread::spawn(move || {
            let _guard = record.lock().expect("lock lease record");
            panic!("poison lease record fixture");
        })
        .join();
        assert!(poisoned.is_err());

        let error = lease
            .release_checked()
            .expect_err("poisoned ownership state must block release");
        assert!(format!("{error:#}").contains("lease record lock was poisoned"));
        assert!(path.exists(), "poisoned ownership marker must be retained");
    }

    #[test]
    fn uncertain_process_identity_is_retained_without_reclaim() {
        let (_workspace, project) = runtime_project("lease-identity-uncertain");
        let mut lease = super::ResourceLease::acquire(
            &project,
            "step:uncertain",
            "workspace",
            "invocation-uncertain",
            None,
            None,
        )
        .expect("acquire uncertain identity lease");
        let path = lease.path.clone();
        // Stop renewal before forging an expired/uncertain fixture. Otherwise
        // the heartbeat can race this test and restore the live identity.
        lease.stop_heartbeat();
        let mut forged = read_record(&path).expect("read uncertain identity lease");
        forged.pid = 0;
        forged.process_start_identity = "unavailable:test".into();
        forged.heartbeat_at = 0;
        forged.expires_at = 0;
        write_record(&path, &forged).expect("write uncertain identity fixture");
        lease.retain();

        let (fake, remove_calls) = fake_runtime(ContainerRuntimeKind::Docker, None);
        let report = cleanup_with_runtime(&project, false, &fake).expect("cleanup report");
        assert_eq!(report.reclaimed, 0);
        assert_eq!(remove_calls.load(Ordering::SeqCst), 0);
        assert!(path.exists(), "uncertain ownership must be retained");
        assert!(report
            .failures
            .iter()
            .any(|failure| failure.contains("LEASE_OWNERSHIP_UNCERTAIN")));
        assert_eq!(
            report.resources[0].state, "ownership-uncertain",
            "cleanup evidence must expose the uncertain state"
        );
    }

    #[test]
    fn fake_docker_and_podman_cleanup_requires_fresh_complete_ownership() {
        for (index, runtime) in [ContainerRuntimeKind::Docker, ContainerRuntimeKind::Podman]
            .into_iter()
            .enumerate()
        {
            let (workspace, project) = runtime_project(&format!("runtime-owned-{index}"));
            let (path, _record, inspection) = container_lease(&project, runtime, true);
            let (fake, remove_calls) = fake_runtime(runtime, Some(inspection.clone()));
            let report = cleanup_with_runtime(&project, false, &fake).expect("cleanup report");
            assert_eq!(report.reclaimed, 1);
            assert!(report.failures.is_empty());
            assert_eq!(remove_calls.load(Ordering::SeqCst), 1);
            assert!(!path.exists());
            drop(workspace);

            let (workspace, project) = runtime_project(&format!("runtime-object-{index}"));
            let (path, _record, mut mismatch) = container_lease(&project, runtime, true);
            mismatch.object_id = "replacement-object".into();
            let (fake, _) = fake_runtime(runtime, Some(mismatch));
            assert_failed_without_remove(&project, &path, &fake, "runtime object identity changed");
            drop(workspace);

            let (workspace, project) = runtime_project(&format!("runtime-label-{index}"));
            let (path, _record, mut mismatch) = container_lease(&project, runtime, true);
            mismatch
                .labels
                .insert(super::LABEL_PROJECT.into(), "other-project".into());
            let (fake, _) = fake_runtime(runtime, Some(mismatch));
            assert_failed_without_remove(&project, &path, &fake, "ownership label");
            drop(workspace);

            let (workspace, project) = runtime_project(&format!("runtime-renamed-{index}"));
            let (path, _record, mut mismatch) = container_lease(&project, runtime, true);
            mismatch.name = "harness-gate-renamed-container".into();
            let (fake, _) = fake_runtime(runtime, Some(mismatch));
            assert_failed_without_remove(&project, &path, &fake, "object name");
            drop(workspace);

            let (workspace, project) = runtime_project(&format!("runtime-inspect-{index}"));
            let (path, _record, _inspection) = container_lease(&project, runtime, true);
            let (fake, _) = fake_runtime(runtime, None);
            assert_failed_without_remove(&project, &path, &fake, "fake inspection failed");
            drop(workspace);

            let (workspace, project) = runtime_project(&format!("runtime-cross-project-{index}"));
            let (path, mut record, inspection) = container_lease(&project, runtime, true);
            record.project_identity = "other-project".into();
            write_record(&path, &record).expect("forge cross-project lease");
            let (fake, _) = fake_runtime(runtime, Some(inspection));
            assert_failed_without_remove(&project, &path, &fake, "project identity");
            drop(workspace);

            let (workspace, project) = runtime_project(&format!("runtime-forged-{index}"));
            let (path, mut record, inspection) = container_lease(&project, runtime, true);
            record.owner_marker = "forged".into();
            write_record(&path, &record).expect("forge owner marker");
            let (fake, _) = fake_runtime(runtime, Some(inspection));
            assert_failed_without_remove(&project, &path, &fake, "owner marker");
            drop(workspace);

            let (workspace, project) = runtime_project(&format!("runtime-renamed-lease-{index}"));
            let (path, _record, inspection) = container_lease(&project, runtime, true);
            let renamed = path.with_file_name("renamed-lease.json");
            std::fs::rename(&path, &renamed).expect("rename lease marker");
            let (fake, _) = fake_runtime(runtime, Some(inspection));
            assert_failed_without_remove(&project, &renamed, &fake, "deterministic resource key");
            drop(workspace);

            let (workspace, project) = runtime_project(&format!("runtime-malformed-{index}"));
            let directory = super::lease_directory(&project).expect("lease directory");
            let malformed = directory.join(format!("{}.json", resource_key("service:database")));
            std::fs::write(&malformed, b"not-json").expect("write malformed lease");
            let (fake, _) = fake_runtime(runtime, None);
            assert_failed_without_remove(&project, &malformed, &fake, "parse");
            drop(workspace);

            let (workspace, project) = runtime_project(&format!("runtime-active-{index}"));
            let (path, _record, inspection) = container_lease(&project, runtime, false);
            let (fake, remove_calls) = fake_runtime(runtime, Some(inspection));
            let report = cleanup_with_runtime(&project, false, &fake).expect("cleanup report");
            assert_eq!(report.active, 1);
            assert_eq!(report.reclaimed, 0);
            assert!(report.failures.is_empty());
            assert_eq!(remove_calls.load(Ordering::SeqCst), 0);
            assert!(path.exists());
            drop(workspace);
        }
    }

    fn report_lock_path(project: &Project, invocation_id: &str) -> PathBuf {
        super::lease_directory(project)
            .unwrap()
            .join("report-directory-locks")
            .join(format!(
                "{}.lock",
                resource_key(&format!("invocation:{invocation_id}"))
            ))
    }

    fn assert_report_guard_held(
        lease: &super::ResourceLease,
        project: &Project,
        root: &Path,
        id: &str,
    ) {
        assert!(lease.report_directory_guard.is_some());
        assert!(super::ReportDirectoryGuard::try_acquire(project, id)
            .unwrap()
            .is_none());
        assert!(!lease
            .report_directory_guard
            .as_ref()
            .unwrap()
            .release_is_proven(root)
            .unwrap());
    }

    #[test]
    fn direct_report_release_requires_stopped_heartbeat_and_keeps_held_state() {
        let (_workspace, project) = retained_report_project("gh286-release-live-");
        let id = "inv-release-live";
        let (lease, root) = report_lease(&project, id);
        // Prevent an actual renewal from changing the bytes during assertions.
        // release must reject before taking this record lock; checked release
        // is performed only after the lock has been dropped.
        {
            let _record = lease.record.lock().unwrap();
            let bytes = std::fs::read(&lease.path).unwrap();
            let error = lease.release().unwrap_err();
            assert!(format!("{error:#}").contains("requires stopped heartbeat"));
            assert_eq!(std::fs::read(&lease.path).unwrap(), bytes);
            assert_report_guard_held(&lease, &project, &root, id);
        }
        lease.retain();
        assert_eq!(
            std::fs::read(report_lock_path(&project, id)).unwrap(),
            b"held\n"
        );
    }

    #[test]
    fn stopped_report_release_rejects_missing_marker_without_certificate() {
        let (_workspace, project) = retained_report_project("gh286-release-missing-");
        let id = "inv-release-missing";
        let (mut lease, root) = report_lease(&project, id);
        lease.stop_heartbeat();
        std::fs::remove_file(&lease.path).unwrap();
        let error = lease.release().unwrap_err();
        assert!(format!("{error:#}").contains("marker disappeared before checked release"));
        assert!(!lease.path.exists());
        assert!(root.is_dir());
        assert_report_guard_held(&lease, &project, &root, id);
        lease.retain();
        assert_eq!(
            std::fs::read(report_lock_path(&project, id)).unwrap(),
            b"held\n"
        );
        let guard = super::ReportDirectoryGuard::try_acquire(&project, id)
            .unwrap()
            .unwrap();
        assert!(!guard.release_is_proven(&root).unwrap());
    }

    #[test]
    fn stopped_legacy_release_is_idempotent_when_marker_is_missing() {
        let (_workspace, project) = runtime_project("release-missing-legacy");
        let mut lease = super::ResourceLease::acquire(
            &project,
            "step:release-missing",
            "workspace",
            "inv-release-missing-legacy",
            None,
            None,
        )
        .unwrap();
        lease.stop_heartbeat();
        assert!(lease.report_directory_guard.is_none());
        let path = lease.path.clone();
        std::fs::remove_file(&path).unwrap();
        lease.release().unwrap();
        lease.release_checked().unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn report_release_real_read_error_keeps_obstruction_and_held_certificate() {
        let (_workspace, project) = retained_report_project("gh286-release-read-");
        let id = "inv-release-read";
        let (mut lease, root) = report_lease(&project, id);
        lease.stop_heartbeat();
        let path = lease.path.clone();
        let original = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        let actual = std::fs::read(&path).unwrap_err();
        assert_ne!(actual.kind(), std::io::ErrorKind::NotFound);
        let error = lease.release().unwrap_err();
        assert!(format!("{error:#}").contains("read lease before release"));
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            actual.kind()
        );
        assert!(path.is_dir());
        assert_eq!(std::fs::read_dir(&path).unwrap().count(), 0);
        assert_report_guard_held(&lease, &project, &root, id);
        lease.retain();
        assert_eq!(
            std::fs::read(report_lock_path(&project, id)).unwrap(),
            b"held\n"
        );
        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, &original).unwrap();
        let guard = super::ReportDirectoryGuard::try_acquire(&project, id)
            .unwrap()
            .unwrap();
        assert!(!guard.release_is_proven(&root).unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }

    #[test]
    fn report_release_malformed_marker_is_retained_without_certificate() {
        let (_workspace, project) = retained_report_project("gh286-release-parse-");
        let id = "inv-release-parse";
        let (mut lease, root) = report_lease(&project, id);
        lease.stop_heartbeat();
        std::fs::write(&lease.path, b"{not-json").unwrap();
        let error = lease.release().unwrap_err();
        assert!(format!("{error:#}").contains("parse lease before release"));
        assert_eq!(std::fs::read(&lease.path).unwrap(), b"{not-json");
        assert_report_guard_held(&lease, &project, &root, id);
        lease.retain();
        assert_eq!(
            std::fs::read(report_lock_path(&project, id)).unwrap(),
            b"held\n"
        );
    }

    #[test]
    fn report_release_unknown_owner_is_retained_without_certificate() {
        let (_workspace, project) = retained_report_project("gh286-release-owner-");
        let id = "inv-release-owner";
        let (mut lease, root) = report_lease(&project, id);
        lease.stop_heartbeat();
        let mut unknown = read_record(&lease.path).unwrap();
        unknown.invocation_id = "different-invocation".into();
        unknown.process_start_identity = "unavailable:unknown-owner".into();
        write_record(&lease.path, &unknown).unwrap();
        let bytes = std::fs::read(&lease.path).unwrap();
        let error = lease.release().unwrap_err();
        assert!(format!("{error:#}").contains("lease ownership changed"));
        assert_eq!(std::fs::read(&lease.path).unwrap(), bytes);
        assert_report_guard_held(&lease, &project, &root, id);
        lease.retain();
        assert_eq!(
            std::fs::read(report_lock_path(&project, id)).unwrap(),
            b"held\n"
        );
    }

    #[cfg(unix)]
    struct RestoreDirectoryPermissions {
        path: PathBuf,
        original: std::fs::Permissions,
    }

    #[cfg(unix)]
    impl Drop for RestoreDirectoryPermissions {
        fn drop(&mut self) {
            std::fs::set_permissions(&self.path, self.original.clone())
                .expect("restore lease directory permissions, including while unwinding");
        }
    }

    #[test]
    #[cfg(unix)]
    fn report_release_real_remove_error_retains_marker_then_allows_checked_retry() {
        use std::os::unix::fs::PermissionsExt;
        // Root or DAC-bypass privileges could turn a permissions fixture into
        // a successful remove. They are an explicit unsupported test identity,
        // never a silent skip or a mocked permission error.
        assert_ne!(
            unsafe { libc::geteuid() },
            0,
            "run this fixture as an unprivileged Unix identity"
        );
        let (_workspace, project) = retained_report_project("gh286-release-remove-");
        let id = "inv-release-remove";
        let (mut lease, root) = report_lease(&project, id);
        lease.stop_heartbeat();
        let path = lease.path.clone();
        let bytes = std::fs::read(&path).unwrap();
        let directory = path.parent().unwrap().to_path_buf();
        let restore = RestoreDirectoryPermissions {
            original: std::fs::metadata(&directory).unwrap().permissions(),
            path: directory.clone(),
        };
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o555)).unwrap();
        // Prove write denial against a real disposable sibling before release;
        // traversing/reading the existing marker must remain possible.
        let probe = directory.join("release-permission-probe");
        let actual = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
            .unwrap_err();
        assert_eq!(actual.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let error = lease.release().unwrap_err();
        assert!(format!("{error:#}").contains("release lease"));
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_report_guard_held(&lease, &project, &root, id);
        // The release certificate was persisted, but the still-present marker
        // blocks retention. Its exact bound fields are independently checked.
        let certificate: serde_json::Value =
            serde_json::from_slice(&std::fs::read(report_lock_path(&project, id)).unwrap())
                .unwrap();
        assert_eq!(
            certificate,
            serde_json::json!({
                "schema_version": 1,
                "invocation_id": id,
                "project_identity": project.input().project_identity,
                "root": root,
            })
        );
        drop(restore);
        lease.release_checked().unwrap();
        assert!(!path.exists());
        let guard = super::ReportDirectoryGuard::try_acquire(&project, id)
            .unwrap()
            .unwrap();
        assert!(guard.release_is_proven(&root).unwrap());
        assert!(root.is_dir());
    }

    #[test]
    fn report_release_invalid_certificate_binding_keeps_marker_and_held_state() {
        let (_workspace, project) = retained_report_project("gh286-release-binding-");
        let id = "inv-release-binding";
        let (mut lease, root) = report_lease(&project, id);
        lease.stop_heartbeat();
        // Ordinary ownership fields still agree. The real report guard must
        // independently reject a different root before writing its certificate.
        let invalid = {
            let mut record = lease.record.lock().unwrap();
            record.resource_name = Some(root.join("different-root").to_string_lossy().into_owned());
            record.clone()
        };
        write_record(&lease.path, &invalid).unwrap();
        let bytes = std::fs::read(&lease.path).unwrap();
        let error = lease.release().unwrap_err();
        assert!(format!("{error:#}").contains("report-directory lease binding is uncertain"));
        assert_eq!(std::fs::read(&lease.path).unwrap(), bytes);
        assert_report_guard_held(&lease, &project, &root, id);
        lease.retain();
        assert_eq!(
            std::fs::read(report_lock_path(&project, id)).unwrap(),
            b"held\n"
        );
    }
}
