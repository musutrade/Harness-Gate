mod discovery;
mod paths;
mod runtime;
#[cfg(test)]
mod tests;

use crate::config::{CompiledScopeRules, FlowConfig};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub(crate) use paths::resolve_repo_path;

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub config_path: PathBuf,
    pub config: FlowConfig,
    pub reports: PathBuf,
    pub audit_config: PathBuf,
    pub secrets_config: PathBuf,
    /// Cross-process resource leases are kept beside the configured report
    /// root, not inside an invocation directory, so concurrent invocations
    /// can observe the same ownership records.
    pub(crate) resource_leases: PathBuf,
    aliases: BTreeMap<String, PathBuf>,
    pub(crate) scope_rules: CompiledScopeRules,
}

impl Project {
    pub(crate) fn invocation_id(&self) -> String {
        let is_invocation = self
            .reports
            .parent()
            .and_then(|parent| parent.file_name())
            .is_some_and(|name| name == "invocations");
        if is_invocation {
            if let Some(name) = self.reports.file_name().and_then(|name| name.to_str()) {
                return name.to_string();
            }
        }
        format!("standalone-{}", std::process::id())
    }
}
