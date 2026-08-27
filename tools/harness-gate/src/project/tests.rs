use super::*;
use crate::test_support::TestWorkspace;
use std::path::Path;

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
