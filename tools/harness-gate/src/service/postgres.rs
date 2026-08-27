use crate::config::ExternalValuePolicy;
use anyhow::{bail, Context, Result};
use std::net::IpAddr;
use url::{Host, Url};

pub(super) const ALLOW_REMOTE_TEST_DATABASE_ENV: &str = "HARNESS_GATE_ALLOW_REMOTE_TEST_DATABASE";

pub(super) fn validate_external_value(policy: ExternalValuePolicy, value: &str) -> Result<()> {
    match policy {
        ExternalValuePolicy::None => Ok(()),
        ExternalValuePolicy::IsolatedPostgres => {
            let production_url = std::env::var("DATABASE_URL").ok();
            let allow_remote = std::env::var(ALLOW_REMOTE_TEST_DATABASE_ENV)
                .is_ok_and(|flag| matches!(flag.as_str(), "1" | "true"));
            validate_isolated_postgres_url(value, production_url.as_deref(), allow_remote)
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct PostgresTarget {
    host: String,
    port: u16,
    database: String,
    loopback: bool,
}

fn parse_postgres_target(value: &str) -> Result<PostgresTarget> {
    let url = Url::parse(value).context("test database URL must be a valid URL")?;
    if !matches!(url.scheme(), "postgres" | "postgresql") {
        bail!("test database URL must use the postgres or postgresql scheme");
    }
    let (host, loopback) = match url
        .host()
        .context("test database URL must include a host")?
    {
        Host::Domain(host) => {
            let normalized = host.to_ascii_lowercase();
            let loopback = host.eq_ignore_ascii_case("localhost")
                || host
                    .parse::<IpAddr>()
                    .is_ok_and(|address| address.is_loopback());
            (normalized, loopback)
        }
        Host::Ipv4(address) => (address.to_string(), IpAddr::V4(address).is_loopback()),
        Host::Ipv6(address) => (address.to_string(), IpAddr::V6(address).is_loopback()),
    };
    let mut segments = url
        .path_segments()
        .context("test database URL must include a database name")?;
    let database = segments
        .next()
        .filter(|segment| !segment.is_empty())
        .context("test database URL must include a database name")?
        .to_ascii_lowercase();
    if segments.any(|segment| !segment.is_empty()) {
        bail!("test database URL must contain exactly one database name");
    }
    Ok(PostgresTarget {
        host,
        port: url.port().unwrap_or(5432),
        database,
        loopback,
    })
}

pub(super) fn validate_isolated_postgres_url(
    value: &str,
    production_url: Option<&str>,
    allow_remote: bool,
) -> Result<()> {
    let target = parse_postgres_target(value)?;
    if !target.database.ends_with("_test") && !target.database.ends_with("-test") {
        bail!("test database name must end with _test or -test");
    }
    if !target.loopback && !allow_remote {
        bail!(
            "remote test databases require {ALLOW_REMOTE_TEST_DATABASE_ENV}=1 after isolation is verified"
        );
    }
    if let Some(production_url) = production_url {
        if value == production_url {
            bail!("TEST_DATABASE_URL must not equal DATABASE_URL");
        }
        if let Ok(production) = parse_postgres_target(production_url) {
            let same_host =
                target.host == production.host || (target.loopback && production.loopback);
            if same_host && target.port == production.port && target.database == production.database
            {
                bail!("TEST_DATABASE_URL must not target the DATABASE_URL database");
            }
        }
    }
    Ok(())
}
