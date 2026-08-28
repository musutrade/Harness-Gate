use super::postgres::{validate_isolated_postgres_url, ALLOW_REMOTE_TEST_DATABASE_ENV};
use super::{check_available, ServiceManager};
use crate::config::ServiceConfig;
use crate::project::Project;
use crate::test_support::TestWorkspace;
use std::time::Duration;

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

fn project() -> (TestWorkspace, Project) {
    let workspace = TestWorkspace::new("service-manager");
    crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
    workspace.init_git();
    let project = Project::discover(Some(workspace.root.clone()), None).expect("discover fixture");
    (workspace, project)
}

#[test]
fn environment_service_injects_configured_value() {
    let (_workspace, mut project) = project();
    let name = "HARNESS_GATE_SERVICE_FIXTURE";
    std::env::set_var(name, "http://127.0.0.1:1234");
    project.config.services.insert(
        "fixture".into(),
        ServiceConfig::Environment {
            source_env: name.into(),
            inject_env: "FIXTURE_URL".into(),
        },
    );
    let mut manager = ServiceManager::new(&project);
    let value = manager.environment("fixture").expect("environment service");
    assert_eq!(
        value,
        ("FIXTURE_URL".into(), "http://127.0.0.1:1234".into())
    );
    std::env::remove_var(name);
}

#[test]
fn missing_service_is_cached_as_a_failure() {
    let (_workspace, project) = project();
    let mut manager = ServiceManager::new(&project);
    let first = manager.environment("unknown").expect_err("unknown service");
    let second = manager.environment("unknown").expect_err("cached failure");
    assert!(first.to_string().contains("unknown service"));
    assert!(second.to_string().contains("unknown service"));
}

#[test]
fn check_available_reports_unknown_service_and_environment_service() {
    let (_workspace, mut project) = project();
    let error =
        check_available(&project, "unknown", Duration::from_secs(1)).expect_err("unknown service");
    assert!(error.to_string().contains("unknown service"));
    let name = "HARNESS_GATE_CHECK_SERVICE_FIXTURE";
    std::env::set_var(name, "configured");
    project.config.services.insert(
        "fixture".into(),
        ServiceConfig::Environment {
            source_env: name.into(),
            inject_env: "FIXTURE".into(),
        },
    );
    assert_eq!(
        check_available(&project, "fixture", Duration::from_secs(1)).expect("available"),
        format!("{name} is configured")
    );
    std::env::remove_var(name);
}

#[test]
fn environment_service_rejects_missing_and_empty_values() {
    let (_workspace, mut project) = project();
    let missing = "HARNESS_GATE_MISSING_SERVICE_FIXTURE";
    std::env::remove_var(missing);
    project.config.services.insert(
        "missing".into(),
        ServiceConfig::Environment {
            source_env: missing.into(),
            inject_env: "MISSING".into(),
        },
    );
    let mut manager = ServiceManager::new(&project);
    assert!(manager.environment("missing").is_err());

    let empty = "HARNESS_GATE_EMPTY_SERVICE_FIXTURE";
    std::env::set_var(empty, "   ");
    project.config.services.insert(
        "empty".into(),
        ServiceConfig::Environment {
            source_env: empty.into(),
            inject_env: "EMPTY".into(),
        },
    );
    let mut manager = ServiceManager::new(&project);
    assert!(manager.environment("empty").is_err());
    std::env::remove_var(empty);
}

#[test]
fn check_available_rejects_empty_environment_service() {
    let (_workspace, mut project) = project();
    let name = "HARNESS_GATE_EMPTY_CHECK_SERVICE_FIXTURE";
    std::env::set_var(name, "");
    project.config.services.insert(
        "empty".into(),
        ServiceConfig::Environment {
            source_env: name.into(),
            inject_env: "EMPTY".into(),
        },
    );
    assert!(check_available(&project, "empty", Duration::from_secs(1)).is_err());
    std::env::remove_var(name);
}

#[test]
fn docker_service_uses_a_valid_external_value_without_docker() {
    let (_workspace, mut project) = project();
    let name = "HARNESS_GATE_DOCKER_EXTERNAL_SERVICE_FIXTURE";
    std::env::set_var(name, "postgres://test:test@127.0.0.1:5432/quality_test");
    project.config.services.insert(
        "database".into(),
        ServiceConfig::Docker {
            image: "postgres:16-alpine".into(),
            image_env: None,
            external_env: Some(name.into()),
            inject_env: "TEST_DATABASE_URL".into(),
            external_value_policy: crate::config::ExternalValuePolicy::IsolatedPostgres,
            startup_timeout_secs: 1,
            timeout_env: None,
            container_port: 5432,
            environment: Default::default(),
            healthcheck: vec!["true".into()],
            connection: "postgres://unused".into(),
        },
    );
    let mut manager = ServiceManager::new(&project);
    assert_eq!(
        manager.environment("database").expect("external value"),
        (
            "TEST_DATABASE_URL".into(),
            "postgres://test:test@127.0.0.1:5432/quality_test".into()
        )
    );
    std::env::remove_var(name);
}
