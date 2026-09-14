//! Bounded LLVM line semantics for a single certified code-only function.
//! Nested counters replace enclosing counters; another function never supplies
//! a line count. LLVM 22.1.8 LineCoverageStats defines the wrapped/entry maximum.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

type Position = (u64, u64);
type Region = (Position, Position, u64);
pub type Lines = BTreeMap<u64, u64>;

fn coordinate(v: &Value) -> Result<u64> {
    v.as_u64().context("invalid LLVM line coordinate/count")
}

pub fn owner(regions: &[Value], source: &str) -> Result<Lines> {
    let source_lines: Vec<_> = source.split('\n').collect();
    let mut parsed = Vec::new();
    for region in regions {
        let start = (coordinate(&region[0])?, coordinate(&region[1])?);
        let end = (coordinate(&region[2])?, coordinate(&region[3])?);
        for (line, column) in [start, end] {
            ensure!(
                line > 0
                    && column > 0
                    && source_lines
                        .get((line - 1) as usize)
                        .is_some_and(|text| column <= text.len() as u64 + 1),
                "LLVM line region lies outside authenticated source"
            );
        }
        ensure!(start < end, "zero-width LLVM line region is uncertified");
        ensure!(
            region[5] == 0 && region[6] == 0 && region[7] == 0,
            "LLVM line coverage requires single-file code regions"
        );
        parsed.push((start, end, coordinate(&region[4])?));
    }
    let mut ordered = parsed.clone();
    ordered.sort_by_key(|r| (r.0, std::cmp::Reverse(r.1)));
    let mut stack: Vec<Region> = Vec::new();
    for region in &ordered {
        while stack.last().is_some_and(|last| last.1 <= region.0) {
            stack.pop();
        }
        ensure!(
            stack.last().is_none_or(|last| region.1 <= last.1),
            "crossing LLVM line regions are uncertified"
        );
        stack.push(*region);
    }
    let points: BTreeSet<_> = parsed.iter().flat_map(|r| [r.0, r.1]).collect();
    let mut segments = Vec::new();
    for point in points {
        let active = parsed
            .iter()
            .filter(|r| r.0 <= point && point < r.1)
            .max_by_key(|r| (r.0, std::cmp::Reverse(r.1)));
        let entry = parsed.iter().any(|r| r.0 == point);
        let count = active.map_or(0, |r| r.2);
        // LLVM may omit a redundant non-entry segment. Such a segment has no
        // effect on the line iterator, so keep it for the independent sweep.
        segments.push(json!([
            point.0,
            point.1,
            count,
            active.is_some(),
            entry,
            false
        ]));
    }
    from_segments(&segments, source)
}

pub fn from_segments(segments: &[Value], source: &str) -> Result<Lines> {
    let source_lines: Vec<_> = source.split('\n').collect();
    for segment in segments {
        let (line, column) = (coordinate(&segment[0])?, coordinate(&segment[1])?);
        ensure!(
            line > 0
                && column > 0
                && source_lines
                    .get((line - 1) as usize)
                    .is_some_and(|text| column <= text.len() as u64 + 1),
            "LLVM line segment lies outside authenticated source"
        );
    }
    let mut lines = Lines::new();
    let Some(first) = segments.first() else {
        return Ok(lines);
    };
    let end = coordinate(&segments.last().unwrap()[0])?;
    let mut cursor = 0;
    let mut wrapped: Option<&Value> = None;
    for line in coordinate(&first[0])?..=end {
        let begin = cursor;
        while cursor < segments.len() && segments[cursor][0] == line {
            cursor += 1;
        }
        let local = &segments[begin..cursor];
        let entries: Vec<_> = local
            .iter()
            .filter(|s| s[3] == true && s[4] == true && s[5] == false)
            .collect();
        let skipped = local.first().is_some_and(|s| s[3] == false && s[4] == true);
        let mapped = (!skipped && (wrapped.is_some_and(|s| s[3] == true) || !entries.is_empty()))
            || local.iter().any(|s| s[3] == true && s[4] == true);
        if mapped {
            let mut count = wrapped.map_or(Ok(0), |s| coordinate(&s[2]))?;
            for entry in entries {
                count = count.max(coordinate(&entry[2])?);
            }
            lines.insert(line, count);
        }
        if let Some(last) = local.last() {
            wrapped = Some(last);
        }
    }
    Ok(lines)
}

pub fn ratio(lines: &Lines) -> Value {
    json!({"type":"ratio", "covered":lines.values().filter(|n| **n > 0).count(), "total":lines.len()})
}

pub fn merge(target: &mut Lines, source: &Lines) {
    for (line, count) in source {
        target
            .entry(*line)
            .and_modify(|old| *old = (*old).max(*count))
            .or_insert(*count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_zero_and_resumed_parent_use_llvm_line_semantics() {
        let source = "fn f() {\n  if x {\n    a();\n  }\n  b();\n}\n";
        let regions = vec![
            json!([1, 1, 6, 2, 1, 0, 0, 0]),
            json!([2, 8, 4, 4, 0, 0, 0, 0]),
        ];
        let lines = owner(&regions, source).unwrap();
        assert_eq!(
            lines,
            BTreeMap::from([(1, 1), (2, 1), (3, 0), (4, 0), (5, 1), (6, 1)])
        );
        assert_eq!(ratio(&lines), json!({"type":"ratio","covered":4,"total":6}));
    }

    #[test]
    fn invalid_coordinates_crossing_and_empty_regions_fail() {
        assert!(from_segments(&[json!([u32::MAX, 1, 0, true, true, false])], "x").is_err());
        for regions in [
            vec![json!([1, 1, 1, 1, 0, 0, 0, 0])],
            vec![json!([1, 1, 9, 1, 0, 0, 0, 0])],
            vec![json!([1, 1, 1, 99, 0, 0, 0, 0])],
            vec![
                json!([1, 1, 2, 3, 1, 0, 0, 0]),
                json!([2, 1, 3, 2, 1, 0, 0, 0]),
            ],
        ] {
            assert!(owner(&regions, "abc\ndef\nghi").is_err());
        }
    }

    #[test]
    fn functions_on_one_line_keep_independent_counts() {
        let source = "fn a() {} fn b() {}";
        let a = owner(&[json!([1, 1, 1, 10, 1, 0, 0, 0])], source).unwrap();
        let b = owner(&[json!([1, 11, 1, 19, 0, 0, 0, 0])], source).unwrap();
        assert_eq!(ratio(&a), json!({"type":"ratio","covered":1,"total":1}));
        assert_eq!(ratio(&b), json!({"type":"ratio","covered":0,"total":1}));
    }
}
