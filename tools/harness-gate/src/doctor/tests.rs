use super::checks::{check_glob, check_remotes};
use crate::test_support::TestWorkspace;
use std::fs;
use std::time::Duration;

#[test]
fn git_remote_check_rejects_non_git_directory() {
    let root = TestWorkspace::new("doctor");

    let error =
        check_remotes(&root, Duration::from_secs(2)).expect_err("non-Git directory must fail");

    assert!(error.to_string().contains("not a Git worktree"));
}

#[test]
fn relative_glob_matches_paths_from_the_project_root() {
    let root = TestWorkspace::new("doctor-glob");
    crate::preset::init(&root.root, "generic", false).expect("initialize fixture");
    root.init_git();
    fs::write(
        root.root.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\n",
    )
    .expect("write glob fixture");
    let project =
        crate::project::Project::discover(Some(root.root.clone()), None).expect("discover fixture");

    let result = check_glob(&project, "Cargo.toml");

    assert!(result.is_ok(), "relative glob should match: {result:?}");
}
