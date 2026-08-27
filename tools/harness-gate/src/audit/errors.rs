use crate::error::CodedError;

/// Errors emitted by the architecture-audit boundary.
#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("audit configuration failed: {message}")]
    Configuration { message: String },
    #[error("audit execution failed: {message}")]
    Execution { message: String },
    #[error("log parsing failed: {message}")]
    LogParsing { message: String },
}

impl AuditError {
    pub(super) fn configuration(error: anyhow::Error) -> Self {
        Self::Configuration {
            message: format!("{error:#}"),
        }
    }
    pub(super) fn execution(error: anyhow::Error) -> Self {
        Self::Execution {
            message: format!("{error:#}"),
        }
    }
    pub(super) fn log_parsing(error: anyhow::Error) -> Self {
        Self::LogParsing {
            message: format!("{error:#}"),
        }
    }
}

impl CodedError for AuditError {
    fn code(&self) -> &'static str {
        match self {
            Self::Configuration { .. } => "E1101",
            Self::Execution { .. } => "E1102",
            Self::LogParsing { .. } => "E1103",
        }
    }
}
