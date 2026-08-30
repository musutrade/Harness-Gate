use super::catalog::PRESETS;
use super::filesystem::{atomic_write, atomic_write_batch, resolve_inside};
use super::initialize::{init, project_id};
use crate::config::FlowConfig;
use crate::test_support::TestWorkspace;
use std::fs;
use std::path::Path;

#[test]
fn every_embedded_preset_is_valid() {
    for preset in PRESETS {
        FlowConfig::from_source(preset.flow)
            .unwrap_or_else(|error| panic!("preset {} is invalid: {error:#}", preset.name));
    }
}

#[test]
fn project_names_become_portable_ids() {
    assert_eq!(project_id(Path::new("/tmp/My New_API")), "my-new-api");
}

#[test]
fn init_writes_required_security_configs() {
    let root = TestWorkspace::new("preset-init");

    init(&root, "generic", false).expect("initialize preset");
    let project = crate::project::Project::discover(Some(root.root.clone()), None)
        .expect("discover initialized project");

    assert!(project.audit_config.is_file());
    assert!(project.secrets_config.is_file());
}

#[cfg(unix)]
#[test]
fn existing_symlink_cannot_escape_project() {
    use std::os::unix::fs::symlink;

    let root = TestWorkspace::new("preset-path");
    let outside = TestWorkspace::new("preset-outside");
    let outside_file = outside.root.join("outside");
    fs::write(&outside_file, "outside").expect("create outside fixture");
    let link = root.root.join("flow.toml");
    symlink(&outside_file, &link).expect("create symlink fixture");

    let result = resolve_inside(&root.root.canonicalize().expect("canonical root"), link);

    assert!(result.is_err());
}

#[test]
fn atomic_write_replaces_complete_content() {
    let root = TestWorkspace::new("preset-write");
    let path = root.root.join("flow.toml");
    fs::write(&path, "old").expect("write old fixture");

    atomic_write(&path, b"new content").expect("replace fixture");

    assert_eq!(
        fs::read_to_string(&path).expect("read fixture"),
        "new content"
    );
}

#[test]
fn atomic_write_batch_writes_all_entries() {
    let root = TestWorkspace::new("preset-batch-write");
    let first = root.root.join(".harness-gate/first");
    let second = root.root.join(".harness-gate/second");

    atomic_write_batch(&[(first.as_path(), b"first"), (second.as_path(), b"second")])
        .expect("write batch");

    assert_eq!(fs::read_to_string(first).expect("read first"), "first");
    assert_eq!(fs::read_to_string(second).expect("read second"), "second");
}

#[test]
fn atomic_write_batch_cleans_staged_files_when_staging_fails() {
    let root = TestWorkspace::new("preset-batch-failure");
    let existing = root.root.join("existing");
    fs::write(&existing, "old").expect("write old fixture");

    let invalid = Path::new("");
    assert!(atomic_write_batch(&[(existing.as_path(), b"new"), (invalid, b"invalid"),]).is_err());

    assert_eq!(fs::read_to_string(existing).expect("read existing"), "old");
    assert!(fs::read_dir(&root.root)
        .expect("read workspace")
        .filter_map(Result::ok)
        .all(|entry| !entry
            .file_name()
            .to_string_lossy()
            .contains("harness-gate-batch")));
}

#[cfg(unix)]
#[test]
fn atomic_write_batch_restores_backups_when_commit_fails() {
    let root = TestWorkspace::new("preset-batch-rollback");
    let existing = root.root.join("existing");
    fs::write(&existing, "old").expect("write old fixture");
    let directory_entry = root.root.join(".");

    assert!(atomic_write_batch(&[
        (existing.as_path(), b"new"),
        (directory_entry.as_path(), b"invalid"),
    ])
    .is_err());

    assert_eq!(fs::read_to_string(existing).expect("read existing"), "old");
    assert!(fs::read_dir(&root.root)
        .expect("read workspace")
        .filter_map(Result::ok)
        .all(|entry| !entry
            .file_name()
            .to_string_lossy()
            .contains("harness-gate-backup")));
}

#[cfg(unix)]
#[test]
fn atomic_write_batch_replaces_broken_symlink() {
    use std::os::unix::fs::symlink;

    let root = TestWorkspace::new("preset-batch-symlink");
    let path = root.root.join("config");
    symlink(root.root.join("missing-target"), &path).expect("create broken symlink");

    atomic_write_batch(&[(path.as_path(), b"content")]).expect("replace symlink");

    assert_eq!(fs::read_to_string(path).expect("read config"), "content");
}
