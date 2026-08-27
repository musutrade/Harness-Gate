use anyhow::{bail, Result};
use std::path::{Component as PathComponent, Path};

pub(super) fn validate_id(name: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || !value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '-' | '_')
        })
    {
        bail!("{name} must be a lowercase identifier, found {value:?}");
    }
    Ok(())
}

pub(super) fn validate_program(name: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.contains('/')
        || value.contains('\\')
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '+')
        })
    {
        bail!("{name} must be a bare executable name, found {value:?}");
    }
    Ok(())
}

pub(super) fn validate_env_name(name: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || !value.chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        })
    {
        bail!("{name} must be an uppercase environment variable name, found {value:?}");
    }
    Ok(())
}

pub(super) fn validate_image(value: &str) -> Result<()> {
    if value.is_empty()
        || value.starts_with('-')
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '.' | '_' | '/' | ':' | '@' | '-')
        })
    {
        bail!("Docker image must be an OCI image reference, found {value:?}");
    }
    Ok(())
}

pub(super) fn validate_repo_path(name: &str, value: &str) -> Result<()> {
    let path = Path::new(value);
    if value.is_empty() || path.is_absolute() {
        bail!("{name} must be a non-empty repository-relative path");
    }
    if path.components().any(|component| {
        matches!(
            component,
            PathComponent::ParentDir | PathComponent::RootDir | PathComponent::Prefix(_)
        )
    }) {
        bail!("{name} may not escape the repository: {value:?}");
    }
    Ok(())
}
