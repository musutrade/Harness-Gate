use super::catalog::PRESETS;
#[cfg(unix)]
use super::filesystem::resolve_inside;
use super::filesystem::{atomic_write, atomic_write_batch};
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
    assert_eq!(project_id(Path::new("/tmp/123")), "project-123");
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
fn atomic_write_batch_rejects_broken_symlink() {
    use std::os::unix::fs::symlink;

    let root = TestWorkspace::new("preset-batch-symlink");
    let path = root.root.join("config");
    symlink(root.root.join("missing-target"), &path).expect("create broken symlink");

    assert!(atomic_write_batch(&[(path.as_path(), b"content")]).is_err());
    assert!(fs::symlink_metadata(path)
        .expect("inspect link")
        .file_type()
        .is_symlink());
}

#[test]
fn generated_reference_presets_cross_validate_quality_and_profile_boundaries() {
    use crate::config::quality::{Assurance, QualityConfig};
    for preset in PRESETS {
        let root = TestWorkspace::new(preset.name);
        init(&root, preset.name, false).unwrap();
        let project = crate::project::Project::discover(Some(root.root.clone()), None).unwrap();
        let quality = QualityConfig::load_optional(&root, &project.config).unwrap();
        if preset.recipe.is_none() {
            assert!(quality.is_none());
            continue;
        }
        let quality = quality.unwrap();
        assert_eq!(quality.project.name, project.config.project.name);
        assert_eq!(quality.profiles["hook"].assurance, Assurance::Partial);
        assert!(quality.profiles["hook"].collectors.is_empty());
        for profile in ["full", "ci"] {
            assert_eq!(quality.profiles[profile].assurance, Assurance::Complete);
            assert_eq!(
                quality.profiles[profile].collectors.len(),
                quality.collectors.len()
            );
            assert_eq!(
                quality.profiles[profile].policies.len(),
                quality.policies.len()
            );
            let mut invalid = quality.clone();
            invalid
                .profiles
                .get_mut(profile)
                .unwrap()
                .collectors
                .clear();
            fs::write(
                root.join(".harness-gate/quality.toml"),
                toml::to_string_pretty(&invalid).unwrap(),
            )
            .unwrap();
            assert!(QualityConfig::load_optional(&root, &project.config).is_err());
        }
        if preset.name == "angular-rust-postgres" {
            assert_eq!(
                quality
                    .components
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                ["backend", "frontend"]
            );
            assert_eq!(quality.relationships.len(), 1);
            assert!(project.config.services.contains_key("test-postgres"));
            let contract = &quality.collectors["frontend-api"];
            assert_eq!(contract.produces.len(), 3);
            assert!(contract
                .produces
                .iter()
                .all(|e| serde_json::to_value(&e.target).unwrap()["kind"] == "relationship"));
        }
    }
}

#[test]
fn unknown_ecosystem_pack_composes_without_catalog_or_core_changes() {
    use crate::config::quality::QualityConfig;
    let root = TestWorkspace::new("nebula-unregistered-2049");
    init(&root, "generic", false).unwrap();
    let project = crate::project::Project::discover(Some(root.root.clone()), None).unwrap();
    let recipe = r#"[{"pack":"nebula-unregistered-2049","bindings":{"binding":"project","source_root":"sources"}}]"#;
    let packs = [(
        "nebula-unregistered-2049",
        include_str!("../../../quality/fixtures/workflow/nebula-unregistered.pack.json"),
    )];
    let mut composed =
        super::composition::compose(recipe, &packs, &project.config.project.name).unwrap();
    // The flow-only generic template does not advertise ci; the caller explicitly
    // selects the existing profiles without a technology-specific migration.
    composed.quality.profiles.remove("ci");
    for (path, content) in composed.files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    fs::write(
        root.join(".harness-gate/quality.toml"),
        toml::to_string_pretty(&composed.quality).unwrap(),
    )
    .unwrap();
    QualityConfig::load_optional(&root, &project.config)
        .unwrap()
        .unwrap();
    // Re-initializing generic must not erase an explicit ecosystem adoption.
    let quality_path = root.join(".harness-gate/quality.toml");
    let original = fs::read(&quality_path).unwrap();
    init(&root, "generic", true).unwrap();
    assert_eq!(fs::read(&quality_path).unwrap(), original);
    assert!(super::composition::compose(recipe, &[], "project").is_err());
    assert!(super::composition::compose(
        &recipe.replace("source_root", "missing"),
        &packs,
        "project"
    )
    .is_err());
    let duplicate = format!(
        "[{},{}]",
        &recipe[1..recipe.len() - 1],
        &recipe[1..recipe.len() - 1]
    );
    assert!(super::composition::compose(&duplicate, &packs, "project").is_err());
    assert!(super::composition::compose(
        &recipe.replace("project", "../escape"),
        &packs,
        "project"
    )
    .is_err());
}

#[test]
fn composition_rejects_conflicts_and_malformed_bindings() {
    let recipe = r#"[{"pack":"future","bindings":{"left":"same","right":"same"}}]"#;
    for source in [
        r#"{"quality":{"version":2},"files":{}}"#,
        r#"{"quality":{"${left}":{},"${right}":{}},"files":{}}"#,
        r#"{"quality":{"project":{"name":"${left"}},"files":{}}"#,
        r#"{"quality":{},"files":{"/absolute.json":{}}}"#,
        r#"{"quality":{},"files":{".harness-gate/packs/../escape.json":{}}}"#,
        r#"{"quality":{},"files":{},"unexpected":true}"#,
    ] {
        assert!(super::composition::compose(recipe, &[("future", source)], "project").is_err());
    }
}

#[test]
fn quality_support_file_conflict_does_not_partially_initialize_project() {
    let root = TestWorkspace::new("preset-quality-conflict");
    let path = root.join(".harness-gate/packs/app/policy.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "existing policy").unwrap();
    assert!(init(&root, "rust-api", false).is_err());
    assert!(!root.join(".harness-gate/flow.toml").exists());
    assert!(!root.join(".harness-gate/quality.toml").exists());
    assert_eq!(fs::read_to_string(path).unwrap(), "existing policy");
}
