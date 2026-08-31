use crate::config::ParserConfig;
use anyhow::Result;
use quick_xml::events::Event;
use quick_xml::Reader;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ShardResult {
    pub shard_index: u32,
    pub shard_total: u32,
    pub test_ids: Vec<String>,
}

/// Validate and merge a complete shard set. The merge identity is the stable
/// test ID rather than completion order, so duplicate or missing shards fail
/// closed instead of silently changing the gate count.
pub(crate) fn merge_shards(results: &[ShardResult], expected_total: u32) -> Result<Vec<String>> {
    if expected_total == 0 {
        return Err(anyhow::anyhow!("shard total must be greater than zero"));
    }
    if results.len() != expected_total as usize {
        return Err(anyhow::anyhow!(
            "missing shard: expected {expected_total}, received {}",
            results.len()
        ));
    }
    let mut seen_shards = std::collections::BTreeSet::new();
    let mut seen_tests = std::collections::BTreeSet::new();
    for result in results {
        if result.shard_total != expected_total {
            return Err(anyhow::anyhow!(
                "shard {} declares total {}, expected {expected_total}",
                result.shard_index,
                result.shard_total
            ));
        }
        if result.shard_index >= expected_total || !seen_shards.insert(result.shard_index) {
            return Err(anyhow::anyhow!(
                "duplicate or out-of-range shard {}",
                result.shard_index
            ));
        }
        for test_id in &result.test_ids {
            if !seen_tests.insert(test_id.clone()) {
                return Err(anyhow::anyhow!(
                    "duplicate test identity {test_id:?} across shards"
                ));
            }
        }
    }
    if seen_shards.len() != expected_total as usize {
        return Err(anyhow::anyhow!("missing shard in declared set"));
    }
    Ok(seen_tests.into_iter().collect())
}

pub(super) fn parse_result_count(content: &str, parser: &ParserConfig) -> Result<(usize, usize)> {
    let ansi = Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]")?;
    let normalized = ansi.replace_all(content, "");
    match parser {
        ParserConfig::Regex {
            patterns,
            capture,
            minimum,
        } => {
            let regexes = patterns
                .iter()
                .map(|pattern| Regex::new(pattern))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let mut count = 0;
            for regex in regexes {
                count += regex
                    .captures_iter(&normalized)
                    .filter_map(|captures| captures.get(*capture)?.as_str().parse::<usize>().ok())
                    .sum::<usize>();
            }
            Ok((count, *minimum))
        }
        ParserConfig::Junit { minimum } => Ok((
            count_xml_elements(normalized.as_ref(), b"testcase")?,
            *minimum,
        )),
        ParserConfig::Trx { minimum } => Ok((
            count_xml_elements(normalized.as_ref(), b"UnitTestResult")?,
            *minimum,
        )),
        ParserConfig::Json {
            count_path,
            minimum,
        } => {
            let value: Value = serde_json::from_str(normalized.as_ref())
                .map_err(|error| anyhow::anyhow!("parse JSON test results: {error}"))?;
            let count = count_json_results(&value, count_path.as_deref())?;
            Ok((count, *minimum))
        }
    }
}

fn count_xml_elements(content: &str, wanted: &[u8]) -> Result<usize> {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);
    let mut count = 0;
    let mut stack = Vec::<Vec<u8>>::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                if event.name().as_ref() == wanted {
                    count += 1;
                }
                stack.push(event.name().as_ref().to_vec());
            }
            Ok(Event::Empty(event)) if event.name().as_ref() == wanted => count += 1,
            Ok(Event::Empty(_)) => {}
            Ok(Event::End(event)) => {
                let Some(open) = stack.pop() else {
                    return Err(anyhow::anyhow!(
                        "parse XML test results: unexpected closing tag"
                    ));
                };
                if open != event.name().as_ref() {
                    return Err(anyhow::anyhow!(
                        "parse XML test results: mismatched closing tag"
                    ));
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(anyhow::anyhow!("parse XML test results: {error}")),
        }
    }
    if !stack.is_empty() {
        return Err(anyhow::anyhow!("parse XML test results: unclosed tag"));
    }
    Ok(count)
}

fn count_json_results(value: &Value, path: Option<&str>) -> Result<usize> {
    if let Some(path) = path {
        let mut current = value;
        for segment in path.split('.') {
            current = current
                .get(segment)
                .ok_or_else(|| anyhow::anyhow!("JSON result path {path:?} is missing"))?;
        }
        return match current {
            Value::Array(items) => Ok(items.len()),
            Value::Number(number) => number
                .as_u64()
                .map(|count| count as usize)
                .ok_or_else(|| anyhow::anyhow!("JSON result count at {path:?} is not an integer")),
            _ => Err(anyhow::anyhow!(
                "JSON result path {path:?} must be an array or integer"
            )),
        };
    }

    fn discover(value: &Value) -> Option<usize> {
        match value {
            Value::Object(map) => {
                for key in [
                    "testcases",
                    "testCases",
                    "test_results",
                    "testResults",
                    "results",
                ] {
                    if let Some(candidate) = map.get(key) {
                        if let Some(count) = discover(candidate) {
                            return Some(count);
                        }
                    }
                }
                map.values().find_map(discover)
            }
            Value::Array(items) => Some(items.len()),
            Value::Number(number) => number.as_u64().map(|count| count as usize),
            _ => None,
        }
    }

    discover(value).ok_or_else(|| anyhow::anyhow!("JSON test results contain no countable results"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_junit_testcases() {
        let parser = ParserConfig::Junit { minimum: 2 };
        assert_eq!(
            parse_result_count("<testsuite><testcase/><testcase/></testsuite>", &parser).unwrap(),
            (2, 2)
        );
    }

    #[test]
    fn parses_trx_results() {
        let parser = ParserConfig::Trx { minimum: 1 };
        assert_eq!(
            parse_result_count("<TestRun><UnitTestResult testId=\"a\"/></TestRun>", &parser)
                .unwrap(),
            (1, 1)
        );
    }

    #[test]
    fn parses_json_result_path() {
        let parser = ParserConfig::Json {
            count_path: Some("summary.total".into()),
            minimum: 3,
        };
        assert_eq!(
            parse_result_count(r#"{"summary":{"total":3}}"#, &parser).unwrap(),
            (3, 3)
        );
    }

    #[test]
    fn malformed_standard_results_fail_closed() {
        let parser = ParserConfig::Junit { minimum: 1 };
        assert!(parse_result_count("<testsuite><testcase>", &parser).is_err());
    }

    #[test]
    fn shard_merge_rejects_missing_and_duplicate_inputs() {
        let first = ShardResult {
            shard_index: 0,
            shard_total: 2,
            test_ids: vec!["a".into()],
        };
        assert!(merge_shards(std::slice::from_ref(&first), 2)
            .unwrap_err()
            .to_string()
            .contains("missing shard"));
        let duplicate = ShardResult {
            shard_index: 1,
            shard_total: 2,
            test_ids: vec!["a".into()],
        };
        assert!(merge_shards(&[first, duplicate], 2)
            .unwrap_err()
            .to_string()
            .contains("duplicate test identity"));
    }

    #[test]
    fn shard_merge_is_deterministic() {
        let results = vec![
            ShardResult {
                shard_index: 1,
                shard_total: 2,
                test_ids: vec!["b".into()],
            },
            ShardResult {
                shard_index: 0,
                shard_total: 2,
                test_ids: vec!["a".into()],
            },
        ];
        assert_eq!(merge_shards(&results, 2).unwrap(), vec!["a", "b"]);
    }
}
