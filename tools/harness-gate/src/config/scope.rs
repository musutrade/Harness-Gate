use super::model::FlowConfig;
use anyhow::Result;
use globset::{Glob, GlobSetBuilder};
use std::collections::BTreeSet;

impl FlowConfig {
    pub fn classify_paths(&self, paths: &[String]) -> Result<(BTreeSet<String>, Vec<String>)> {
        let matchers = self
            .scope
            .rules
            .iter()
            .map(|rule| {
                let mut builder = GlobSetBuilder::new();
                for pattern in &rule.patterns {
                    builder.add(Glob::new(pattern)?);
                }
                Ok(builder.build()?)
            })
            .collect::<Result<Vec<_>>>()?;
        let mut components = BTreeSet::new();
        let mut unmatched = Vec::new();
        for path in paths {
            let mut matched = false;
            for (rule, matcher) in self.scope.rules.iter().zip(&matchers) {
                if matcher.is_match(path) {
                    matched = true;
                    components.extend(rule.components.iter().cloned());
                }
            }
            if !matched {
                unmatched.push(path.clone());
            }
        }
        Ok((components, unmatched))
    }
}
