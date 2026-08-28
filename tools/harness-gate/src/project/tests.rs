use super::*;
use crate::test_support::TestWorkspace;
use std::{env, path::Path};

#[test]
fn preparation_does_not_change_the_process_working_directory() {
    let workspace = TestWorkspace::new("project-prepare");
    crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
    let project = Project::discover(Some(workspace.root.clone()), None).expect("discover fixture");
    let before = env::current_dir().expect("read working directory");

    project.prepare().expect("prepare project");

    assert_eq!(
        env::current_dir().expect("read working directory"),
        before,
        "preparing one project must not affect another project's process context"
    );
}

#[cfg(unix)]
#[test]
fn repository_path_rejects_symlink_escape() {
    use std::os::unix::fs::symlink;

    let root = TestWorkspace::new("project-root");
    let outside = TestWorkspace::new("project-outside");
    symlink(&outside.root, root.join("reports")).expect("create symlink");

    let error = resolve_repo_path(&root, Path::new("reports"), "reports", false)
        .expect_err("symlink escape must fail");

    assert!(error.to_string().contains("escapes the repository"));
}
