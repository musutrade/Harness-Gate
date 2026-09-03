//! Signed, out-of-process adapter host.
//!
//! The host deliberately owns only the process boundary. Scheduler result
//! mapping and report publication stay in the existing verification modules.

use super::command::{isolate_process_tree, terminate};
use super::reader::{
    collect_limited_reader, spawn_limited_reader, DEFAULT_CAPTURE_BYTES, DEFAULT_READER_DEADLINE,
};
use super::signal::cancelled;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Protocol v2 signs the complete request, not only the executable
/// declaration. Protocol v1 requests are intentionally rejected because they
/// leave invocation arguments and capabilities mutable after signing.
pub const PROTOCOL_VERSION: u32 = 2;
pub const RESULT_SCHEMA_VERSION: &str = "1";
pub const FAILURE_CODE: &str = "ADAPTER_PROTOCOL_FAILURE";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3600);
const DEFAULT_REQUEST_AGE: Duration = Duration::from_secs(3600);
const DEFAULT_CLOCK_SKEW: Duration = Duration::from_secs(30);
const MAX_REQUEST_BYTES: u64 = 8 * 1024 * 1024;
const SIGNING_DOMAIN: &str = "harness-gate/adapter-request/v2";

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("{FAILURE_CODE}: {0}")]
    Protocol(String),
    #[error("read adapter request: {0}")]
    Request(#[source] anyhow::Error),
    #[error(
        "{FAILURE_CODE}: {stream} output exceeded {limit} bytes (captured {captured} bytes; truncated=true)"
    )]
    OutputLimit {
        stream: &'static str,
        limit: usize,
        captured: usize,
    },
    #[error("{FAILURE_CODE}: {stream} reader deadline exceeded after {deadline_ms} ms")]
    ReaderDeadline {
        stream: &'static str,
        deadline_ms: u128,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterRequest {
    pub protocol_version: u32,
    pub result_schema_version: String,
    pub adapter: AdapterDeclaration,
    pub invocation_id: String,
    pub step_id: String,
    pub timeout_ms: u64,
    pub config_digest: String,
    pub artifact_root: PathBuf,
    /// A signer-controlled one-time value. Hosts reject a nonce more than
    /// once under the same replay guard.
    pub nonce: String,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub args: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub capabilities: AdapterCapabilities,
    pub input: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterDeclaration {
    pub name: String,
    pub version: String,
    pub executable: PathBuf,
    pub source_digest: String,
    pub signature: AdapterSignature,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterSignature {
    pub algorithm: String,
    pub key_id: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterCapabilities {
    pub network: BTreeSet<String>,
    pub resources: BTreeSet<String>,
    pub environment: BTreeSet<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CapabilityPolicy {
    pub network: BTreeSet<String>,
    pub resources: BTreeSet<String>,
    pub environment: BTreeSet<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedKey {
    pub key_id: String,
    pub public_key: String,
}

#[derive(Debug, Clone)]
pub struct HostPolicy {
    pub trusted_keys: Vec<TrustedKey>,
    pub capabilities: CapabilityPolicy,
    pub max_timeout: Option<Duration>,
    pub max_request_age: Option<Duration>,
    pub clock_skew: Duration,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub max_output_bytes: usize,
    pub reader_deadline: Duration,
    pub max_artifact_bytes: Option<u64>,
    pub replay_guard: ReplayGuard,
    /// Optional durable nonce ledger directory. The CLI supplies a sidecar
    /// directory next to the request; embedders may point this at their own
    /// control-plane storage.
    pub replay_state_dir: Option<PathBuf>,
}

/// In-memory replay state, optionally paired with an atomic durable sidecar.
/// Long-lived orchestrators can point the sidecar at their control-plane
/// storage; the CLI supplies a request-adjacent sidecar by default.
#[derive(Debug, Clone, Default)]
pub struct ReplayGuard {
    seen: Arc<Mutex<BTreeMap<String, u64>>>,
}

impl ReplayGuard {
    fn claim(
        &self,
        nonce: &str,
        issued_at_ms: u64,
        expires_at_ms: u64,
        now_ms: u64,
        state_dir: Option<&Path>,
    ) -> Result<(), AdapterError> {
        let mut seen = self.seen.lock().map_err(|_| {
            AdapterError::Protocol("replay guard state is poisoned; refusing request".into())
        })?;
        seen.retain(|_, expiry| *expiry >= now_ms);
        if seen.contains_key(nonce) {
            return Err(AdapterError::Protocol(
                "adapter request nonce has already been used".into(),
            ));
        }
        seen.insert(nonce.to_string(), expires_at_ms);
        if let Some(state_dir) = state_dir {
            claim_durable_nonce(state_dir, nonce, issued_at_ms, expires_at_ms, now_ms)?;
        }
        Ok(())
    }
}

impl Default for HostPolicy {
    fn default() -> Self {
        Self {
            trusted_keys: Vec::new(),
            capabilities: CapabilityPolicy::default(),
            max_timeout: None,
            max_request_age: Some(DEFAULT_REQUEST_AGE),
            clock_skew: DEFAULT_CLOCK_SKEW,
            max_stdout_bytes: DEFAULT_CAPTURE_BYTES,
            max_stderr_bytes: DEFAULT_CAPTURE_BYTES,
            max_output_bytes: DEFAULT_CAPTURE_BYTES.saturating_mul(2),
            reader_deadline: DEFAULT_READER_DEADLINE,
            max_artifact_bytes: Some(DEFAULT_CAPTURE_BYTES as u64 * 4),
            replay_guard: ReplayGuard::default(),
            replay_state_dir: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AdapterOutcome {
    pub response: Value,
    pub status_code: Option<i32>,
    pub timed_out: bool,
    pub cancelled: bool,
    pub duration_ms: u128,
    pub stderr_bytes: usize,
}

/// Execute one signed adapter request in an independent process group with
/// bounded host-side cleanup.
pub fn run(request: AdapterRequest, policy: &HostPolicy) -> Result<AdapterOutcome, AdapterError> {
    run_with_cancel(request, policy, cancelled)
}

/// Testable variant whose cancellation source is supplied by the caller.
pub fn run_with_cancel<F>(
    request: AdapterRequest,
    policy: &HostPolicy,
    is_cancelled: F,
) -> Result<AdapterOutcome, AdapterError>
where
    F: Fn() -> bool,
{
    validate_request(&request, policy)?;
    let artifact_root = canonical_directory(&request.artifact_root, "artifact root")?;
    let executable = canonical_file(&request.adapter.executable, "adapter executable")?;
    let request_json = serde_json::to_vec(&request)
        .map_err(|error| AdapterError::Protocol(format!("serialize request: {error}")))?;
    if request_json.len() as u64 > MAX_REQUEST_BYTES {
        return Err(AdapterError::Protocol(format!(
            "adapter request exceeds {} bytes",
            MAX_REQUEST_BYTES
        )));
    }

    let started = Instant::now();
    let mut command = Command::new(&executable);
    command
        .args(&request.args)
        .current_dir(&artifact_root)
        .env_clear()
        .env("HARNESS_GATE_INVOCATION_ID", &request.invocation_id)
        .env("HARNESS_GATE_STEP_ID", &request.step_id)
        .env("HARNESS_GATE_ARTIFACT_ROOT", &artifact_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in &request.environment {
        command.env(key, value);
    }
    isolate_process_tree(&mut command);
    let mut child = command.spawn().map_err(|error| {
        AdapterError::Protocol(format!("start adapter {}: {error}", executable.display()))
    })?;
    let stdin = match child.stdin.take() {
        Some(stdin) => stdin,
        None => {
            let _ = terminate(&mut child);
            return Err(AdapterError::Protocol("adapter stdin was not piped".into()));
        }
    };
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let _ = terminate(&mut child);
            return Err(AdapterError::Protocol(
                "adapter stdout was not piped".into(),
            ));
        }
    };
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            let _ = terminate(&mut child);
            return Err(AdapterError::Protocol(
                "adapter stderr was not piped".into(),
            ));
        }
    };
    let stdout_overflow = Arc::new(AtomicBool::new(false));
    let stderr_overflow = Arc::new(AtomicBool::new(false));
    let (stdout_handle, stdout_receiver) = spawn_limited_reader(
        stdout,
        policy.max_stdout_bytes,
        Arc::clone(&stdout_overflow),
    );
    let (stderr_handle, stderr_receiver) = spawn_limited_reader(
        stderr,
        policy.max_stderr_bytes,
        Arc::clone(&stderr_overflow),
    );
    let (stdin_handle, stdin_receiver) = spawn_request_writer(stdin, request_json);
    let timeout = Duration::from_millis(request.timeout_ms);
    let mut timed_out = false;
    let mut was_cancelled = false;
    let mut output_limited = None;
    let mut request_write_error: Option<AdapterError> = None;
    let mut request_write_done = false;
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| AdapterError::Protocol(format!("wait for adapter: {error}")))?
        {
            break status;
        }
        if !request_write_done {
            match stdin_receiver.try_recv() {
                Ok(Ok(())) => request_write_done = true,
                Ok(Err(error)) => {
                    request_write_error = Some(AdapterError::Protocol(format!(
                        "write adapter request: {error}"
                    )));
                    break terminate(&mut child).map_err(|error| {
                        AdapterError::Protocol(format!(
                            "terminate adapter after stdin error: {error}"
                        ))
                    })?;
                }
                Err(TryRecvError::Disconnected) => {
                    request_write_error = Some(AdapterError::Protocol(
                        "adapter stdin writer disconnected".into(),
                    ));
                    break terminate(&mut child).map_err(|error| {
                        AdapterError::Protocol(format!(
                            "terminate adapter after stdin error: {error}"
                        ))
                    })?;
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        if stdout_overflow.load(Ordering::Acquire) {
            output_limited = Some(("stdout", policy.max_stdout_bytes));
            terminate(&mut child).map_err(|error| {
                AdapterError::Protocol(format!("terminate noisy adapter: {error}"))
            })?;
            break child
                .try_wait()
                .map_err(|error| AdapterError::Protocol(format!("reap noisy adapter: {error}")))?
                .ok_or_else(|| AdapterError::Protocol("noisy adapter did not exit".into()))?;
        }
        if stderr_overflow.load(Ordering::Acquire) {
            output_limited = Some(("stderr", policy.max_stderr_bytes));
            terminate(&mut child).map_err(|error| {
                AdapterError::Protocol(format!("terminate noisy adapter: {error}"))
            })?;
            break child
                .try_wait()
                .map_err(|error| AdapterError::Protocol(format!("reap noisy adapter: {error}")))?
                .ok_or_else(|| AdapterError::Protocol("noisy adapter did not exit".into()))?;
        }
        if is_cancelled() {
            was_cancelled = true;
            terminate(&mut child).map_err(|error| {
                AdapterError::Protocol(format!("terminate cancelled adapter: {error}"))
            })?;
            break child
                .try_wait()
                .map_err(|error| {
                    AdapterError::Protocol(format!("reap cancelled adapter: {error}"))
                })?
                .ok_or_else(|| AdapterError::Protocol("cancelled adapter did not exit".into()))?;
        }
        if started.elapsed() >= timeout {
            timed_out = true;
            terminate(&mut child).map_err(|error| {
                AdapterError::Protocol(format!("terminate timed out adapter: {error}"))
            })?;
            break child
                .try_wait()
                .map_err(|error| {
                    AdapterError::Protocol(format!("reap timed out adapter: {error}"))
                })?
                .ok_or_else(|| AdapterError::Protocol("timed out adapter did not exit".into()))?;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let reader_started = Instant::now();
    let stdout = collect_limited_reader(
        stdout_handle,
        stdout_receiver,
        policy.reader_deadline,
        "adapter stdout",
    )
    .map_err(|error| map_reader_error("stdout", policy.reader_deadline, error))?;
    let remaining_reader_deadline = policy
        .reader_deadline
        .saturating_sub(reader_started.elapsed());
    let stderr = collect_limited_reader(
        stderr_handle,
        stderr_receiver,
        remaining_reader_deadline,
        "adapter stderr",
    )
    .map_err(|error| map_reader_error("stderr", remaining_reader_deadline, error))?;
    let writer_deadline = policy
        .reader_deadline
        .saturating_sub(reader_started.elapsed());
    if request_write_done || request_write_error.is_some() {
        if stdin_handle.join().is_err() {
            request_write_error = Some(AdapterError::Protocol(
                "adapter stdin writer thread panicked".into(),
            ));
        }
    } else {
        request_write_error =
            collect_request_writer(stdin_handle, stdin_receiver, writer_deadline).err();
    }
    if let Some((stream, limit)) = output_limited {
        let captured = if stream == "stdout" {
            stdout.bytes.len()
        } else {
            stderr.bytes.len()
        };
        return Err(AdapterError::OutputLimit {
            stream,
            limit,
            captured,
        });
    }
    let combined_bytes = stdout.bytes.len().saturating_add(stderr.bytes.len());
    if stdout.truncated || stderr.truncated || combined_bytes > policy.max_output_bytes {
        let (stream, limit, captured) = if stdout.truncated {
            ("stdout", policy.max_stdout_bytes, stdout.bytes.len())
        } else if stderr.truncated {
            ("stderr", policy.max_stderr_bytes, stderr.bytes.len())
        } else {
            ("combined", policy.max_output_bytes, combined_bytes)
        };
        return Err(AdapterError::OutputLimit {
            stream,
            limit,
            captured,
        });
    }
    let duration_ms = started.elapsed().as_millis();
    if timed_out {
        return Err(AdapterError::Protocol(format!(
            "adapter timed out after {}ms",
            request.timeout_ms
        )));
    }
    if was_cancelled {
        return Err(AdapterError::Protocol("adapter cancelled".into()));
    }
    if let Some(error) = request_write_error {
        return Err(error);
    }
    if !status.success() {
        return Err(AdapterError::Protocol(format!(
            "adapter exited with {}",
            status
                .code()
                .map_or_else(|| "signal".to_string(), |code| code.to_string())
        )));
    }
    let response = parse_response(&stdout.bytes)?;
    validate_artifact_root_budget(&artifact_root, policy.max_artifact_bytes)?;
    validate_artifacts_with_budget(
        &response,
        &artifact_root,
        &request,
        policy.max_artifact_bytes,
    )?;
    Ok(AdapterOutcome {
        response,
        status_code: status.code(),
        timed_out,
        cancelled: was_cancelled,
        duration_ms,
        stderr_bytes: stderr.bytes.len(),
    })
}

fn spawn_request_writer(
    mut stdin: std::process::ChildStdin,
    request: Vec<u8>,
) -> (JoinHandle<()>, Receiver<io::Result<()>>) {
    let (sender, receiver) = mpsc::sync_channel(1);
    let handle = std::thread::spawn(move || {
        let result = stdin
            .write_all(&request)
            .and_then(|_| stdin.write_all(b"\n"));
        let _ = sender.send(result);
    });
    (handle, receiver)
}

fn collect_request_writer(
    handle: JoinHandle<()>,
    receiver: Receiver<io::Result<()>>,
    deadline: Duration,
) -> Result<(), AdapterError> {
    match receiver.recv_timeout(deadline) {
        Ok(result) => {
            if handle.join().is_err() {
                return Err(AdapterError::Protocol(
                    "adapter stdin writer thread panicked".into(),
                ));
            }
            result
                .map_err(|error| AdapterError::Protocol(format!("write adapter request: {error}")))
        }
        Err(RecvTimeoutError::Timeout) => {
            drop(handle);
            Err(AdapterError::ReaderDeadline {
                stream: "adapter stdin",
                deadline_ms: deadline.as_millis(),
            })
        }
        Err(RecvTimeoutError::Disconnected) => {
            let _ = handle.join();
            Err(AdapterError::Protocol(
                "adapter stdin writer disconnected".into(),
            ))
        }
    }
}

pub fn read_request(path: &Path) -> Result<AdapterRequest, AdapterError> {
    let metadata =
        fs::metadata(path).map_err(|error| AdapterError::Request(anyhow::Error::new(error)))?;
    if metadata.len() > MAX_REQUEST_BYTES {
        return Err(AdapterError::Protocol(format!(
            "adapter request exceeds {} bytes",
            MAX_REQUEST_BYTES
        )));
    }
    // The metadata check is only an early rejection. Read through a bounded
    // handle as well so a concurrent file growth cannot turn request parsing
    // into an unbounded allocation.
    let file =
        File::open(path).map_err(|error| AdapterError::Request(anyhow::Error::new(error)))?;
    let mut bytes = Vec::with_capacity(metadata.len().min(MAX_REQUEST_BYTES) as usize);
    file.take(MAX_REQUEST_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| AdapterError::Request(anyhow::Error::new(error)))?;
    if bytes.len() as u64 > MAX_REQUEST_BYTES {
        return Err(AdapterError::Protocol(format!(
            "adapter request exceeds {} bytes",
            MAX_REQUEST_BYTES
        )));
    }
    serde_json::from_slice(&bytes).map_err(|error| AdapterError::Request(anyhow::Error::new(error)))
}

fn validate_request(request: &AdapterRequest, policy: &HostPolicy) -> Result<(), AdapterError> {
    validate_request_at(request, policy, unix_time_millis()?)
}

fn validate_request_at(
    request: &AdapterRequest,
    policy: &HostPolicy,
    now_ms: u64,
) -> Result<(), AdapterError> {
    if request.protocol_version != PROTOCOL_VERSION {
        return Err(AdapterError::Protocol(format!(
            "unsupported protocol version {}",
            request.protocol_version
        )));
    }
    if request.result_schema_version != RESULT_SCHEMA_VERSION {
        return Err(AdapterError::Protocol(format!(
            "unsupported result schema version {}",
            request.result_schema_version
        )));
    }
    // Verify the signed envelope before applying policy to any mutable request
    // field. This makes a supported-version field substitution an explicit
    // signature failure rather than a policy oracle.
    verify_signature(request, policy)?;
    if request.invocation_id.trim().is_empty() || request.step_id.trim().is_empty() {
        return Err(AdapterError::Protocol(
            "invocation_id and step_id are required".into(),
        ));
    }
    if request.nonce.trim().is_empty()
        || request.nonce.len() > 256
        || !request
            .nonce
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
    {
        return Err(AdapterError::Protocol(
            "adapter request nonce is required, path-safe, and must be at most 256 bytes".into(),
        ));
    }
    if request.expires_at_ms <= request.issued_at_ms {
        return Err(AdapterError::Protocol(
            "adapter request validity window is invalid".into(),
        ));
    }
    let skew_ms = policy.clock_skew.as_millis().min(u64::MAX as u128) as u64;
    if request.issued_at_ms > now_ms.saturating_add(skew_ms) {
        return Err(AdapterError::Protocol(
            "adapter request is issued in the future".into(),
        ));
    }
    if request.expires_at_ms < now_ms.saturating_sub(skew_ms) {
        return Err(AdapterError::Protocol("adapter request has expired".into()));
    }
    if let Some(max_age) = policy.max_request_age {
        let max_age_ms = max_age.as_millis().min(u64::MAX as u128) as u64;
        if request.expires_at_ms.saturating_sub(request.issued_at_ms) > max_age_ms {
            return Err(AdapterError::Protocol(
                "adapter request validity window exceeds host policy".into(),
            ));
        }
    }
    let timeout = Duration::from_millis(request.timeout_ms);
    if timeout.is_zero() || timeout > policy.max_timeout.unwrap_or(DEFAULT_TIMEOUT) {
        return Err(AdapterError::Protocol(
            "adapter timeout is outside host policy".into(),
        ));
    }
    validate_capabilities(&request.capabilities, policy)?;
    for key in request.environment.keys() {
        if !request.capabilities.environment.contains(key) {
            return Err(AdapterError::Protocol(format!(
                "environment key {key:?} was not declared"
            )));
        }
    }
    for (key, value) in &request.environment {
        validate_environment_entry(key, value)?;
    }
    for argument in &request.args {
        if argument.contains('\0') {
            return Err(AdapterError::Protocol(
                "adapter argument contains NUL".into(),
            ));
        }
    }
    let declaration = &request.adapter;
    if declaration.name.trim().is_empty() || declaration.version.trim().is_empty() {
        return Err(AdapterError::Protocol(
            "adapter name and version are required".into(),
        ));
    }
    let executable = canonical_file(&declaration.executable, "adapter executable")?;
    let digest = sha256_file(&executable).map_err(|error| {
        AdapterError::Protocol(format!(
            "hash adapter executable {}: {error}",
            executable.display()
        ))
    })?;
    if !constant_time_eq(&digest, &declaration.source_digest) {
        return Err(AdapterError::Protocol(
            "adapter executable digest mismatch".into(),
        ));
    }
    policy.replay_guard.claim(
        &request.nonce,
        request.issued_at_ms,
        request.expires_at_ms,
        now_ms,
        policy.replay_state_dir.as_deref(),
    )?;
    Ok(())
}

fn validate_capabilities(
    requested: &AdapterCapabilities,
    policy: &HostPolicy,
) -> Result<(), AdapterError> {
    for capability in &requested.network {
        if !policy.capabilities.network.contains(capability) {
            return Err(AdapterError::Protocol(format!(
                "network capability {capability:?} is not allowed"
            )));
        }
    }
    for capability in &requested.resources {
        if !policy.capabilities.resources.contains(capability) {
            return Err(AdapterError::Protocol(format!(
                "resource capability {capability:?} is not allowed"
            )));
        }
    }
    for capability in &requested.environment {
        if !policy.capabilities.environment.contains(capability) {
            return Err(AdapterError::Protocol(format!(
                "environment capability {capability:?} is not allowed"
            )));
        }
    }
    Ok(())
}

fn verify_signature(request: &AdapterRequest, policy: &HostPolicy) -> Result<(), AdapterError> {
    let declaration = &request.adapter;
    if declaration.signature.algorithm != "ed25519" {
        return Err(AdapterError::Protocol(
            "unsupported signature algorithm".into(),
        ));
    }
    let trusted = policy
        .trusted_keys
        .iter()
        .find(|key| key.key_id == declaration.signature.key_id)
        .ok_or_else(|| AdapterError::Protocol("adapter signer is not trusted".into()))?;
    let public_key = BASE64
        .decode(&trusted.public_key)
        .map_err(|error| AdapterError::Protocol(format!("decode adapter public key: {error}")))?;
    let public_key: [u8; 32] = public_key
        .try_into()
        .map_err(|_| AdapterError::Protocol("adapter public key must be 32 bytes".into()))?;
    let key = VerifyingKey::from_bytes(&public_key)
        .map_err(|error| AdapterError::Protocol(format!("parse adapter public key: {error}")))?;
    let signature = BASE64
        .decode(&declaration.signature.value)
        .map_err(|error| AdapterError::Protocol(format!("decode adapter signature: {error}")))?;
    let signature = Signature::from_slice(&signature)
        .map_err(|error| AdapterError::Protocol(format!("parse adapter signature: {error}")))?;
    let payload = signing_payload(request)?;
    key.verify(&payload, &signature)
        .map_err(|_| AdapterError::Protocol("adapter signature verification failed".into()))
}

#[derive(Serialize)]
struct UnsignedAdapterSignature<'a> {
    algorithm: &'a str,
    key_id: &'a str,
}

#[derive(Serialize)]
struct UnsignedDeclaration<'a> {
    name: &'a str,
    version: &'a str,
    executable: &'a Path,
    source_digest: &'a str,
    signature: UnsignedAdapterSignature<'a>,
}

#[derive(Serialize)]
struct UnsignedRequest<'a> {
    domain: &'static str,
    protocol_version: u32,
    result_schema_version: &'a str,
    adapter: UnsignedDeclaration<'a>,
    invocation_id: &'a str,
    step_id: &'a str,
    timeout_ms: u64,
    config_digest: &'a str,
    artifact_root: &'a Path,
    nonce: &'a str,
    issued_at_ms: u64,
    expires_at_ms: u64,
    args: &'a [String],
    environment: &'a BTreeMap<String, String>,
    capabilities: &'a AdapterCapabilities,
    input: &'a Value,
}

fn signing_payload(request: &AdapterRequest) -> Result<Vec<u8>, AdapterError> {
    serde_json::to_vec(&UnsignedRequest {
        domain: SIGNING_DOMAIN,
        protocol_version: request.protocol_version,
        result_schema_version: &request.result_schema_version,
        adapter: UnsignedDeclaration {
            name: &request.adapter.name,
            version: &request.adapter.version,
            executable: &request.adapter.executable,
            source_digest: &request.adapter.source_digest,
            signature: UnsignedAdapterSignature {
                algorithm: &request.adapter.signature.algorithm,
                key_id: &request.adapter.signature.key_id,
            },
        },
        invocation_id: &request.invocation_id,
        step_id: &request.step_id,
        timeout_ms: request.timeout_ms,
        config_digest: &request.config_digest,
        artifact_root: &request.artifact_root,
        nonce: &request.nonce,
        issued_at_ms: request.issued_at_ms,
        expires_at_ms: request.expires_at_ms,
        args: &request.args,
        environment: &request.environment,
        capabilities: &request.capabilities,
        input: &request.input,
    })
    .map_err(|error| AdapterError::Protocol(format!("serialize signed request: {error}")))
}

fn validate_environment_entry(key: &str, value: &str) -> Result<(), AdapterError> {
    if key.is_empty()
        || key.contains('\0')
        || key.contains('=')
        || key.chars().any(char::is_control)
    {
        return Err(AdapterError::Protocol(format!(
            "invalid adapter environment key {key:?}"
        )));
    }
    if key
        .get(.."HARNESS_GATE_".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("HARNESS_GATE_"))
    {
        return Err(AdapterError::Protocol(format!(
            "adapter environment key {key:?} is reserved"
        )));
    }
    if value.contains('\0') {
        return Err(AdapterError::Protocol(format!(
            "adapter environment value for {key:?} contains NUL"
        )));
    }
    Ok(())
}

fn invalid_relative_artifact_path(path: &str) -> bool {
    path.is_empty()
        || path.contains('\0')
        // Reject both native absolute paths and Windows drive/UNC forms even
        // when the host is running on Unix. Requests are portable protocol
        // data, not platform-specific path syntax.
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.as_bytes().get(1).is_some_and(|byte| *byte == b':')
        || path
            .split(['/', '\\'])
            .any(|component| component == "..")
}

fn unix_time_millis() -> Result<u64, AdapterError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
        .map_err(|error| {
            AdapterError::Protocol(format!("system clock is before Unix epoch: {error}"))
        })
}

fn claim_durable_nonce(
    state_dir: &Path,
    nonce: &str,
    issued_at_ms: u64,
    expires_at_ms: u64,
    now_ms: u64,
) -> Result<(), AdapterError> {
    if let Ok(metadata) = fs::symlink_metadata(state_dir) {
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(AdapterError::Protocol(
                "adapter replay state path is not a regular directory".into(),
            ));
        }
    } else {
        fs::create_dir_all(state_dir).map_err(|error| {
            AdapterError::Protocol(format!("create adapter replay state directory: {error}"))
        })?;
    }
    let digest = Sha256::digest(nonce.as_bytes());
    let marker = state_dir.join(format!("nonce-{digest:x}.json"));
    let mut file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&marker)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(AdapterError::Protocol(
                "adapter request nonce has already been used".into(),
            ));
        }
        Err(error) => {
            return Err(AdapterError::Protocol(format!(
                "claim adapter replay nonce: {error}"
            )))
        }
    };
    let record = serde_json::json!({
        "nonce": nonce,
        "issued_at_ms": issued_at_ms,
        "claimed_at_ms": now_ms,
        "expires_at_ms": expires_at_ms,
    });
    if let Err(error) = file
        .write_all(record.to_string().as_bytes())
        .and_then(|_| file.sync_all())
    {
        let _ = fs::remove_file(&marker);
        return Err(AdapterError::Protocol(format!(
            "persist adapter replay nonce: {error}"
        )));
    }
    Ok(())
}

fn map_reader_error(
    stream: &'static str,
    deadline: Duration,
    error: std::io::Error,
) -> AdapterError {
    if error.kind() == std::io::ErrorKind::TimedOut {
        AdapterError::ReaderDeadline {
            stream,
            deadline_ms: deadline.as_millis(),
        }
    } else {
        AdapterError::Protocol(format!("read adapter {stream}: {error}"))
    }
}

fn parse_response(stdout: &[u8]) -> Result<Value, AdapterError> {
    let text = std::str::from_utf8(stdout)
        .map_err(|error| AdapterError::Protocol(format!("adapter stdout is not UTF-8: {error}")))?;
    let mut stream = serde_json::Deserializer::from_str(text).into_iter::<Value>();
    let response = stream
        .next()
        .transpose()
        .map_err(|error| AdapterError::Protocol(format!("malformed adapter response: {error}")))?
        .ok_or_else(|| AdapterError::Protocol("adapter returned no JSON response".into()))?;
    if stream.next().is_some() {
        return Err(AdapterError::Protocol(
            "adapter returned more than one JSON response".into(),
        ));
    }
    let object = response
        .as_object()
        .ok_or_else(|| AdapterError::Protocol("adapter response must be a JSON object".into()))?;
    let schema = object
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| AdapterError::Protocol("adapter response lacks schema_version".into()))?;
    if schema != RESULT_SCHEMA_VERSION {
        return Err(AdapterError::Protocol(format!(
            "unsupported adapter response schema {schema:?}"
        )));
    }
    let status = object
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| AdapterError::Protocol("adapter response lacks status".into()))?;
    if !matches!(status, "PASS" | "FAIL" | "CANCELLED" | "SKIPPED" | "WAIVED") {
        return Err(AdapterError::Protocol(format!(
            "unsupported adapter status {status:?}"
        )));
    }
    Ok(response)
}

fn validate_artifacts_with_budget(
    response: &Value,
    artifact_root: &Path,
    request: &AdapterRequest,
    max_artifact_bytes: Option<u64>,
) -> Result<(), AdapterError> {
    if let Some(invocation_id) = response.get("invocation_id").and_then(Value::as_str) {
        if invocation_id != request.invocation_id {
            return Err(AdapterError::Protocol(
                "adapter invocation_id mismatch".into(),
            ));
        }
    }
    let Some(artifacts) = response.get("artifacts") else {
        return Ok(());
    };
    let artifacts = artifacts
        .as_array()
        .ok_or_else(|| AdapterError::Protocol("adapter artifacts must be an array".into()))?;
    let mut total_bytes = 0_u64;
    let mut seen_targets = BTreeSet::new();
    for artifact in artifacts {
        let object = artifact
            .as_object()
            .ok_or_else(|| AdapterError::Protocol("adapter artifact must be an object".into()))?;
        let path = object
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| AdapterError::Protocol("adapter artifact lacks path".into()))?;
        let relative = Path::new(path);
        if invalid_relative_artifact_path(path)
            || relative.is_absolute()
            || relative.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(AdapterError::Protocol(format!(
                "adapter artifact escapes invocation root: {path}"
            )));
        }
        let target = artifact_root.join(relative);
        let canonical = fs::canonicalize(&target).map_err(|error| {
            AdapterError::Protocol(format!("resolve adapter artifact {path:?}: {error}"))
        })?;
        if !canonical.starts_with(artifact_root) {
            return Err(AdapterError::Protocol(format!(
                "adapter artifact escapes invocation root: {path}"
            )));
        }
        let metadata = fs::metadata(&canonical).map_err(|error| {
            AdapterError::Protocol(format!("inspect adapter artifact {path:?}: {error}"))
        })?;
        if !metadata.is_file() {
            return Err(AdapterError::Protocol(format!(
                "adapter artifact is not a regular file: {path}"
            )));
        }
        if seen_targets.insert(canonical) {
            total_bytes = total_bytes.checked_add(metadata.len()).ok_or_else(|| {
                AdapterError::Protocol("adapter artifact byte budget overflow".into())
            })?;
        }
        if let Some(limit) = max_artifact_bytes {
            if total_bytes > limit {
                return Err(AdapterError::Protocol(format!(
                    "adapter artifacts exceed {} bytes (observed {}; truncated=false)",
                    limit, total_bytes
                )));
            }
        }
    }
    Ok(())
}

fn validate_artifact_root_budget(
    root: &Path,
    max_artifact_bytes: Option<u64>,
) -> Result<(), AdapterError> {
    let Some(limit) = max_artifact_bytes else {
        return Ok(());
    };
    let mut total = 0_u64;
    accumulate_artifact_bytes(root, limit, &mut total)?;
    Ok(())
}

fn accumulate_artifact_bytes(path: &Path, limit: u64, total: &mut u64) -> Result<(), AdapterError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        AdapterError::Protocol(format!(
            "inspect adapter artifact root {}: {error}",
            path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(AdapterError::Protocol(format!(
            "adapter artifact root contains a symbolic link: {}",
            path.display()
        )));
    }
    if metadata.is_file() {
        *total = total.checked_add(metadata.len()).ok_or_else(|| {
            AdapterError::Protocol("adapter artifact byte budget overflow".into())
        })?;
        if *total > limit {
            return Err(AdapterError::Protocol(format!(
                "adapter artifact root exceeds {} bytes (observed {}; truncated=false)",
                limit, total
            )));
        }
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(AdapterError::Protocol(format!(
            "adapter artifact root contains a non-regular entry: {}",
            path.display()
        )));
    }
    for entry in fs::read_dir(path).map_err(|error| {
        AdapterError::Protocol(format!(
            "read adapter artifact root {}: {error}",
            path.display()
        ))
    })? {
        let entry = entry.map_err(|error| {
            AdapterError::Protocol(format!("read adapter artifact entry: {error}"))
        })?;
        accumulate_artifact_bytes(&entry.path(), limit, total)?;
    }
    Ok(())
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, AdapterError> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| AdapterError::Protocol(format!("resolve {label}: {error}")))?;
    if !canonical.is_dir() {
        return Err(AdapterError::Protocol(format!(
            "{label} is not a directory"
        )));
    }
    Ok(canonical)
}

fn canonical_file(path: &Path, label: &str) -> Result<PathBuf, AdapterError> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| AdapterError::Protocol(format!("resolve {label}: {error}")))?;
    if !canonical.is_file() {
        return Err(AdapterError::Protocol(format!("{label} is not a file")));
    }
    Ok(canonical)
}

fn sha256_file(path: &Path) -> anyhow::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    left.len() == right.len()
        && left
            .as_bytes()
            .iter()
            .zip(right.as_bytes())
            .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
            == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use std::fs;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Arc;
    use std::thread;
    use tempfile::tempdir;

    static NONCE_COUNTER: AtomicU64 = AtomicU64::new(1);

    fn fixture_request(root: &Path, mode: &str) -> (AdapterRequest, HostPolicy) {
        let interpreter = std::env::var("PYTHON").unwrap_or_else(|_| {
            if cfg!(windows) {
                "python".into()
            } else {
                "python3".into()
            }
        });
        let executable = resolve_executable(&interpreter).expect("python interpreter");
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("tools/quality/fixtures/adapter/adapter_fixture.py");
        let source_digest = sha256_file(&executable).expect("interpreter digest");
        let declaration = AdapterDeclaration {
            name: "fixture".into(),
            version: "1.0.0".into(),
            executable: executable.clone(),
            source_digest,
            signature: AdapterSignature {
                algorithm: "ed25519".into(),
                key_id: "fixture".into(),
                value: String::new(),
            },
        };
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let now_ms = unix_time_millis().expect("system clock");
        let nonce_suffix = format!(
            "{}-{}-{}",
            std::process::id(),
            now_ms,
            NONCE_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let mut request = AdapterRequest {
            protocol_version: PROTOCOL_VERSION,
            result_schema_version: RESULT_SCHEMA_VERSION.into(),
            adapter: declaration,
            invocation_id: "inv-1".into(),
            step_id: "step-1".into(),
            // Windows-hosted runners can spend more than one second starting
            // the Python interpreter before the deterministic fixture runs.
            timeout_ms: 5_000,
            config_digest: "config".into(),
            artifact_root: root.to_path_buf(),
            nonce: format!("fixture-{mode}-{nonce_suffix}"),
            issued_at_ms: now_ms.saturating_sub(1_000),
            expires_at_ms: now_ms.saturating_add(300_000),
            args: vec![fixture.to_string_lossy().to_string()],
            environment: BTreeMap::from([("ADAPTER_MODE".into(), "stable".into())]),
            capabilities: AdapterCapabilities {
                environment: BTreeSet::from(["ADAPTER_MODE".into()]),
                ..AdapterCapabilities::default()
            },
            input: serde_json::json!({"mode": mode}),
        };
        let signature = signing_key.sign(&signing_payload(&request).unwrap());
        request.adapter.signature.value = BASE64.encode(signature.to_bytes());
        let policy = HostPolicy {
            trusted_keys: vec![TrustedKey {
                key_id: "fixture".into(),
                public_key: BASE64.encode(signing_key.verifying_key().to_bytes()),
            }],
            capabilities: CapabilityPolicy {
                environment: BTreeSet::from(["ADAPTER_MODE".into()]),
                ..CapabilityPolicy::default()
            },
            ..HostPolicy::default()
        };
        (request, policy)
    }

    fn resign_request(request: &mut AdapterRequest) {
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        request.adapter.signature.value = BASE64.encode(
            signing_key
                .sign(&signing_payload(request).expect("serialize request"))
                .to_bytes(),
        );
    }

    fn resolve_executable(name: &str) -> Option<PathBuf> {
        let lookup = if cfg!(windows) { "where" } else { "which" };
        let output = std::process::Command::new(lookup).arg(name).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let output = String::from_utf8(output.stdout).ok()?;
        let path = output.lines().next()?.trim();
        fs::canonicalize(path).ok()
    }

    #[test]
    fn signed_fixture_runs_and_publishes_artifact() {
        let directory = tempdir().unwrap();
        let (request, policy) = fixture_request(directory.path(), "pass");
        let outcome = run(request, &policy).expect("fixture succeeds");
        assert_eq!(outcome.response["status"], "PASS");
        assert!(directory.path().join("adapter-result.txt").is_file());
    }

    #[test]
    fn capability_and_signature_fail_closed() {
        let directory = tempdir().unwrap();
        let (mut request, policy) = fixture_request(directory.path(), "pass");
        request.capabilities.network.insert("internet".into());
        let error = run(request, &policy).expect_err("undeclared capability must fail");
        assert!(error.to_string().contains(FAILURE_CODE));
        let (mut request, policy) = fixture_request(directory.path(), "pass");
        request.adapter.signature.value = BASE64.encode([0_u8; 64]);
        let error = run(request, &policy).expect_err("bad signature must fail");
        assert!(error.to_string().contains("signature verification failed"));
    }

    #[test]
    fn crash_timeout_cancellation_and_escape_are_isolated() {
        let directory = tempdir().unwrap();
        let (request, policy) = fixture_request(directory.path(), "crash");
        assert!(run(request, &policy).is_err());
        let (mut request, policy) = fixture_request(directory.path(), "sleep");
        request.timeout_ms = 50;
        resign_request(&mut request);
        assert!(run(request, &policy).is_err());
        let (request, policy) = fixture_request(directory.path(), "sleep");
        let cancelled = Arc::new(AtomicBool::new(false));
        let trigger = Arc::clone(&cancelled);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            trigger.store(true, Ordering::Relaxed);
        });
        assert!(run_with_cancel(request, &policy, || cancelled.load(Ordering::Relaxed)).is_err());
        let (request, policy) = fixture_request(directory.path(), "escape");
        assert!(run(request, &policy).is_err());
        let (request, policy) = fixture_request(directory.path(), "malformed");
        assert!(run(request, &policy).is_err());
    }

    #[test]
    fn blocked_stdin_writer_cannot_bypass_adapter_timeout() {
        let directory = tempdir().unwrap();
        let (mut request, mut policy) = fixture_request(directory.path(), "pass");
        request.args = vec!["-c".into(), "import time; time.sleep(30)".into()];
        // A request larger than the OS pipe buffer makes a non-reading child
        // block the writer. The host must still reach its timeout boundary.
        request.input = Value::String("x".repeat(1_024 * 1024));
        request.timeout_ms = 100;
        policy.reader_deadline = Duration::from_millis(500);
        resign_request(&mut request);

        let started = Instant::now();
        let error = run(request, &policy).expect_err("non-reading adapter must time out");
        assert!(started.elapsed() < Duration::from_secs(10));
        assert!(
            error.to_string().contains("timed out"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn request_and_response_validation_edges_fail_closed() {
        let directory = tempdir().unwrap();
        let (request, policy) = fixture_request(directory.path(), "pass");

        for invalid in [
            serde_json::to_vec(&serde_json::json!({"unknown": true})).unwrap(),
            b"not-json".to_vec(),
        ] {
            let path = directory.path().join("request.json");
            fs::write(&path, invalid).unwrap();
            assert!(read_request(&path).is_err());
        }
        assert!(read_request(&directory.path().join("missing.json")).is_err());

        let mut invalid = request.clone();
        invalid.protocol_version = 9;
        assert!(validate_request(&invalid, &policy).is_err());
        invalid = request.clone();
        invalid.result_schema_version = "2".into();
        assert!(validate_request(&invalid, &policy).is_err());
        invalid = request.clone();
        invalid.invocation_id.clear();
        resign_request(&mut invalid);
        assert!(validate_request(&invalid, &policy).is_err());
        invalid = request.clone();
        invalid.timeout_ms = 0;
        resign_request(&mut invalid);
        assert!(validate_request(&invalid, &policy).is_err());
        invalid = request.clone();
        invalid.environment.insert("UNDECLARED".into(), "x".into());
        resign_request(&mut invalid);
        assert!(validate_request(&invalid, &policy).is_err());
        invalid = request.clone();
        invalid.adapter.name.clear();
        resign_request(&mut invalid);
        assert!(validate_request(&invalid, &policy).is_err());
        invalid = request.clone();
        invalid.adapter.source_digest = "0".repeat(64);
        resign_request(&mut invalid);
        assert!(validate_request(&invalid, &policy).is_err());
        invalid = request.clone();
        invalid.adapter.signature.algorithm = "rsa".into();
        assert!(validate_request(&invalid, &policy).is_err());
        invalid = request.clone();
        invalid.adapter.signature.key_id = "unknown".into();
        assert!(validate_request(&invalid, &policy).is_err());

        invalid = request.clone();
        invalid.issued_at_ms = unix_time_millis().unwrap().saturating_add(120_000);
        invalid.expires_at_ms = invalid.issued_at_ms.saturating_add(1_000);
        resign_request(&mut invalid);
        assert!(validate_request(&invalid, &policy).is_err());
        invalid = request.clone();
        invalid.expires_at_ms = invalid.issued_at_ms.saturating_add(7_200_000);
        resign_request(&mut invalid);
        assert!(validate_request(&invalid, &policy).is_err());
        invalid = request.clone();
        invalid.capabilities.network.insert("internet".into());
        resign_request(&mut invalid);
        assert!(validate_request(&invalid, &policy).is_err());
        invalid = request.clone();
        invalid.capabilities.resources.insert("database".into());
        resign_request(&mut invalid);
        assert!(validate_request(&invalid, &policy).is_err());
        invalid = request.clone();
        invalid.args.push("contains\0nul".into());
        resign_request(&mut invalid);
        assert!(validate_request(&invalid, &policy).is_err());
        invalid = request.clone();
        invalid.nonce = "../reused".into();
        resign_request(&mut invalid);
        assert!(validate_request(&invalid, &policy).is_err());

        for response in [
            b"".as_slice(),
            b"not-json".as_slice(),
            b"{}".as_slice(),
            br#"{"schema_version":"2","status":"PASS"}"#.as_slice(),
            br#"{"schema_version":"1","status":"UNKNOWN"}"#.as_slice(),
            br#"{"schema_version":"1","status":"PASS"} {"schema_version":"1","status":"PASS"}"#
                .as_slice(),
        ] {
            assert!(parse_response(response).is_err());
        }
        assert!(parse_response(br#"{"schema_version":"1","status":"PASS"}"#).is_ok());
        let mismatch =
            serde_json::json!({"schema_version":"1", "status":"PASS", "invocation_id":"other"});
        assert!(
            validate_artifacts_with_budget(&mismatch, directory.path(), &request, None).is_err()
        );
        let missing = serde_json::json!({"schema_version":"1", "status":"PASS", "artifacts":[{"path":"missing.txt","kind":"x"}]});
        assert!(
            validate_artifacts_with_budget(&missing, directory.path(), &request, None).is_err()
        );
        for escaped_path in [
            r"..\escape.txt",
            r"C:\escape.txt",
            r"\\server\share\escape.txt",
        ] {
            let escaped = serde_json::json!({
                "schema_version": "1",
                "status": "PASS",
                "artifacts": [{"path": escaped_path, "kind": "x"}]
            });
            assert!(
                validate_artifacts_with_budget(&escaped, directory.path(), &request, None).is_err(),
                "escaped artifact path must be rejected: {escaped_path}"
            );
        }
        assert!(canonical_directory(&directory.path().join("missing"), "root").is_err());
        assert!(canonical_file(directory.path(), "file").is_err());
        assert!(!constant_time_eq("a", "b"));
    }

    #[test]
    fn full_request_signature_binds_mutable_fields() {
        let directory = tempdir().unwrap();
        let (request, _) = fixture_request(directory.path(), "pass");
        for mutate in [
            |request: &mut AdapterRequest| request.invocation_id.push_str("-other"),
            |request: &mut AdapterRequest| request.step_id.push_str("-other"),
            |request: &mut AdapterRequest| request.timeout_ms += 1,
            |request: &mut AdapterRequest| request.config_digest.push_str("-other"),
            |request: &mut AdapterRequest| request.nonce.push_str("-other"),
            |request: &mut AdapterRequest| request.issued_at_ms += 1,
            |request: &mut AdapterRequest| request.expires_at_ms += 1,
            |request: &mut AdapterRequest| request.args.push("changed".into()),
            |request: &mut AdapterRequest| {
                request.capabilities.network.insert("changed".into());
            },
            |request: &mut AdapterRequest| {
                request
                    .environment
                    .insert("ADAPTER_MODE".into(), "changed".into());
            },
            |request: &mut AdapterRequest| request.input = serde_json::json!({"mode":"crash"}),
            |request: &mut AdapterRequest| request.artifact_root = PathBuf::from("other-root"),
        ] {
            let mut tampered = request.clone();
            mutate(&mut tampered);
            let policy = fixture_request(directory.path(), "pass").1;
            let error = run(tampered, &policy).expect_err("tampered request must fail");
            assert!(
                error.to_string().contains("signature verification failed"),
                "unexpected error: {error}"
            );
        }
    }

    #[test]
    fn validity_window_and_nonce_replay_fail_closed() {
        let directory = tempdir().unwrap();
        let (mut expired, policy) = fixture_request(directory.path(), "pass");
        expired.expires_at_ms = expired.issued_at_ms;
        resign_request(&mut expired);
        let error = run(expired, &policy).expect_err("invalid window must fail");
        assert!(error.to_string().contains("validity window is invalid"));

        let (mut expired, policy) = fixture_request(directory.path(), "pass");
        expired.issued_at_ms = 1;
        expired.expires_at_ms = 2;
        resign_request(&mut expired);
        let error = run(expired, &policy).expect_err("expired request must fail");
        assert!(error.to_string().contains("expired"));

        let (request, policy) = fixture_request(directory.path(), "pass");
        let first = request.clone();
        run(first, &policy).expect("first nonce use");
        let error = run(request, &policy).expect_err("replayed nonce must fail");
        assert!(error.to_string().contains("nonce has already been used"));
    }

    #[test]
    fn durable_nonce_sidecar_rejects_replay_after_host_restart() {
        let directory = tempdir().unwrap();
        let (request, mut first_policy) = fixture_request(directory.path(), "pass");
        let replay_dir = directory.path().join("replay-state");
        first_policy.replay_state_dir = Some(replay_dir.clone());
        run(request.clone(), &first_policy).expect("first durable nonce use");

        let (_, mut second_policy) = fixture_request(directory.path(), "pass");
        second_policy.replay_state_dir = Some(replay_dir);
        let error = run(request, &second_policy).expect_err("sidecar must reject replay");
        assert!(error.to_string().contains("nonce has already been used"));
    }

    #[test]
    fn invalid_environment_entries_are_rejected_before_spawn() {
        let directory = tempdir().unwrap();
        for (key, value) in [
            ("BAD=KEY", "value"),
            ("BAD\nKEY", "value"),
            ("BAD", "nul\0value"),
            ("HARNESS_GATE_FUTURE", "value"),
            ("harness_gate_future", "value"),
        ] {
            let (mut request, mut policy) = fixture_request(directory.path(), "pass");
            request.capabilities.environment.insert(key.into());
            request.environment.insert(key.into(), value.into());
            policy.capabilities.environment.insert(key.into());
            resign_request(&mut request);
            let error = run(request, &policy).expect_err("invalid environment must fail");
            assert!(
                error.to_string().contains("invalid adapter environment")
                    || error.to_string().contains("contains NUL")
                    || error.to_string().contains("is reserved")
            );
        }
    }

    #[test]
    fn output_budget_fails_closed_with_a_truncation_marker() {
        let directory = tempdir().unwrap();
        let (request, mut policy) = fixture_request(directory.path(), "stdout-spam");
        policy.max_stdout_bytes = 128;
        let error = run(request, &policy).expect_err("oversized adapter output must fail");
        let rendered = error.to_string();
        assert!(rendered.contains("output exceeded 128 bytes"));
        assert!(rendered.contains("truncated=true"));
    }

    #[test]
    fn artifact_root_budget_counts_undeclared_files() {
        let directory = tempdir().unwrap();
        let (request, mut policy) = fixture_request(directory.path(), "artifact-spam");
        policy.max_artifact_bytes = Some(128);
        let error = run(request, &policy).expect_err("artifact budget must fail closed");
        assert!(error
            .to_string()
            .contains("artifact root exceeds 128 bytes"));
    }
}
