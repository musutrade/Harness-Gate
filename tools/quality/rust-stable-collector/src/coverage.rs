//! Validate public LLVM JSON before retaining it as raw, test-inclusive evidence.
//! Layout reference: LLVM 22.1.6 CoverageExporterJson.cpp. This is not an owner
//! mapping or a certificate that LLVM counters match source-level functions.
use anyhow::{ensure, Context, Result};
use serde_json::Value;
use std::collections::BTreeSet;

pub fn parse(bytes: &[u8]) -> Result<Value> {
    crate::strict_json::parse_coverage(bytes).context("invalid LLVM JSON")
}

fn array(value: &Value) -> Result<&Vec<Value>> {
    value.as_array().context("LLVM array missing")
}

fn count(value: &Value) -> Result<u64> {
    // The exporter clamps execution counts to INT64_MAX.
    let n = value.as_u64().context("invalid LLVM nonnegative integer")?;
    ensure!(n <= i64::MAX as u64, "LLVM integer exceeds exporter range");
    Ok(n)
}

fn name(value: &Value) -> Result<&str> {
    let text = value.as_str().context("LLVM name missing")?;
    ensure!(
        !text.is_empty() && !text.contains('\0'),
        "invalid LLVM name"
    );
    Ok(text)
}

fn filenames(value: &Value) -> Result<usize> {
    let files = array(value)?;
    ensure!(!files.is_empty(), "empty LLVM filenames");
    for file in files {
        name(file)?;
    }
    Ok(files.len())
}

fn region(value: &Value, files: usize, branch: bool) -> Result<()> {
    let fields = array(value)?;
    ensure!(
        fields.len() == if branch { 9 } else { 8 },
        "invalid LLVM region width"
    );
    let fields = fields.iter().map(count).collect::<Result<Vec<_>>>()?;
    ensure!(
        fields[..4].iter().all(|n| *n <= u32::MAX as u64),
        "LLVM source coordinate overflow"
    );
    ensure!(
        (fields[0], fields[1]) <= (fields[2], fields[3]),
        "reversed LLVM region"
    );
    let file_index = if branch { 6 } else { 5 };
    ensure!(
        fields[file_index] < files as u64 && fields[file_index + 1] < files as u64,
        "LLVM region file ID out of range"
    );
    let kind = fields[file_index + 2];
    ensure!(
        if branch { kind == 4 } else { kind <= 3 },
        "unsupported LLVM region kind: {kind}"
    );
    Ok(())
}

fn branches(value: &Value, files: usize) -> Result<()> {
    for entry in array(value)? {
        region(entry, files, true)?;
    }
    Ok(())
}

fn mcdc(value: &Value) -> Result<()> {
    // No MC/DC producer is part of this stable candidate's verified capability.
    ensure!(array(value)?.is_empty(), "unsupported LLVM MC/DC records");
    Ok(())
}

fn summary(value: &Value) -> Result<()> {
    for key in [
        "lines",
        "functions",
        "instantiations",
        "regions",
        "branches",
        "mcdc",
    ] {
        let row = &value[key];
        let total = count(&row["count"])?;
        let covered = count(&row["covered"])?;
        ensure!(covered <= total, "LLVM {key} covered exceeds count");
        if matches!(key, "regions" | "branches" | "mcdc") {
            ensure!(
                count(&row["notcovered"])? == total - covered,
                "LLVM {key} notcovered mismatch"
            );
        }
        let percent = row["percent"].as_f64().context("LLVM percentage missing")?;
        let expected = if total == 0 {
            0.0
        } else {
            covered as f64 / total as f64 * 100.0
        };
        ensure!(
            percent.is_finite()
                && (0.0..=100.0).contains(&percent)
                && (percent - expected).abs() <= 1e-9,
            "LLVM {key} percentage mismatch"
        );
        if key == "mcdc" {
            ensure!(total == 0, "unsupported LLVM MC/DC summary");
        }
    }
    Ok(())
}

pub fn validate(value: &Value) -> Result<()> {
    ensure!(
        value["type"] == "llvm.coverage.json.export",
        "wrong LLVM JSON type"
    );
    ensure!(
        value["version"] == "3.1.0",
        "unsupported LLVM JSON export version: {}",
        value["version"]
    );
    let data = array(&value["data"])?;
    // The verified cargo-llvm-cov invocation emits one CoverageMapping export.
    ensure!(data.len() == 1, "unsupported LLVM export object count");
    let object = &data[0];
    let files = array(&object["files"])?;
    ensure!(!files.is_empty(), "empty LLVM files");
    let mut names = BTreeSet::new();
    for file in files {
        ensure!(
            names.insert(name(&file["filename"])?),
            "duplicate LLVM file"
        );
        summary(&file["summary"])?;
        let mut previous = None;
        for segment in array(&file["segments"])? {
            let fields = array(segment)?;
            ensure!(
                fields.len() == 6 && fields[3..].iter().all(Value::is_boolean),
                "invalid LLVM segment fields"
            );
            let position = (count(&fields[0])?, count(&fields[1])?);
            ensure!(
                position.0 <= u32::MAX as u64 && position.1 <= u32::MAX as u64,
                "LLVM segment coordinate overflow"
            );
            count(&fields[2])?;
            ensure!(
                previous.is_none_or(|p| p < position),
                "unordered LLVM segments"
            );
            previous = Some(position);
        }
        // File branches may refer to function-local filename tables. Validate
        // their scalar layout here; only function/expansion records bind IDs.
        branches(&file["branches"], u32::MAX as usize)?;
        mcdc(&file["mcdc_records"])?;
        for expansion in array(&file["expansions"])? {
            let size = filenames(&expansion["filenames"])?;
            region(&expansion["source_region"], size, false)?;
            for entry in array(&expansion["target_regions"])? {
                region(entry, size, false)?;
            }
            branches(&expansion["branches"], size)?;
        }
    }
    summary(&object["totals"])?;
    let functions = array(&object["functions"])?;
    ensure!(!functions.is_empty(), "empty LLVM functions");
    for function in functions {
        name(&function["name"])?;
        count(&function["count"])?;
        let size = filenames(&function["filenames"])?;
        let regions = array(&function["regions"])?;
        ensure!(!regions.is_empty(), "empty LLVM function regions");
        for entry in regions {
            region(entry, size, false)?;
        }
        branches(&function["branches"], size)?;
        mcdc(&function["mcdc_records"])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn region_ids_counts_and_order_are_checked() {
        assert!(region(&json!([1, 1, 2, 1, 0, 0, 0, 0]), 1, false).is_ok());
        for row in [
            json!([1, 1, 2, 1, -1, 0, 0, 0]),
            json!([1, 1, 2, 1, 0, 1, 0, 0]),
            json!([2, 1, 1, 1, 0, 0, 0, 0]),
            json!([1, 1, 2, 1, 0, 0, 0, 99]),
        ] {
            assert!(region(&row, 1, false).is_err());
        }
    }
}
