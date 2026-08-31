use crate::config::ContainerRuntimeKind;
use crate::project::Project;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs::{self, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(crate) const LEASE_SCHEMA_VERSION: u32 = 1;
pub(crate) const OWNER_MARKER: &str = "harness-gate";
const LEASE_TTL: Duration = Duration::from_secs(15 * 60);
const RENEW_AFTER: Duration = Duration::from_secs(30);
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LeaseRecord {
    pub(crate) owner_marker: String,
    pub(crate) schema_version: u32,
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
}

#[derive(Debug)]
pub(crate) struct ResourceLease {
    path: PathBuf,
    record: Mutex<LeaseRecord>,
    last_renewed: Mutex<Instant>,
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
        let record = LeaseRecord {
            owner_marker: OWNER_MARKER.into(),
            schema_version: LEASE_SCHEMA_VERSION,
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
        };

        loop {
            match create_record(&path, &record) {
                Ok(()) => {
                    return Ok(Self {
                        path,
                        record: Mutex::new(record),
                        last_renewed: Mutex::new(Instant::now()),
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    let existing = read_record(&path).with_context(|| {
                        format!("inspect existing lease for resource {resource_id:?}")
                    })?;
                    validate_record(&existing, &resource_id)?;
                    if !is_stale(&existing, epoch_seconds()) {
                        bail!(
                            "resource lease conflict for {resource_id:?}: invocation {} (pid {}) owns it",
                            existing.invocation_id,
                            existing.pid
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
        let mut last = self
            .last_renewed
            .lock()
            .map_err(|_| anyhow::anyhow!("lease renewal lock was poisoned"))?;
        if last.elapsed() < RENEW_AFTER {
            return Ok(());
        }
        let mut record = self
            .record
            .lock()
            .map_err(|_| anyhow::anyhow!("lease record lock was poisoned"))?;
        let current = read_record(&self.path)
            .with_context(|| format!("read lease {}", self.path.display()))?;
        ensure_owner(&current, &record)?;
        let now = epoch_seconds();
        record.heartbeat_at = now;
        record.expires_at = now.saturating_add(LEASE_TTL.as_secs());
        write_record(&self.path, &record)?;
        *last = Instant::now();
        Ok(())
    }

    pub(crate) fn release(&self) -> Result<()> {
        let record = self
            .record
            .lock()
            .map_err(|_| anyhow::anyhow!("lease record lock was poisoned"))?;
        let contents = match fs::read(&self.path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error).with_context(|| "read lease before release"),
        };
        let current: LeaseRecord =
            serde_json::from_slice(&contents).context("parse lease before release")?;
        ensure_owner(&current, &record)?;
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => {
                Err(error).with_context(|| format!("release lease {}", self.path.display()))
            }
        }
    }
}

impl Drop for ResourceLease {
    fn drop(&mut self) {
        let _ = self.release();
    }
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
    let directory = lease_directory(project)?;
    let mut report = CleanupReport {
        schema_version: LEASE_SCHEMA_VERSION,
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
        if validate_record(&record, &record.resource_id).is_err() {
            // Unknown or malformed markers are intentionally never reclaimed.
            continue;
        }
        let stale = is_stale(&record, epoch_seconds());
        let lease_file = path.display().to_string();
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
            if let Err(error) = reclaim_resource(project, &path, &record) {
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
        super::runtime::stop_owned_container(runtime, &project.root, name, Duration::from_secs(5))
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

fn validate_record(record: &LeaseRecord, resource_id: &str) -> Result<()> {
    if record.owner_marker != OWNER_MARKER {
        bail!("lease owner marker is not Harness-Gate");
    }
    if record.schema_version != LEASE_SCHEMA_VERSION {
        bail!("unsupported lease schema version {}", record.schema_version);
    }
    if record.resource_id != resource_id {
        bail!("lease resource identity mismatch");
    }
    Ok(())
}

fn ensure_owner(current: &LeaseRecord, expected: &LeaseRecord) -> Result<()> {
    validate_record(current, &expected.resource_id)?;
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

fn resource_key(resource_id: &str) -> String {
    let mut hasher = DefaultHasher::new();
    resource_id.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn lease_directory(project: &Project) -> Result<PathBuf> {
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
    process_start_identity_checked(pid).unwrap_or_else(|| format!("pid:{pid}"))
}

#[cfg(target_os = "linux")]
fn process_start_identity_checked(pid: u32) -> Option<String> {
    let contents = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, rest) = contents.rsplit_once(") ")?;
    let start_time = rest.split_whitespace().nth(19)?;
    Some(format!("linux:{start_time}"))
}

#[cfg(not(target_os = "linux"))]
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
    use super::{
        is_stale, process_start_identity_checked, resource_key, LeaseRecord, OWNER_MARKER,
    };

    #[test]
    fn resource_keys_are_stable_and_path_safe() {
        let key = resource_key("service:database");
        assert_eq!(key.len(), 16);
        assert!(key.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(key, resource_key("service:database"));
    }

    #[test]
    fn current_process_identity_is_available_on_linux() {
        #[cfg(target_os = "linux")]
        assert!(process_start_identity_checked(std::process::id()).is_some());
    }

    #[test]
    fn dead_process_is_stale_even_before_expiry() {
        let record = LeaseRecord {
            owner_marker: OWNER_MARKER.into(),
            schema_version: 1,
            resource_id: "fixture".into(),
            resource_kind: "workspace".into(),
            invocation_id: "invocation".into(),
            pid: 0,
            process_start_identity: "unknown".into(),
            created_at: 1,
            heartbeat_at: 1,
            expires_at: u64::MAX,
            resource_name: None,
            runtime: None,
        };
        assert!(is_stale(&record, 2));
    }
}
