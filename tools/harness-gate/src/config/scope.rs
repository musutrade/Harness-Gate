use super::model::FlowConfig;
use anyhow::Result;
use globset::{Glob, GlobSetBuilder};
use std::collections::BTreeSet;

impl FlowConfig {
    pub fn classify_paths(&self, paths: &[String]) -> Result<(BTreeSet<String>, Vec<String>)> {
        let mut components = BTreeSet::new();
        let mut unmatched = Vec::new();
        for path in paths {
            let mut matched = false;
            for rule in &self.scope.rules {
                let mut builder = GlobSetBuilder::new();
                for pattern in &rule.patterns {
                    builder.add(Glob::new(pattern)?);
                }
                let matcher = builder.build()?;
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
