use super::postgres::{validate_isolated_postgres_url, ALLOW_REMOTE_TEST_DATABASE_ENV};

#[test]
fn accepts_an_isolated_loopback_database() {
    let result = validate_isolated_postgres_url(
        "postgres://tester:secret@127.0.0.1:5432/arc_admin_test",
        Some("postgres://developer:secret@localhost:5432/arc_admin"),
        false,
    );
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn accepts_an_ipv6_loopback_database() {
    assert!(validate_isolated_postgres_url(
        "postgres://tester:secret@[::1]:5432/arc_admin_test",
        None,
        false,
    )
    .is_ok());
}

#[test]
fn rejects_a_database_without_a_test_suffix() {
    let error =
        validate_isolated_postgres_url("postgres://tester:secret@localhost/arc_admin", None, false)
            .expect_err("production-like database name must fail");
    assert!(error.to_string().contains("must end with"));
}

#[test]
fn rejects_a_remote_database_without_an_explicit_override() {
    let error = validate_isolated_postgres_url(
        "postgresql://tester:secret@test-db.example.com/arc_admin_test",
        None,
        false,
    )
    .expect_err("remote database must fail closed");
    assert!(error.to_string().contains(ALLOW_REMOTE_TEST_DATABASE_ENV));
}

#[test]
fn accepts_an_explicitly_allowed_remote_test_database() {
    assert!(validate_isolated_postgres_url(
        "postgres://tester:secret@test-db.example.com/arc-admin-test",
        None,
        true,
    )
    .is_ok());
}

#[test]
fn rejects_the_configured_runtime_database() {
    let error = validate_isolated_postgres_url(
        "postgres://tester:secret@127.0.0.1:5432/arc_admin_test",
        Some("postgresql://runtime:secret@localhost:5432/arc_admin_test"),
        false,
    )
    .expect_err("same database target must fail");
    assert!(
        error.to_string().contains("DATABASE_URL database"),
        "{error:?}"
    );
}
