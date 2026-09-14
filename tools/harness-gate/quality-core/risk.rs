//! Core-owned migration preview. This never emits evidence or a gate result and
//! cannot satisfy required CRAP, authorize a series, or adopt a baseline.
use super::{array, error, evidence, require, Result};
use serde_json::{json, Value};

const LINE_MODEL: &str = "crap-line-1";

fn metric<'a>(record: &'a Value, name: &str) -> Option<&'a Value> {
    let supported = array(&record["capabilities"])
        .iter()
        .any(|c| c["metric"] == name && c["state"] == "supported");
    supported
        .then(|| {
            array(&record["metrics"])
                .iter()
                .find(|m| m["name"] == name)
                .map(|m| &m["value"])
        })
        .flatten()
}

fn line_score(cc: u64, covered: u64, total: u64) -> Result<Value> {
    require(
        cc > 0 && total > 0 && covered <= total,
        "invalid CRAP line inputs",
    )?;
    let total = u128::from(total);
    let missing = total - u128::from(covered);
    let cc = u128::from(cc);
    let denominator = total
        .checked_pow(3)
        .ok_or_else(|| error("CRAP denominator overflow"))?;
    let numerator = missing
        .checked_pow(3)
        .and_then(|n| n.checked_mul(cc)?.checked_mul(cc))
        .and_then(|n| n.checked_add(cc.checked_mul(denominator)?))
        .ok_or_else(|| error("CRAP numerator overflow"))?;
    let (mut a, mut b) = (numerator, denominator);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    // Core's arbitrary-precision JSON contract preserves exact integers.
    Ok(json!({"type":"rational", "numerator":numerator/a, "denominator":denominator/a}))
}

/// Preview the explicitly selected line model only after validating complete
/// source/artifact/context evidence. Both inputs must belong to the same record,
/// subject, series and artifact set. No file/parent/other-run fallback is allowed.
pub fn preview_line_crap(
    records: &Value,
    context: &evidence::ValidationContext<'_>,
    model: &str,
) -> Result<Value> {
    require(model == LINE_MODEL, "unsupported CRAP preview model")?;
    evidence::validate_evidence(records, context)?;
    let mut rows = Vec::new();
    for record in array(records) {
        let (Some(complexity), Some(lines)) = (
            metric(record, "complexity.cyclomatic"),
            metric(record, "coverage.line"),
        ) else {
            rows.push(
                json!({"subject":record["subject"]["id"], "state":"unsupported",
                "reason":"same-owner complexity and line coverage are required"}),
            );
            continue;
        };
        let integer = |v: &Value| {
            v.as_u64()
                .ok_or_else(|| error("CRAP preview input exceeds u64"))
        };
        let value = line_score(
            integer(&complexity["value"])?,
            integer(&lines["covered"])?,
            integer(&lines["total"])?,
        )?;
        rows.push(
            json!({"subject":record["subject"]["id"], "series":record["series"]["id"],
            "source":record["source"], "context":record["context"], "artifacts":record["artifacts"],
            "state":"review_required", "complexity":complexity, "lines":lines, "value":value}),
        );
    }
    Ok(
        json!({"schema":"core-crap-migration-preview/v1", "model":model,
        "state":"review_required", "authoritative":false, "baseline_adopted":false, "rows":rows}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_scores_preserve_exact_thresholds_and_zero_execution() {
        assert_eq!(
            line_score(3, 5, 7).unwrap(),
            json!({"type":"rational","numerator":1101,"denominator":343})
        );
        assert_eq!(
            line_score(1, 0, 3).unwrap(),
            json!({"type":"rational","numerator":2,"denominator":1})
        );
        assert_eq!(
            line_score(30, 7, 7).unwrap(),
            json!({"type":"rational","numerator":30,"denominator":1})
        );
        let high = line_score(31, 7, 7).unwrap();
        assert!(!super::super::policy::compare(
            &high,
            "le",
            &json!({"type":"rational","numerator":30,"denominator":1})
        )
        .unwrap());
    }

    #[test]
    fn missing_invalid_or_overflowing_inputs_never_become_zero() {
        for inputs in [
            (0, 0, 1),
            (1, 0, 0),
            (1, 2, 1),
            (1, 0, u64::MAX),
            (u64::MAX, 0, 100),
        ] {
            assert!(line_score(inputs.0, inputs.1, inputs.2).is_err());
        }
        assert!(metric(
            &json!({"capabilities":[{"metric":"coverage.line","state":"unsupported"}],
            "metrics":[{"name":"coverage.line","value":{"covered":1,"total":1}}]}),
            "coverage.line"
        )
        .is_none());
    }
}
