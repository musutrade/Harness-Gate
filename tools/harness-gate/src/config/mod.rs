mod loader;
mod migration;
mod model;
mod path;
mod scope;
mod validation;

#[cfg(test)]
mod tests;

pub use migration::migrate_v1;
#[allow(unused_imports)]
pub use model::{
    DoctorCheck, DoctorCheckKind, DoctorConfig, ExternalValuePolicy, FlowConfig, ParserConfig,
    PathAlias, PathType, PathsConfig, PolicyConfig, ProjectConfig, ScopeConfig, ScopeRule,
    ServiceConfig, StepConfig, UnmatchedScope, CONFIG_VERSION, DEFAULT_CONFIG_PATH,
};
pub use path::resolve_config_path;
