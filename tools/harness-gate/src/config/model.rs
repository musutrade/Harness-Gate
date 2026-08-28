use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const CONFIG_VERSION: u32 = 2;
pub const DEFAULT_CONFIG_PATH: &str = ".harness-gate/flow.toml";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FlowConfig {
    pub version: u32,
    pub project: ProjectConfig,
    pub paths: PathsConfig,
    #[serde(default)]
    pub policy: PolicyConfig,
    #[serde(default)]
    pub doctor: DoctorConfig,
    #[serde(default)]
    pub services: BTreeMap<String, ServiceConfig>,
    #[serde(default)]
    pub parsers: BTreeMap<String, ParserConfig>,
    pub scope: ScopeConfig,
    pub steps: Vec<StepConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    pub name: String,
    pub default_profile: String,
    pub hook_profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PathsConfig {
    pub reports: String,
    pub audit_config: String,
    #[serde(default = "default_secrets_config_path")]
    pub secrets_config: String,
    #[serde(default)]
    pub aliases: BTreeMap<String, PathAlias>,
}

fn default_secrets_config_path() -> String {
    ".harness-gate/secrets.toml".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PathAlias {
    pub path: String,
    #[serde(default)]
    pub env: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PolicyConfig {
    #[serde(default)]
    pub required_steps: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DoctorConfig {
    #[serde(default)]
    pub checks: Vec<DoctorCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DoctorCheck {
    pub id: String,
    pub label: String,
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default)]
    pub help: Option<String>,
    #[serde(default = "default_doctor_timeout")]
    pub timeout_secs: u64,
    #[serde(flatten)]
    pub kind: DoctorCheckKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DoctorCheckKind {
    Command {
        program: String,
        #[serde(default)]
        args: Vec<String>,
    },
    Path {
        path: String,
        #[serde(default)]
        path_type: PathType,
    },
    Glob {
        pattern: String,
    },
    Env {
        name: String,
    },
    EnvOrFile {
        env: String,
        path: String,
        contains: String,
    },
    GitConfig {
        key: String,
        expected: String,
    },
    GitRemotes,
    Version {
        program: String,
        #[serde(default)]
        args: Vec<String>,
        path: String,
        #[serde(default)]
        trim_prefix: String,
    },
    Service {
        service: String,
    },
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PathType {
    #[default]
    Any,
    File,
    Directory,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ServiceConfig {
    Docker {
        image: String,
        #[serde(default)]
        image_env: Option<String>,
        #[serde(default)]
        external_env: Option<String>,
        inject_env: String,
        #[serde(default)]
        external_value_policy: ExternalValuePolicy,
        startup_timeout_secs: u64,
        #[serde(default)]
        timeout_env: Option<String>,
        container_port: u16,
        #[serde(default)]
        environment: BTreeMap<String, String>,
        healthcheck: Vec<String>,
        connection: String,
    },
    Environment {
        source_env: String,
        inject_env: String,
    },
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ExternalValuePolicy {
    #[default]
    None,
    IsolatedPostgres,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ParserConfig {
    Regex {
        patterns: Vec<String>,
        #[serde(default = "default_capture")]
        capture: usize,
        #[serde(default = "default_minimum")]
        minimum: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScopeConfig {
    #[serde(default)]
    pub unmatched: UnmatchedScope,
    pub rules: Vec<ScopeRule>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum UnmatchedScope {
    #[default]
    Fail,
    All,
    Ignore,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScopeRule {
    pub patterns: Vec<String>,
    pub components: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StepConfig {
    pub id: String,
    pub label: String,
    pub component: String,
    pub profiles: BTreeSet<String>,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub log: String,
    pub timeout_secs: u64,
    #[serde(default)]
    pub timeout_env: Option<String>,
    #[serde(default)]
    pub parser: Option<String>,
    #[serde(default)]
    pub services: Vec<String>,
    #[serde(default)]
    pub remove_env: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

const fn default_true() -> bool {
    true
}

pub(super) const fn default_doctor_timeout() -> u64 {
    15
}

const fn default_capture() -> usize {
    1
}

const fn default_minimum() -> usize {
    1
}
