mod diagnostic;
mod loader;
mod migration;
mod model;
mod path;
mod scope;
mod validation;

#[cfg(test)]
mod tests;

pub(crate) use diagnostic::{report_for_error, ConfigDiagnostics, MINIMAL_CONFIG_SNIPPET};
#[cfg(test)]
pub(crate) use diagnostic::{ConfigDiagnostic, DiagnosticSeverity};
pub use loader::schema_json;
pub use migration::migrate_v1;
pub(crate) use model::{
    ContainerRuntimeKind, DoctorCheck, DoctorCheckKind, ExternalValuePolicy, FlowConfig,
    ParserConfig, PathType, RunnerConfig, RunnerResultFormat, ServiceConfig, StepConfig, StepInput,
    TestIsolation, UnmatchedScope, WaiverConfig, WebhookConfig, CONFIG_VERSION,
    DEFAULT_CONFIG_PATH,
};
pub use path::resolve_config_path;

pub(crate) use scope::CompiledScopeRules;
