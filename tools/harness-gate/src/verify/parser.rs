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
            let mut count = 0;
            for pattern in patterns {
                let regex = Regex::new(pattern)?;
                count += regex
                    .captures_iter(&normalized)
                    .filter_map(|captures| captures.get(*capture)?.as_str().parse::<usize>().ok())
                    .sum::<usize>();
            }
            Ok((count, *minimum))
        }
    }
}
