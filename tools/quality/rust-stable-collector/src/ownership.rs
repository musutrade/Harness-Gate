//! Exact source/LLVM ownership proof for ASCII, unannotated free functions.
//! No demangling, macro expansion, line coverage or parent-count inheritance.
use crate::source;
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeSet, fs, path::Path};

pub const RULE: &str = "rust-llvm-exact-free-owner/v3-candidate";

type Position = (u64, u64);
fn span(value: &Value) -> Result<(Position, Position)> {
    let a = value.as_array().context("owner span missing")?;
    ensure!(a.len() >= 4, "owner span width");
    let n = a[..4]
        .iter()
        .map(|v| v.as_u64().context("owner coordinate"))
        .collect::<Result<Vec<_>>>()?;
    Ok(((n[0], n[1]), (n[2], n[3])))
}
fn contains(outer: (Position, Position), inner: (Position, Position)) -> bool {
    outer.0 <= inner.0 && inner.1 <= outer.1
}

pub fn file(root: &Path, path: &str, raw: &Value) -> Result<Value> {
    let text = fs::read_to_string(root.join(path))?;
    let inventory = serde_json::to_value(source::analyze(&text)?)?;
    let functions = inventory["functions"]
        .as_array()
        .context("source functions")?;
    let unsupported = || json!({"state":"unsupported","reason":"requires ASCII source containing only unannotated, nongeneric free functions at root or in unannotated inline modules; expansion/activation not certified","functions":[]});
    if !text.is_ascii()
        || !inventory["unsupported"]
            .as_array()
            .context("source unsupported")?
            .is_empty()
        || functions.iter().any(|f| f["coverage_eligible"] != true)
    {
        return Ok(unsupported());
    }
    let filename = root.join(path);
    let filename = filename.to_str().context("source path UTF-8")?;
    let raw_functions = raw["data"][0]["functions"]
        .as_array()
        .context("LLVM functions")?;
    let mut assigned = BTreeSet::new();
    let mut symbols = BTreeSet::new();
    let mut mapped = Vec::new();
    let mut excluded = Vec::new();
    let mut file_regions = 0_u64;
    let mut file_covered = 0_u64;
    for (index, record) in raw_functions.iter().enumerate() {
        let files = record["filenames"].as_array().context("LLVM filenames")?;
        if !files.iter().any(|f| f == filename) {
            continue;
        }
        ensure!(files.len() == 1, "ambiguous cross-file LLVM owner");
        ensure!(
            symbols.insert(record["name"].as_str().context("LLVM symbol")?),
            "duplicate LLVM owner symbol"
        );
        let regions = record["regions"].as_array().context("LLVM regions")?;
        let mut envelope = ((u64::MAX, u64::MAX), (0, 0));
        let mut spans = BTreeSet::new();
        let mut covered = 0_u64;
        for region in regions {
            let range = span(region)?;
            ensure!(
                region[5] == 0 && region[6] == 0 && region[7] == 0,
                "unsupported LLVM owner region mapping"
            );
            ensure!(spans.insert(range), "duplicate LLVM owner region span");
            covered += u64::from(region[4].as_u64().context("LLVM region count")? > 0);
            envelope.0 = envelope.0.min(range.0);
            envelope.1 = envelope.1.max(range.1);
        }
        ensure!(!regions.is_empty(), "missing LLVM owner regions");
        let total = u64::try_from(regions.len())?;
        file_regions = file_regions
            .checked_add(total)
            .context("LLVM region total overflow")?;
        file_covered = file_covered
            .checked_add(covered)
            .context("LLVM covered total overflow")?;
        let excluded_matches = inventory["excluded_spans"]
            .as_array()
            .context("excluded spans")?
            .iter()
            .map(span)
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .filter(|s| contains(*s, envelope))
            .count();
        let matches = functions
            .iter()
            .enumerate()
            .filter_map(|(i, f)| (span(&f["span"]).ok() == Some(envelope)).then_some(i))
            .collect::<Vec<_>>();
        ensure!(
            matches.len() + excluded_matches == 1,
            "missing or ambiguous LLVM source owner"
        );
        if excluded_matches == 1 {
            excluded.push(json!({"llvm_function_index":index,"symbol":record["name"],"reason":"inside explicit test source span"}));
            continue;
        }
        let owner = matches[0];
        ensure!(
            assigned.insert(owner),
            "multiple LLVM records for source owner"
        );
        ensure!(
            record["branches"].as_array().is_some_and(Vec::is_empty),
            "unsupported LLVM owner branches"
        );
        let count = record["count"].as_u64().context("LLVM execution count")?;
        ensure!(
            span(&regions[0])?.0 == envelope.0 && regions[0][4] == count,
            "LLVM owner entry count mismatch"
        );
        ensure!(
            count != 0 || regions.iter().all(|r| r[4] == 0),
            "unexecuted LLVM owner has executed regions"
        );
        mapped.push(
            json!({"name":functions[owner]["name"],"span":functions[owner]["span"],
            "llvm_function_index":index,"symbol":record["name"],"execution_count":count,
            "coverage_function":{"type":"ratio","covered":u64::from(count > 0),"total":1},
            "coverage_region":{"type":"ratio","covered":covered,"total":total}}),
        );
    }
    ensure!(
        assigned.len() == functions.len(),
        "source owner missing from LLVM export"
    );
    let files = raw["data"][0]["files"].as_array().context("LLVM files")?;
    let summaries: Vec<_> = files.iter().filter(|f| f["filename"] == filename).collect();
    ensure!(
        summaries.len() == 1,
        "missing or ambiguous LLVM file summary"
    );
    let summary = &summaries[0]["summary"]["regions"];
    ensure!(
        summary["count"] == file_regions && summary["covered"] == file_covered,
        "LLVM owner regions disagree with file summary"
    );
    Ok(json!({"state":"supported","rule":RULE,"functions":mapped,"excluded":excluded}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_sources_do_not_gain_coverage() {
        let dir = tempfile::tempdir().unwrap();
        for source in [
            "async fn f() {}",
            "fn f<T>() {}",
            "#[inline] fn f() {}",
            "fn f() { let x = || 1; }",
            "fn café() {}",
            "#[cfg(feature=\"x\")] mod m { fn f() {} }",
            "#[allow(dead_code)] mod m { fn f() {} }",
            "struct S; impl S { fn f() {} }",
        ] {
            fs::write(dir.path().join("lib.rs"), source).unwrap();
            assert_eq!(
                file(dir.path(), "lib.rs", &json!({})).unwrap()["state"],
                "unsupported"
            );
        }
    }
}
