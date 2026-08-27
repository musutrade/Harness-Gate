use super::super::AllowlistEntry;
use crate::project::resolve_repo_path;
use anyhow::{bail, Context, Result};
use regex::{Regex, RegexBuilder};
use std::collections::HashMap;
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
