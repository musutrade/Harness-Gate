use super::checks::check_remotes;
use crate::test_support::TestWorkspace;
use std::time::Duration;

#[test]
fn git_remote_check_rejects_non_git_directory() {
    let root = TestWorkspace::new("doctor");

    let error =
        check_remotes(&root, Duration::from_secs(2)).expect_err("non-Git directory must fail");

    assert!(error.to_string().contains("not a Git worktree"));
}
