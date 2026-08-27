use super::{
    AllowlistEntry, ArchViolation, BlockCommentSyntax, CommentSyntax, Config, EngineConfig,
    HardRule, StringSyntax, Violation,
};
use crate::project::resolve_repo_path;
use anyhow::{bail, Context, Result};
use ignore::WalkBuilder;
use rayon::prelude::*;
use regex::{Regex, RegexBuilder};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

pub(super) fn compile_regexes(patterns: &[String]) -> Result<Vec<Regex>> {
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

pub(super) fn resolve_rule_roots(
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

pub(super) fn resolve_excludes(project_root: &Path, entries: &[String]) -> Result<Vec<PathBuf>> {
    entries
        .iter()
        .map(|entry| {
            resolve_repo_path(project_root, Path::new(entry), "audit excluded path", false)
        })
        .collect()
}

fn source_line_starts(content: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            content
                .match_indices('\n')
                .map(|(newline_offset, _)| newline_offset + 1),
        )
        .collect()
}

fn source_line_at<'a>(
    content: &'a str,
    line_starts: &[usize],
    match_start: usize,
) -> (usize, &'a str, usize) {
    let line_index = line_starts
        .partition_point(|line_start| *line_start <= match_start)
        .saturating_sub(1);
    let line_start = line_starts[line_index];
    let line_end = content[line_start..]
        .find('\n')
        .map(|offset| line_start + offset)
        .unwrap_or(content.len());
    (
        line_index + 1,
        &content[line_start..line_end],
        match_start.saturating_sub(line_start),
    )
}

enum LexicalState<'a> {
    Code,
    String(&'a StringSyntax),
    LineComment {
        start: usize,
    },
    BlockComment {
        syntax: &'a BlockCommentSyntax,
        start: usize,
        depth: usize,
    },
}

fn token_at(bytes: &[u8], offset: usize, token: &str) -> bool {
    bytes[offset..].starts_with(token.as_bytes())
}

fn comment_ranges(content: &str, syntax: &CommentSyntax) -> Vec<Range<usize>> {
    let bytes = content.as_bytes();
    let mut ranges = Vec::new();
    let mut state = LexicalState::Code;
    let mut offset = 0;

    while offset < bytes.len() {
        match state {
            LexicalState::Code => {
                if let Some(string) = syntax
                    .strings
                    .iter()
                    .filter(|string| token_at(bytes, offset, &string.start))
                    .max_by_key(|string| string.start.len())
                {
                    offset += string.start.len();
                    state = LexicalState::String(string);
                } else if let Some(block) = syntax
                    .block
                    .iter()
                    .filter(|block| token_at(bytes, offset, &block.start))
                    .max_by_key(|block| block.start.len())
                {
                    let start = offset;
                    offset += block.start.len();
                    state = LexicalState::BlockComment {
                        syntax: block,
                        start,
                        depth: 1,
                    };
                } else if let Some(line) = syntax
                    .line
                    .iter()
                    .filter(|line| token_at(bytes, offset, line))
                    .max_by_key(|line| line.len())
                {
                    let start = offset;
                    offset += line.len();
                    state = LexicalState::LineComment { start };
                } else {
                    offset += 1;
                }
            }
            LexicalState::String(string) => {
                if string
                    .escape
                    .as_deref()
                    .is_some_and(|escape| token_at(bytes, offset, escape))
                {
                    offset += string.escape.as_deref().map_or(0, str::len);
                    offset = (offset + 1).min(bytes.len());
                } else if token_at(bytes, offset, &string.end) {
                    offset += string.end.len();
                    state = LexicalState::Code;
                } else {
                    offset += 1;
                }
            }
            LexicalState::LineComment { start } => {
                if bytes[offset] == b'\n' {
                    ranges.push(start..offset);
                    state = LexicalState::Code;
                }
                offset += 1;
            }
            LexicalState::BlockComment {
                syntax: block,
                start,
                mut depth,
            } => {
                if block.nested && token_at(bytes, offset, &block.start) {
                    depth += 1;
                    offset += block.start.len();
                    state = LexicalState::BlockComment {
                        syntax: block,
                        start,
                        depth,
                    };
                } else if token_at(bytes, offset, &block.end) {
                    depth -= 1;
                    offset += block.end.len();
                    if depth == 0 {
                        ranges.push(start..offset);
                        state = LexicalState::Code;
                    } else {
                        state = LexicalState::BlockComment {
                            syntax: block,
                            start,
                            depth,
                        };
                    }
                } else {
                    offset += 1;
                    state = LexicalState::BlockComment {
                        syntax: block,
                        start,
                        depth,
                    };
                }
            }
        }
    }

    match state {
        LexicalState::LineComment { start } | LexicalState::BlockComment { start, .. } => {
            ranges.push(start..bytes.len())
        }
        LexicalState::Code | LexicalState::String(_) => {}
    }
    ranges
}

fn is_comment_offset(ranges: &[Range<usize>], offset: usize) -> bool {
    let index = ranges.partition_point(|range| range.start <= offset);
    index > 0 && ranges[index - 1].contains(&offset)
}

pub(super) fn is_allowlisted(
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

pub(super) fn scan_files(
    project_root: &Path,
    root_paths: &[PathBuf],
    exclude_dirs: &[PathBuf],
    rule: &HardRule,
    engine: &EngineConfig,
) -> Result<Vec<Violation>> {
    if root_paths.is_empty() || rule.patterns.is_empty() {
        return Ok(Vec::new());
    }

    let regexes = compile_regexes(&rule.patterns)?;
    let exclude_regexes = compile_regexes(&rule.exclude_patterns)?;

    let rule_name = rule.name.clone();
    let mut walk_builder = WalkBuilder::new(root_paths[0].clone());
    for root_path in root_paths.iter().skip(1) {
        walk_builder.add(root_path);
    }
    // `ignore` owns traversal and custom ignore-file semantics; audit only
    // supplies the configured filename and evaluates returned files.
    let entries = walk_builder
        .add_custom_ignore_filename(&engine.ignore_filename)
        .follow_links(false)
        .build()
        .collect::<std::result::Result<Vec<_>, ignore::Error>>()?;
    let violations = entries
        .into_par_iter()
        .filter(|entry| {
            let path = entry.path();
            if path.is_dir() {
                return false;
            }
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if !rule.extensions.contains(&ext.to_string()) {
                    return false;
                }
            } else {
                return false;
            }
            for excl in exclude_dirs {
                if path.starts_with(excl) {
                    return false;
                }
            }
            let relative = path.strip_prefix(project_root).unwrap_or(path);
            let path_str = relative.to_str().unwrap_or("");
            if exclude_regexes.iter().any(|re| re.is_match(path_str)) {
                return false;
            }
            !is_allowlisted(path, project_root, &rule.allowlist)
        })
        .map(|entry| -> Result<Vec<Violation>> {
            let path = entry.path();
            let content = fs::read_to_string(path)
                .with_context(|| format!("read audit source {}", path.display()))?;
            let mut violations = Vec::new();
            let mut reported_lines = HashSet::new();
            let line_starts = source_line_starts(&content);
            let extension = path.extension().and_then(|value| value.to_str());
            let comments = extension
                .and_then(|extension| engine.comment_syntax.get(extension))
                .map(|syntax| comment_ranges(&content, syntax))
                .unwrap_or_default();

            for (idx, re) in regexes.iter().enumerate() {
                for matched in re.find_iter(&content) {
                    let (line_number, line, _) =
                        source_line_at(&content, &line_starts, matched.start());
                    if is_comment_offset(&comments, matched.start())
                        || !reported_lines.insert(line_number)
                    {
                        continue;
                    }
                    violations.push(Violation {
                        file: path
                            .strip_prefix(project_root)
                            .unwrap_or(path)
                            .to_path_buf(),
                        line: line_number,
                        content: line.trim().to_string(),
                        rule_name: format!("{}:{}", rule_name, rule.patterns[idx]),
                    });
                }
            }
            violations.sort_by_key(|violation| violation.line);
            Ok(violations)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(violations.into_iter().flatten().collect())
}

pub(super) fn scan_arch_rules(
    project_root: &Path,
    config: &Config,
    exclude_dirs: &[PathBuf],
) -> Result<Vec<ArchViolation>> {
    let mut all_violations = Vec::new();

    for rule in &config.arch_rules {
        let root_paths =
            resolve_rule_roots(project_root, &rule.paths, &config.paths.aliases, &rule.name)?;
        let extensions = rule.extensions.clone();
        let patterns = rule.forbidden_patterns.clone();
        let allowed_patterns = rule.allowed_patterns.clone();
        let exclude_patterns = rule.exclude_patterns.clone();
        let allowlist = rule.allowlist.clone();

        if patterns.is_empty() {
            continue;
        }

        let regexes = compile_regexes(&patterns)?;
        let allowed_regexes = compile_regexes(&allowed_patterns)?;
        let exclude_regexes = compile_regexes(&exclude_patterns)?;

        let rule_name = rule.name.clone();
        let mut walk_builder = WalkBuilder::new(root_paths[0].clone());
        for root_path in root_paths.iter().skip(1) {
            walk_builder.add(root_path);
        }
        // Keep ignore-file traversal in the audit boundary so rule scanning
        // cannot accidentally bypass repository exclusions.
        let entries = walk_builder
            .add_custom_ignore_filename(&config.engine.ignore_filename)
            .follow_links(false)
            .build()
            .collect::<std::result::Result<Vec<_>, ignore::Error>>()?;
        let rule_violations = entries
            .into_par_iter()
            .filter(|entry| {
                let path = entry.path();
                if path.is_dir() {
                    return false;
                }
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if !extensions.contains(&ext.to_string()) {
                        return false;
                    }
                } else {
                    return false;
                }
                for excl in exclude_dirs {
                    if path.starts_with(excl) {
                        return false;
                    }
                }
                let relative = path.strip_prefix(project_root).unwrap_or(path);
                let path_str = relative.to_str().unwrap_or("");
                if exclude_regexes.iter().any(|re| re.is_match(path_str)) {
                    return false;
                }
                !is_allowlisted(path, project_root, &allowlist)
            })
            .map(|entry| -> Result<Vec<ArchViolation>> {
                let path = entry.path();
                let content = fs::read_to_string(path)
                    .with_context(|| format!("read audit source {}", path.display()))?;
                let mut violations = Vec::new();
                let mut reported_lines = HashSet::new();
                let line_starts = source_line_starts(&content);
                let extension = path.extension().and_then(|value| value.to_str());
                let comments = extension
                    .and_then(|extension| config.engine.comment_syntax.get(extension))
                    .map(|syntax| comment_ranges(&content, syntax))
                    .unwrap_or_default();

                for re in &regexes {
                    for matched in re.find_iter(&content) {
                        let (line_number, line, _) =
                            source_line_at(&content, &line_starts, matched.start());
                        if is_comment_offset(&comments, matched.start())
                            || allowed_regexes.iter().any(|allowed| allowed.is_match(line))
                            || !reported_lines.insert(line_number)
                        {
                            continue;
                        }
                        violations.push(ArchViolation {
                            file: path
                                .strip_prefix(project_root)
                                .unwrap_or(path)
                                .to_path_buf(),
                            line: line_number,
                            content: line.trim().to_string(),
                            rule_name: rule_name.clone(),
                        });
                    }
                }
                violations.sort_by_key(|violation| violation.line);
                Ok(violations)
            })
            .collect::<Result<Vec<_>>>()?;

        all_violations.extend(rule_violations.into_iter().flatten());
    }

    Ok(all_violations)
}
