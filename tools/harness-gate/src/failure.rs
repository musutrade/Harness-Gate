use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable machine-facing failure registry. Display text is deliberately kept
/// separate so human wording can evolve without changing retry or reporting
/// behavior.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum FailureCode {
    WebhookDestinationDenied,
    WebhookRedirectDenied,
    LeaseOwnershipUncertain,
    ServiceSetupFailure,
    ServiceLeaseFailure,
    ResultParseFailure,
    ResultZero,
    ResultPartial,
    SchedulerFailure,
    SecretScanFailure,
    ArchitectureAuditFailure,
    StepExecutionFailure,
    StepSkipped,
    OutputLimitExceeded,
    ReaderDeadlineExceeded,
    StepCancelled,
    StepTimeout,
    StepFailed,
    EvidencePathEscape,
    EvidencePending,
    EvidenceFinalizationFailure,
    EvidencePublicationFailure,
    EvidenceDuplicatePath,
    EvidenceStepUnbound,
    EvidenceInvocationMismatch,
    EvidenceMissing,
    EvidenceUndeclaredFile,
    EvidenceSymlink,
    EvidenceInvalidType,
    EvidenceReadFailure,
    EvidenceInvalidMetadata,
}

impl fmt::Display for FailureCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::WebhookDestinationDenied => "WEBHOOK_DESTINATION_DENIED",
            Self::WebhookRedirectDenied => "WEBHOOK_REDIRECT_DENIED",
            Self::LeaseOwnershipUncertain => "LEASE_OWNERSHIP_UNCERTAIN",
            Self::ServiceSetupFailure => "SERVICE_SETUP_FAILURE",
            Self::ServiceLeaseFailure => "SERVICE_LEASE_FAILURE",
            Self::ResultParseFailure => "RESULT_PARSE_FAILURE",
            Self::ResultZero => "RESULT_ZERO",
            Self::ResultPartial => "RESULT_PARTIAL",
            Self::SchedulerFailure => "SCHEDULER_FAILURE",
            Self::SecretScanFailure => "SECRET_SCAN_FAILURE",
            Self::ArchitectureAuditFailure => "ARCHITECTURE_AUDIT_FAILURE",
            Self::StepExecutionFailure => "STEP_EXECUTION_FAILURE",
            Self::StepSkipped => "STEP_SKIPPED",
            Self::OutputLimitExceeded => "OUTPUT_LIMIT_EXCEEDED",
            Self::ReaderDeadlineExceeded => "READER_DEADLINE_EXCEEDED",
            Self::StepCancelled => "STEP_CANCELLED",
            Self::StepTimeout => "STEP_TIMEOUT",
            Self::StepFailed => "STEP_FAILED",
            Self::EvidencePathEscape => "EVIDENCE_PATH_ESCAPE",
            Self::EvidencePending => "EVIDENCE_PENDING",
            Self::EvidenceFinalizationFailure => "EVIDENCE_FINALIZATION_FAILURE",
            Self::EvidencePublicationFailure => "EVIDENCE_PUBLICATION_FAILURE",
            Self::EvidenceDuplicatePath => "EVIDENCE_DUPLICATE_PATH",
            Self::EvidenceStepUnbound => "EVIDENCE_STEP_UNBOUND",
            Self::EvidenceInvocationMismatch => "EVIDENCE_INVOCATION_MISMATCH",
            Self::EvidenceMissing => "EVIDENCE_MISSING",
            Self::EvidenceUndeclaredFile => "EVIDENCE_UNDECLARED_FILE",
            Self::EvidenceSymlink => "EVIDENCE_SYMLINK",
            Self::EvidenceInvalidType => "EVIDENCE_INVALID_TYPE",
            Self::EvidenceReadFailure => "EVIDENCE_READ_FAILURE",
            Self::EvidenceInvalidMetadata => "EVIDENCE_INVALID_METADATA",
        })
    }
}

impl TryFrom<&str> for FailureCode {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            "WEBHOOK_DESTINATION_DENIED" => Self::WebhookDestinationDenied,
            "WEBHOOK_REDIRECT_DENIED" => Self::WebhookRedirectDenied,
            "LEASE_OWNERSHIP_UNCERTAIN" => Self::LeaseOwnershipUncertain,
            "SERVICE_SETUP_FAILURE" => Self::ServiceSetupFailure,
            "SERVICE_LEASE_FAILURE" => Self::ServiceLeaseFailure,
            "RESULT_PARSE_FAILURE" => Self::ResultParseFailure,
            "RESULT_ZERO" => Self::ResultZero,
            "RESULT_PARTIAL" => Self::ResultPartial,
            "SCHEDULER_FAILURE" => Self::SchedulerFailure,
            "SECRET_SCAN_FAILURE" => Self::SecretScanFailure,
            "ARCHITECTURE_AUDIT_FAILURE" => Self::ArchitectureAuditFailure,
            "STEP_EXECUTION_FAILURE" => Self::StepExecutionFailure,
            "STEP_SKIPPED" => Self::StepSkipped,
            "OUTPUT_LIMIT_EXCEEDED" => Self::OutputLimitExceeded,
            "READER_DEADLINE_EXCEEDED" => Self::ReaderDeadlineExceeded,
            "STEP_CANCELLED" => Self::StepCancelled,
            "STEP_TIMEOUT" => Self::StepTimeout,
            "STEP_FAILED" => Self::StepFailed,
            "EVIDENCE_PATH_ESCAPE" => Self::EvidencePathEscape,
            "EVIDENCE_PENDING" => Self::EvidencePending,
            "EVIDENCE_FINALIZATION_FAILURE" => Self::EvidenceFinalizationFailure,
            "EVIDENCE_PUBLICATION_FAILURE" => Self::EvidencePublicationFailure,
            "EVIDENCE_DUPLICATE_PATH" => Self::EvidenceDuplicatePath,
            "EVIDENCE_STEP_UNBOUND" => Self::EvidenceStepUnbound,
            "EVIDENCE_INVOCATION_MISMATCH" => Self::EvidenceInvocationMismatch,
            "EVIDENCE_MISSING" => Self::EvidenceMissing,
            "EVIDENCE_UNDECLARED_FILE" => Self::EvidenceUndeclaredFile,
            "EVIDENCE_SYMLINK" => Self::EvidenceSymlink,
            "EVIDENCE_INVALID_TYPE" => Self::EvidenceInvalidType,
            "EVIDENCE_READ_FAILURE" => Self::EvidenceReadFailure,
            "EVIDENCE_INVALID_METADATA" => Self::EvidenceInvalidMetadata,
            _ => return Err(()),
        })
    }
}

/// Retry classes are a closed configuration vocabulary. Serde keeps the
/// existing lowercase TOML/JSON spelling while rejecting unknown classes.
#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(rename_all = "lowercase")]
pub(crate) enum RetryClass {
    Cancelled,
    Timeout,
    Parser,
    Exit,
}

impl fmt::Display for RetryClass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Cancelled => "cancelled",
            Self::Timeout => "timeout",
            Self::Parser => "parser",
            Self::Exit => "exit",
        })
    }
}

impl TryFrom<&str> for RetryClass {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "cancelled" => Ok(Self::Cancelled),
            "timeout" => Ok(Self::Timeout),
            "parser" => Ok(Self::Parser),
            "exit" => Ok(Self::Exit),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FailureCode, RetryClass};

    #[test]
    fn failure_code_wire_names_are_stable_and_closed() {
        let code = FailureCode::EvidenceInvalidMetadata;
        assert_eq!(code.to_string(), "EVIDENCE_INVALID_METADATA");
        assert_eq!(FailureCode::try_from(code.to_string().as_str()), Ok(code));
        assert!(FailureCode::try_from("future-code").is_err());
    }

    #[test]
    fn retry_class_wire_names_are_stable_and_closed() {
        assert_eq!(RetryClass::try_from("timeout"), Ok(RetryClass::Timeout));
        assert!(RetryClass::try_from("future-retry").is_err());
    }
}
