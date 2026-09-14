//! Release verification and transactional selection. No signer.
//! The host pins trust; release files cannot choose a key, verifier or identity.
use crate::{artifact, process::Runner, strict_json, support::Support};
use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use rsa::{
    pkcs1v15, pkcs8::DecodePublicKey, signature::Verifier, traits::PublicKeyParts, RsaPublicKey,
};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
};

pub(crate) const PROGRAM: &str = "harness-gate-rust-stable-collector";
const INVENTORY: &str = "release-inventory.json";
const SIGNATURE: &str = "release-inventory.sig";
pub(crate) const FILES: [&str; 5] = [PROGRAM, "LICENSE", "support.json", INVENTORY, SIGNATURE];
const IDENTITY: &str = "https://github.com/musutrade/Harness-Gate/.github/workflows/rust-collector-release.yml@refs/heads/main";
const ISSUER: &str = "https://token.actions.githubusercontent.com";
const LIMIT: u64 = 64 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inventory {
    schema: String,
    version: String,
    target: String,
    files: BTreeMap<String, artifact::FileIdentity>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Signatures {
    schema: String,
    rsa_signature: String,
    sigstore_bundle: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Trust {
    schema: String,
    public_key: PathBuf,
    public_key_sha256: String,
    cosign: PathBuf,
    cosign_sha256: String,
    trusted_root: PathBuf,
    trusted_root_sha256: String,
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn absolute(path: &Path) -> Result<()> {
    ensure!(
        path.is_absolute()
            && path
                .canonicalize()
                .with_context(|| format!("cannot resolve {}", path.display()))?
                == path,
        "absolute canonical path required: {}",
        path.display()
    );
    Ok(())
}
fn read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    ensure!(
        file.metadata()?.is_file() && file.metadata()?.len() <= limit,
        "invalid or oversized file: {}",
        path.display()
    );
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "file grew beyond limit");
    Ok(bytes)
}
fn typed<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    Ok(serde_json::from_value(strict_json::parse(bytes)?)?)
}
pub(crate) fn pinned(path: &Path, digest: &str) -> Result<Vec<u8>> {
    absolute(path)?;
    let bytes = read(path, LIMIT)?;
    ensure!(
        valid_digest(digest) && artifact::digest(&bytes) == digest,
        "host trust pin changed: {}",
        path.display()
    );
    Ok(bytes)
}
// cosign is an external dependency and can be larger than our release payload.
// Hash it incrementally; its size must not determine installer memory use.
fn tool_pin(path: &Path, digest: &str) -> Result<()> {
    absolute(path)?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    ensure!(file.metadata()?.is_file(), "verifier is not a regular file");
    let mut hasher = Sha256::new();
    let mut buffer = [0; 65536];
    let mut bytes = 0_u64;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes += count as u64;
        ensure!(
            bytes <= 512 * 1024 * 1024,
            "external verifier exceeds size limit"
        );
        hasher.update(&buffer[..count]);
    }
    ensure!(
        valid_digest(digest) && format!("{:x}", hasher.finalize()) == digest,
        "host verifier pin changed: {}",
        path.display()
    );
    Ok(())
}
fn write(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(mode)
        .open(path)?;
    file.write_all(bytes)?;
    file.set_permissions(fs::Permissions::from_mode(mode))?;
    file.sync_all()?;
    Ok(())
}
fn sync_dir(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}
fn exact_files(root: &Path) -> Result<()> {
    absolute(root)?;
    ensure!(fs::metadata(root)?.is_dir(), "bundle must be a directory");
    let mut actual = BTreeSet::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        ensure!(
            entry.file_type()?.is_file(),
            "bundle contains a symlink, directory or special file"
        );
        actual.insert(
            entry
                .file_name()
                .into_string()
                .map_err(|_| anyhow::anyhow!("non-UTF8 asset"))?,
        );
    }
    ensure!(
        actual == FILES.into_iter().map(String::from).collect(),
        "release must contain exactly the five declared assets"
    );
    Ok(())
}
fn trust(path: &Path, digest: &str, bundle: &Path, install: Option<&Path>) -> Result<Trust> {
    let value: Trust = typed(&pinned(path, digest)?)?;
    ensure!(
        value.schema == "rust-stable-release-trust/v1",
        "unsupported host trust schema"
    );
    for file in [path, &value.public_key, &value.cosign, &value.trusted_root] {
        absolute(file)?;
        ensure!(
            !file.starts_with(bundle) && !install.is_some_and(|root| file.starts_with(root)),
            "release/install cannot supply host trust"
        );
    }
    pinned(&value.public_key, &value.public_key_sha256)?;
    tool_pin(&value.cosign, &value.cosign_sha256).context("provision the explicitly pinned cosign verifier at the host trust path; no automatic installation")?;
    pinned(&value.trusted_root, &value.trusted_root_sha256)?;
    Ok(value)
}

/// A verified snapshot is the only input accepted by the activation operation.
struct Verified {
    directory: tempfile::TempDir,
    digest: String,
    version: String,
    bytes: u64,
    trust_digest: String,
}
fn snapshot(
    bundle: &Path,
    parent: &Path,
    host: &Path,
    host_digest: &str,
    install: Option<&Path>,
    runner: &mut Runner,
) -> Result<Verified> {
    exact_files(bundle)?;
    let host_trust = trust(host, host_digest, bundle, install)?;
    let stage = tempfile::Builder::new()
        .prefix(".staging-")
        .tempdir_in(parent)?;
    let mut total = 0;
    for name in FILES {
        let bytes = read(
            &bundle.join(name),
            if name == PROGRAM {
                LIMIT
            } else {
                8 * 1024 * 1024
            },
        )?;
        total += bytes.len() as u64;
        write(
            &stage.path().join(name),
            &bytes,
            if name == PROGRAM { 0o755 } else { 0o644 },
        )?;
    }
    let staged_identity = artifact::inventory(stage.path(), false)?;
    let raw = read(&stage.path().join(INVENTORY), 8 * 1024 * 1024)?;
    let inventory: Inventory = typed(&raw)?;
    ensure!(
        inventory.schema == "rust-stable-release-inventory/v1",
        "unsupported release inventory"
    );
    ensure!(
        inventory.target == "x86_64-unknown-linux-gnu"
            && cfg!(all(
                target_os = "linux",
                target_arch = "x86_64",
                target_env = "gnu"
            )),
        "unsupported release target"
    );
    let version = &inventory.version;
    ensure!(
        !version.is_empty()
            && version.len() <= 64
            && version
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b)),
        "invalid release version"
    );
    ensure!(
        inventory
            .files
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            == [PROGRAM, "LICENSE", "support.json"].into_iter().collect(),
        "invalid payload inventory"
    );
    for (name, expected) in &inventory.files {
        ensure!(
            valid_digest(&expected.sha256)
                && artifact::identity(&stage.path().join(name))? == *expected,
            "release payload mismatch: {name}"
        );
    }
    let support: Support = typed(&read(&stage.path().join("support.json"), 8 * 1024 * 1024)?)?;
    support.validate(
        &inventory.version,
        &inventory.target,
        &inventory.files[PROGRAM],
        &inventory.files["LICENSE"],
    )?;
    let executable = read(&stage.path().join(PROGRAM), LIMIT)?;
    ensure!(
        executable.len() > 20
            && executable[..5] == *b"\x7fELF\x02"
            && executable[5] == 1
            && executable[18..20] == [62, 0],
        "release program must be an x86_64 ELF executable"
    );
    let signatures: Signatures = typed(&read(&stage.path().join(SIGNATURE), 8 * 1024 * 1024)?)?;
    ensure!(
        signatures.schema == "rust-collector-signatures/v2"
            && signatures
                .sigstore_bundle
                .as_object()
                .is_some_and(|v| !v.is_empty()),
        "both RSA and Sigstore signatures are required"
    );
    let pem = pinned(&host_trust.public_key, &host_trust.public_key_sha256)?;
    let key = RsaPublicKey::from_public_key_pem(std::str::from_utf8(&pem)?)?;
    ensure!(
        (2048..=8192).contains(&key.n().bits()),
        "RSA key must have 2048..8192 bits"
    );
    let signature = STANDARD.decode(&signatures.rsa_signature)?;
    let signature = pkcs1v15::Signature::try_from(signature.as_slice())?;
    pkcs1v15::VerifyingKey::<Sha256>::new(key)
        .verify(&raw, &signature)
        .context("RSA release signature verification failed")?;
    let verification = tempfile::Builder::new()
        .prefix(".verification-")
        .tempdir_in(parent)?;
    write(&verification.path().join(INVENTORY), &raw, 0o600)?;
    write(
        &verification.path().join("bundle.json"),
        &serde_json::to_vec(&signatures.sigstore_bundle)?,
        0o600,
    )?;
    write(
        &verification.path().join("trusted-root.json"),
        &pinned(&host_trust.trusted_root, &host_trust.trusted_root_sha256)?,
        0o600,
    )?;
    runner.environment = BTreeMap::from([
        ("PATH".into(), "/usr/bin:/bin".into()),
        ("HOME".into(), verification.path().display().to_string()),
        ("LANG".into(), "C".into()),
    ]);
    runner
        .run(
            &host_trust.cosign,
            &[
                "verify-blob".into(),
                "--bundle".into(),
                verification
                    .path()
                    .join("bundle.json")
                    .display()
                    .to_string(),
                "--trusted-root".into(),
                verification
                    .path()
                    .join("trusted-root.json")
                    .display()
                    .to_string(),
                "--offline".into(),
                "--certificate-identity".into(),
                IDENTITY.into(),
                "--certificate-oidc-issuer".into(),
                ISSUER.into(),
                verification.path().join(INVENTORY).display().to_string(),
            ],
            &BTreeMap::new(),
        )
        .context("Sigstore identity/inclusion/signature verification failed")?;
    // External verification must not change its input or any trust material.
    ensure!(
        read(&verification.path().join(INVENTORY), LIMIT)? == raw,
        "verifier changed inventory"
    );
    ensure!(
        read(&verification.path().join("bundle.json"), LIMIT)?
            == serde_json::to_vec(&signatures.sigstore_bundle)?,
        "verifier changed signature bundle"
    );
    ensure!(
        artifact::identity(&verification.path().join("trusted-root.json"))?.sha256
            == host_trust.trusted_root_sha256,
        "verifier changed trust root"
    );
    trust(host, host_digest, bundle, install)?;
    let check_stage = || -> Result<()> {
        for (name, expected) in &inventory.files {
            ensure!(
                artifact::identity(&stage.path().join(name))? == *expected,
                "staged payload changed"
            );
        }
        ensure!(
            read(&stage.path().join(INVENTORY), LIMIT)? == raw,
            "staged inventory changed"
        );
        ensure!(
            artifact::inventory(stage.path(), false)? == staged_identity,
            "staged release changed during verification"
        );
        Ok(())
    };
    check_stage()?;
    // Execute only the authenticated snapshot, after both signature checks.
    // This checks launch/version compatibility, not measurement certification.
    let version = runner
        .text(&stage.path().join(PROGRAM), &["--version"])
        .context("authenticated release program launch check failed")?;
    ensure!(
        version == format!("{PROGRAM} {}\n", inventory.version),
        "authenticated release program version mismatch"
    );
    check_stage()?;
    trust(host, host_digest, bundle, install)?;
    fs::set_permissions(stage.path(), fs::Permissions::from_mode(0o755))?;
    sync_dir(stage.path())?;
    Ok(Verified {
        directory: stage,
        digest: artifact::digest(&raw),
        version: inventory.version,
        bytes: total,
        trust_digest: host_digest.into(),
    })
}

struct LockedRoot {
    root: PathBuf,
    _lock: File,
}
impl LockedRoot {
    fn open(root: &Path) -> Result<Self> {
        absolute(root)?;
        let meta = fs::metadata(root)?;
        ensure!(
            meta.is_dir() && meta.uid() == unsafe { libc::geteuid() } && meta.mode() & 0o022 == 0,
            "install root must be owned by this user and not group/world writable"
        );
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(root.join(".lock"))?;
        ensure!(lock.metadata()?.is_file(), "invalid installation lock");
        ensure!(
            unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
            "another installation transaction is active"
        );
        let versions = root.join("versions");
        if !versions.exists() {
            // Set the creation mode atomically; umask may only remove access.
            // Existing directories still undergo validation without chmod.
            fs::DirBuilder::new().mode(0o755).create(&versions)?;
        }
        absolute(&versions)?;
        let metadata = fs::metadata(&versions)?;
        ensure!(
            metadata.is_dir() && metadata.uid() == meta.uid() && metadata.mode() & 0o022 == 0,
            "unsafe versions directory"
        );
        Ok(Self {
            root: root.into(),
            _lock: lock,
        })
    }
    fn current(&self) -> Result<Option<String>> {
        let path = self.root.join("current");
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
            Ok(metadata) => {
                ensure!(metadata.is_symlink(), "current selection must be a symlink");
                let target = fs::read_link(path)?;
                let digest = target
                    .file_name()
                    .and_then(|v| v.to_str())
                    .context("invalid current selection")?;
                ensure!(
                    valid_digest(digest) && target == Path::new("versions").join(digest),
                    "unsafe current selection"
                );
                ensure!(
                    self.root.join(&target).is_dir(),
                    "current selection is missing"
                );
                Ok(Some(digest.into()))
            }
        }
    }
    fn activate(&self, verified: Verified) -> Result<Value> {
        let previous = self.current()?;
        let versions = self.root.join("versions");
        let destination = versions.join(&verified.digest);
        if destination.exists() {
            exact_files(&destination)?;
            ensure!(
                fs::metadata(&destination)?.mode() & 0o022 == 0,
                "unsafe installed version permissions"
            );
            for name in FILES {
                let mode = fs::metadata(destination.join(name))?.mode();
                ensure!(
                    mode & 0o022 == 0 && (name != PROGRAM || mode & 0o111 == 0o111),
                    "unsafe installed asset permissions"
                );
            }
            ensure!(
                artifact::inventory(&destination, false)?
                    == artifact::inventory(verified.directory.path(), false)?,
                "existing version is corrupt or differs from signed snapshot"
            );
        } else {
            fs::rename(verified.directory.path(), &destination)?;
            sync_dir(&versions)?;
        }
        // A private directory avoids predictable temporary symlink names. rename
        // is the only operation changing the live selection, on the same FS.
        let selection = tempfile::Builder::new()
            .prefix(".selection-")
            .tempdir_in(&self.root)?;
        std::os::unix::fs::symlink(
            Path::new("versions").join(&verified.digest),
            selection.path().join("current"),
        )?;
        sync_dir(selection.path())?;
        fs::rename(selection.path().join("current"), self.root.join("current"))?;
        sync_dir(&self.root).context(
            "selection committed but directory sync failed; inspect current before retrying",
        )?;
        Ok(
            json!({"schema":"rust-stable-installation/v1", "current":verified.digest, "previous":previous,
            "version":verified.version, "installed_version_bytes":verified.bytes, "package_bytes":verified.bytes,
            "download_bytes":0, "trust_sha256":verified.trust_digest, "executable":self.root.join("current").join(PROGRAM)}),
        )
    }
}

pub fn verify(bundle: &Path, host: &Path, digest: &str, log: &Path) -> Result<Value> {
    absolute(bundle)?;
    let mut runner = Runner::create(log, bundle, 60)?;
    let parent = runner.root.clone();
    let verified = snapshot(bundle, &parent, host, digest, None, &mut runner)?;
    Ok(
        json!({"schema":"rust-stable-release-verification/v1", "inventory_sha256":verified.digest,
        "version":verified.version, "package_bytes":verified.bytes, "signatures":"rsa-and-sigstore"}),
    )
}
pub fn install(bundle: &Path, host: &Path, digest: &str, root: &Path, log: &Path) -> Result<Value> {
    absolute(bundle)?;
    let install = LockedRoot::open(root)?;
    ensure!(
        !bundle.starts_with(root) && !root.starts_with(bundle),
        "install and input bundle must be separate"
    );
    let mut runner = Runner::create(log, root, 60)?;
    let snapshot = snapshot(
        bundle,
        &root.join("versions"),
        host,
        digest,
        Some(root),
        &mut runner,
    )?;
    install.activate(snapshot)
}
pub fn download_install(
    request: &Path,
    request_digest: &str,
    host: &Path,
    digest: &str,
    root: &Path,
    log: &Path,
) -> Result<Value> {
    // Reject unpinned input/trust before any network access. Keep the lock across
    // transport, authentication and activation so concurrent upgrades cannot race.
    pinned(request, request_digest)?;
    let install = LockedRoot::open(root)?;
    let mut runner = Runner::create(log, root, 60)?;
    let stage = tempfile::Builder::new()
        .prefix(".download-")
        .tempdir_in(root)?;
    trust(host, digest, stage.path(), Some(root))?;
    let bytes = crate::download::fetch(request, request_digest, stage.path(), root, log)?;
    let verified = snapshot(
        stage.path(),
        &root.join("versions"),
        host,
        digest,
        Some(root),
        &mut runner,
    )?;
    pinned(request, request_digest).context("download request changed before activation")?;
    let mut result = install.activate(verified)?;
    result["download_bytes"] = json!(bytes);
    result["download_request_sha256"] = json!(request_digest);
    result["download_byte_scope"] =
        json!("HTTP response bodies read; excludes headers, TLS and proxy overhead");
    Ok(result)
}

pub fn rollback(
    root: &Path,
    version: &str,
    host: &Path,
    digest: &str,
    log: &Path,
) -> Result<Value> {
    ensure!(
        valid_digest(version),
        "rollback needs the exact inventory SHA-256"
    );
    let install = LockedRoot::open(root)?;
    ensure!(
        install.current()?.is_some(),
        "no current installation to roll back"
    );
    let bundle = root.join("versions").join(version);
    let mut runner = Runner::create(log, root, 60)?;
    let verified = snapshot(
        &bundle,
        &root.join("versions"),
        host,
        digest,
        Some(root),
        &mut runner,
    )?;
    ensure!(verified.digest == version, "rollback identity mismatch");
    install.activate(verified)
}
