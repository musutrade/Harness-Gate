use super::super::AllowlistEntry;
use crate::project::resolve_repo_path;
use anyhow::{bail, Context, Result};
use regex::{Regex, RegexBuilder};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn compile_regexes(patterns: &[String]) -> Result<Vec<Regex>> {
    patterns
        .iter()
        .map(|pattern| {
            RegexBuilder::new(pattern)
                .multi_line(true)
                .build()
                .with_context(|| format!("invalid audit regex {pattern:?}"))
        })
        .collect()
}

pub(crate) fn resolve_rule_roots(
    project_root: &Path,
    entries: &[String],
    aliases: &HashMap<String, String>,
    rule_name: &str,
) -> Result<Vec<PathBuf>> {
    if entries.is_empty() {
        bail!("audit rule {rule_name:?} requires at least one path");
    }
    entries
        .iter()
        .map(|entry| aliases.get(entry).unwrap_or(entry))
        .map(|entry| {
            resolve_repo_path(
                project_root,
                Path::new(entry),
                &format!("audit rule {rule_name:?} path"),
                true,
            )
        })
        .collect()
}

pub(crate) fn resolve_excludes(project_root: &Path, entries: &[String]) -> Result<Vec<PathBuf>> {
    entries
        .iter()
        .map(|entry| {
            resolve_repo_path(project_root, Path::new(entry), "audit excluded path", false)
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn is_allowlisted(
    path: &Path,
    project_root: &Path,
    allowlist: &[AllowlistEntry],
) -> bool {
    let relative = path.strip_prefix(project_root).unwrap_or(path);
    let path_str = relative.to_str().unwrap_or("");
    allowlist.iter().any(|entry| match entry {
        AllowlistEntry::PathPrefix { path } => relative.starts_with(Path::new(path)),
        AllowlistEntry::Regex { pattern } => Regex::new(pattern)
            .map(|re| re.is_match(path_str))
            .unwrap_or(false),
    })
}

#[derive(Debug, Clone)]
pub(crate) enum CompiledAllowlistEntry {
    PathPrefix(PathBuf),
    Regex(Regex),
}

pub(crate) fn compile_allowlist(
    allowlist: &[AllowlistEntry],
    rule_name: &str,
) -> Result<Vec<CompiledAllowlistEntry>> {
    allowlist
        .iter()
        .map(|entry| match entry {
            AllowlistEntry::PathPrefix { path } => {
                Ok(CompiledAllowlistEntry::PathPrefix(PathBuf::from(path)))
            }
            AllowlistEntry::Regex { pattern } => Regex::new(pattern)
                .map(CompiledAllowlistEntry::Regex)
                .with_context(|| {
                    format!("audit rule {rule_name:?} has invalid allowlist regex {pattern:?}")
                }),
        })
        .collect()
}

pub(crate) fn is_allowlisted_compiled(
    path: &Path,
    project_root: &Path,
    allowlist: &[CompiledAllowlistEntry],
) -> bool {
    let relative = path.strip_prefix(project_root).unwrap_or(path);
    let path_str = relative.to_str().unwrap_or("");
    allowlist.iter().any(|entry| match entry {
        CompiledAllowlistEntry::PathPrefix(prefix) => relative.starts_with(prefix),
        CompiledAllowlistEntry::Regex(regex) => regex.is_match(path_str),
    })
}

/// `ignore` can return file symlinks even when link traversal is disabled.
/// Reject them immediately before any scanner reads file contents.
pub(crate) fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file() && !metadata.file_type().is_symlink())
        .unwrap_or(false)
}
