//! Bounded HTTPS transport for an explicitly pinned five-asset request.
//! A successful transfer is not signature verification or installation authority.
use crate::{artifact, release, strict_json};
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use ureq::{
    http::Uri,
    tls::{Certificate, RootCerts, TlsConfig},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Asset {
    url: String,
    sha256: String,
    bytes: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CertificatePin {
    path: PathBuf,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: String,
    timeout_seconds: u64,
    tls_root: Option<CertificatePin>,
    assets: BTreeMap<String, Asset>,
}

fn https_url(value: &str) -> Result<Uri> {
    ensure!(
        value.len() <= 8192 && !value.contains('#'),
        "invalid HTTPS asset URL"
    );
    let uri: Uri = value.parse().context("invalid asset URI")?;
    ensure!(
        uri.scheme_str() == Some("https")
            && uri.host().is_some_and(|h| !h.is_empty())
            && uri.authority().is_some_and(|a| !a.as_str().contains('@')),
        "asset URL requires HTTPS without credentials"
    );
    Ok(uri)
}

/// The caller holds the installation lock and supplies a private, empty stage.
/// Every received payload byte is accounted for, including a partial failed body.
pub fn fetch(
    request: &Path,
    digest: &str,
    stage: &Path,
    install: &Path,
    log: &Path,
) -> Result<u64> {
    let raw = release::pinned(request, digest)?;
    let value: Request = serde_json::from_value(strict_json::parse(&raw)?)?;
    ensure!(
        value.schema == "rust-stable-download-request/v1",
        "unsupported download request schema"
    );
    ensure!(
        (1..=120).contains(&value.timeout_seconds),
        "download timeout must be 1..120 seconds per asset"
    );
    ensure!(
        value
            .assets
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>()
            == release::FILES.into_iter().collect(),
        "download requires exactly five release assets"
    );
    for (name, asset) in &value.assets {
        https_url(&asset.url)?;
        let limit = if name == release::PROGRAM {
            64 * 1024 * 1024
        } else {
            8 * 1024 * 1024
        };
        ensure!(
            asset.bytes > 0 && asset.bytes <= limit,
            "invalid asset size: {name}"
        );
        ensure!(
            asset.sha256.len() == 64
                && asset
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "invalid asset SHA-256: {name}"
        );
    }
    let mut tls = TlsConfig::builder();
    if let Some(pin) = &value.tls_root {
        ensure!(
            !pin.path.starts_with(install) && !pin.path.starts_with(stage),
            "download cannot supply TLS trust"
        );
        let pem = release::pinned(&pin.path, &pin.sha256)?;
        tls = tls.root_certs(RootCerts::new_with_certs(&[Certificate::from_pem(&pem)?]));
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .http_status_as_error(false)
        .tls_config(tls.build())
        .build()
        .into();
    let mut records = Vec::new();
    let mut received = 0;
    let outcome = (|| -> Result<()> {
        for (name, asset) in &value.assets {
            let mut bytes = 0;
            let started = Instant::now();
            let result = (|| -> Result<()> {
                let mut uri = https_url(&asset.url)?;
                let mut response = None;
                for redirect in 0..=3 {
                    let remaining = Duration::from_secs(value.timeout_seconds)
                        .checked_sub(started.elapsed())
                        .context("download timeout")?;
                    let reply = agent
                        .get(uri.to_string())
                        .header("Accept-Encoding", "identity")
                        .config()
                        .timeout_global(Some(remaining))
                        .build()
                        .call()?;
                    let status = reply.status().as_u16();
                    if matches!(status, 301 | 302 | 303 | 307 | 308) {
                        ensure!(redirect < 3, "too many download redirects");
                        uri = https_url(
                            reply
                                .headers()
                                .get("location")
                                .context("redirect missing location")?
                                .to_str()?,
                        )?;
                    } else {
                        ensure!(status == 200, "download HTTP status {status}");
                        response = Some(reply);
                        break;
                    }
                }
                let mut response = response.context("missing download response")?;
                ensure!(
                    response
                        .headers()
                        .get("content-encoding")
                        .is_none_or(|v| v == "identity"),
                    "encoded release asset rejected"
                );
                if let Some(length) = response.headers().get("content-length") {
                    ensure!(
                        length.to_str()?.parse::<u64>()? == asset.bytes,
                        "asset Content-Length mismatch"
                    );
                }
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(stage.join(name))?;
                let mut reader = response.body_mut().as_reader().take(asset.bytes + 1);
                let mut hash = Sha256::new();
                let mut buffer = [0_u8; 65536];
                loop {
                    let count = reader.read(&mut buffer)?;
                    if count == 0 {
                        break;
                    }
                    bytes += count as u64;
                    received += count as u64;
                    ensure!(bytes <= asset.bytes, "asset exceeded declared size");
                    file.write_all(&buffer[..count])?;
                    hash.update(&buffer[..count]);
                }
                ensure!(bytes == asset.bytes, "truncated release asset");
                ensure!(
                    format!("{:x}", hash.finalize()) == asset.sha256,
                    "download asset hash mismatch"
                );
                file.sync_all()?;
                Ok(())
            })();
            records.push(
                json!({"asset":name, "received_body_bytes":bytes, "verified":result.is_ok(),
                "elapsed_ms":started.elapsed().as_millis()}),
            );
            result.with_context(|| format!("download failed for {name}"))?;
        }
        release::pinned(request, digest).context("download request changed")?;
        if let Some(pin) = &value.tls_root {
            release::pinned(&pin.path, &pin.sha256).context("TLS root changed")?;
        }
        Ok(())
    })();
    artifact::write(
        &log.join("download.json"),
        &json!({"schema":"rust-stable-download/v1", "request_sha256":digest,
        "received_body_bytes":received, "byte_scope":"HTTP response bodies read; excludes headers, TLS and proxy overhead",
        "transfers":records, "transfer_complete":outcome.is_ok(), "signature_verification":"separate-required-step", "cache_bytes":0}),
    )?;
    outcome?;
    Ok(received)
}
