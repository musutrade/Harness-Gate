mod diagnostic;
mod loader;
mod migration;
mod model;
mod path;
mod scope;
mod validation;

#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use diagnostic::{
    report_for_error, ConfigCheckReport, ConfigDiagnostic, ConfigDiagnostics, DiagnosticSeverity,
};
pub use loader::schema_json;
pub use migration::migrate_v1;
#[allow(unused_imports)]
pub use model::{
    DoctorCheck, DoctorCheckKind, DoctorConfig, ExternalValuePolicy, FlowConfig, ParserConfig,
    PathAlias, PathType, PathsConfig, PolicyConfig, ProjectConfig, ReportTemplatesConfig,
    ScopeConfig, ScopeRule, ServiceConfig, StepConfig, UnmatchedScope, CONFIG_VERSION,
    DEFAULT_CONFIG_PATH,
};
pub use path::resolve_config_path;
