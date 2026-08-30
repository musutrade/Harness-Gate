use crate::config::ParserConfig;
use anyhow::Result;
use regex::Regex;

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
    }
}
