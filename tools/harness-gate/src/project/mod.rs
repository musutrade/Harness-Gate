mod discovery;
mod paths;
mod runtime;
#[cfg(test)]
mod tests;

use crate::config::FlowConfig;
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
    aliases: BTreeMap<String, PathBuf>,
}
