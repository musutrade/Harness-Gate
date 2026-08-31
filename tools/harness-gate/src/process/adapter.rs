//! Signed, out-of-process adapter host.
//!
//! The host deliberately owns only the process boundary. Scheduler result
//! mapping and report publication stay in the existing verification modules.

use super::command::{isolate_process_tree, terminate};
use super::signal::cancelled;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

pub const PROTOCOL_VERSION: u32 = 1;
pub const RESULT_SCHEMA_VERSION: &str = "1";
pub const FAILURE_CODE: &str = "ADAPTER_PROTOCOL_FAILURE";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3600);

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("{FAILURE_CODE}: {0}")]
    Protocol(String),
    #[error("read adapter request: {0}")]
    Request(#[source] anyhow::Error),
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
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    pub capabilities: AdapterCapabilities,
    #[serde(default)]
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
    #[serde(default)]
    pub network: BTreeSet<String>,
    #[serde(default)]
    pub resources: BTreeSet<String>,
    #[serde(default)]
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

#[derive(Debug, Clone, Default)]
pub struct HostPolicy {
    pub trusted_keys: Vec<TrustedKey>,
    pub capabilities: CapabilityPolicy,
    pub max_timeout: Option<Duration>,
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

/// Execute one signed adapter request with the process tree isolated from the host.
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
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(&request_json)
            .and_then(|_| stdin.write_all(b"\n"))
            .map_err(|error| AdapterError::Protocol(format!("write adapter request: {error}")))?;
    }
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AdapterError::Protocol("adapter stdout was not piped".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AdapterError::Protocol("adapter stderr was not piped".into()))?;
    let stdout_reader = thread::spawn(move || read_all(stdout));
    let stderr_reader = thread::spawn(move || read_all(stderr));
    let timeout = Duration::from_millis(request.timeout_ms);
    let mut timed_out = false;
    let mut was_cancelled = false;
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| AdapterError::Protocol(format!("wait for adapter: {error}")))?
        {
            break status;
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
        thread::sleep(Duration::from_millis(20));
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| AdapterError::Protocol("adapter stdout reader panicked".into()))?
        .map_err(|error| AdapterError::Protocol(format!("read adapter stdout: {error}")))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| AdapterError::Protocol("adapter stderr reader panicked".into()))?
        .map_err(|error| AdapterError::Protocol(format!("read adapter stderr: {error}")))?;
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
    if !status.success() {
        return Err(AdapterError::Protocol(format!(
            "adapter exited with {}",
            status
                .code()
                .map_or_else(|| "signal".to_string(), |code| code.to_string())
        )));
    }
    let response = parse_response(&stdout)?;
    validate_artifacts(&response, &artifact_root, &request)?;
    Ok(AdapterOutcome {
        response,
        status_code: status.code(),
        timed_out,
        cancelled: was_cancelled,
        duration_ms,
        stderr_bytes: stderr.len(),
    })
}

pub fn read_request(path: &Path) -> Result<AdapterRequest, AdapterError> {
    let bytes = fs::read(path).map_err(|error| AdapterError::Request(anyhow::Error::new(error)))?;
    serde_json::from_slice(&bytes).map_err(|error| AdapterError::Request(anyhow::Error::new(error)))
}

fn validate_request(request: &AdapterRequest, policy: &HostPolicy) -> Result<(), AdapterError> {
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
    if request.invocation_id.trim().is_empty() || request.step_id.trim().is_empty() {
        return Err(AdapterError::Protocol(
            "invocation_id and step_id are required".into(),
        ));
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
    verify_signature(declaration, policy)?;
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

fn verify_signature(
    declaration: &AdapterDeclaration,
    policy: &HostPolicy,
) -> Result<(), AdapterError> {
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
    let payload = signing_payload(declaration)?;
    key.verify(&payload, &signature)
        .map_err(|_| AdapterError::Protocol("adapter signature verification failed".into()))
}

#[derive(Serialize)]
struct UnsignedDeclaration<'a> {
    name: &'a str,
    version: &'a str,
    executable: &'a Path,
    source_digest: &'a str,
}

fn signing_payload(declaration: &AdapterDeclaration) -> Result<Vec<u8>, AdapterError> {
    serde_json::to_vec(&UnsignedDeclaration {
        name: &declaration.name,
        version: &declaration.version,
        executable: &declaration.executable,
        source_digest: &declaration.source_digest,
    })
    .map_err(|error| AdapterError::Protocol(format!("serialize signed declaration: {error}")))
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

fn validate_artifacts(
    response: &Value,
    artifact_root: &Path,
    request: &AdapterRequest,
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
    for artifact in artifacts {
        let object = artifact
            .as_object()
            .ok_or_else(|| AdapterError::Protocol("adapter artifact must be an object".into()))?;
        let path = object
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| AdapterError::Protocol("adapter artifact lacks path".into()))?;
        let relative = Path::new(path);
        if relative.is_absolute()
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

fn read_all(mut reader: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use std::fs;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use tempfile::tempdir;

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
        let signature = signing_key.sign(&signing_payload(&declaration).unwrap());
        let mut declaration = declaration;
        declaration.signature.value = BASE64.encode(signature.to_bytes());
        let request = AdapterRequest {
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
            args: vec![fixture.to_string_lossy().to_string()],
            environment: BTreeMap::new(),
            capabilities: AdapterCapabilities::default(),
            input: serde_json::json!({"mode": mode}),
        };
        let policy = HostPolicy {
            trusted_keys: vec![TrustedKey {
                key_id: "fixture".into(),
                public_key: BASE64.encode(signing_key.verifying_key().to_bytes()),
            }],
            ..HostPolicy::default()
        };
        (request, policy)
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
        assert!(validate_request(&invalid, &policy).is_err());
        invalid = request.clone();
        invalid.timeout_ms = 0;
        assert!(validate_request(&invalid, &policy).is_err());
        invalid = request.clone();
        invalid.environment.insert("UNDECLARED".into(), "x".into());
        assert!(validate_request(&invalid, &policy).is_err());
        invalid = request.clone();
        invalid.adapter.name.clear();
        assert!(validate_request(&invalid, &policy).is_err());
        invalid = request.clone();
        invalid.adapter.source_digest = "0".repeat(64);
        assert!(validate_request(&invalid, &policy).is_err());
        invalid = request.clone();
        invalid.adapter.signature.algorithm = "rsa".into();
        assert!(validate_request(&invalid, &policy).is_err());
        invalid = request.clone();
        invalid.adapter.signature.key_id = "unknown".into();
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
        assert!(validate_artifacts(&mismatch, directory.path(), &request).is_err());
        let missing = serde_json::json!({"schema_version":"1", "status":"PASS", "artifacts":[{"path":"missing.txt","kind":"x"}]});
        assert!(validate_artifacts(&missing, directory.path(), &request).is_err());
        assert!(canonical_directory(&directory.path().join("missing"), "root").is_err());
        assert!(canonical_file(directory.path(), "file").is_err());
        assert!(!constant_time_eq("a", "b"));
    }
}
