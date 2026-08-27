use super::super::{EngineConfig, HardRule, Violation};
use super::{
    comment_ranges, compile_regexes, is_allowlisted, is_comment_offset, source_line_at,
    source_line_starts,
};
use anyhow::{Context, Result};
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn scan_files(
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
