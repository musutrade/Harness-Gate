//! Shared redaction for every text boundary that can leave an invocation.

use regex::Regex;
use std::sync::OnceLock;

pub(crate) const REDACTION_TEXT_LIMIT: usize = 16 * 1024 * 1024;

/// Replace credential-bearing values while preserving enough surrounding
/// context for a human to identify the failing rule or operation.
pub(crate) fn redact_text(input: &str) -> String {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        vec![
            Regex::new(r"(?is)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----.*?-----END [A-Z0-9 ]*PRIVATE KEY-----")
                .expect("private key redaction regex"),
            Regex::new(r"(?im)^(?:authorization|proxy-authorization|cookie|set-cookie|x-api-key|x-auth-token)\s*:[^\r\n]*$")
                .expect("header redaction regex"),
            Regex::new(r#"(?i)\b(?:postgres(?:ql)?|mysql|redis|mongodb(?:\+srv)?)://[^\s<>'\"]+"#)
                .expect("connection string redaction regex"),
            Regex::new(r"(?i)\b(?:bearer|basic)\s+[A-Za-z0-9._~+/=-]+")
                .expect("authorization redaction regex"),
            Regex::new(r#"(?i)\"[^\"]*(?:api[_-]?key|access[_-]?token|refresh[_-]?token|id[_-]?token|token|password|passwd|secret|client[_-]?secret)[^\"]*\"\s*:\s*\"[^\"]*\""#)
                .expect("json secret redaction regex"),
            Regex::new(r##"(?i)\b(?:api[_-]?key|access[_-]?token|refresh[_-]?token|id[_-]?token|token|password|passwd|secret|client[_-]?secret)\b\s*[:=]\s*["']?[^\s"'`,;}]+"##)
                .expect("assignment redaction regex"),
        ]
    });
    patterns.iter().fold(input.to_string(), |text, pattern| {
        pattern.replace_all(&text, "[REDACTED]").into_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::redact_text;

    #[test]
    fn removes_common_credential_shapes() {
        let text = concat!(
            "Authorization: Bearer opaque-token\n",
            "cookie: session=opaque-cookie\n",
            "DATABASE_URL=postgres://user:opaque-pass@db.example.test/app\n",
            "password=opaque-value\n",
            "{\"api_key\":\"opaque-json\"}\n",
            "-----BEGIN PRIVATE KEY-----\nopaque-material\n-----END PRIVATE KEY-----"
        );
        let redacted = redact_text(text);
        for secret in [
            "opaque-token",
            "opaque-cookie",
            "opaque-pass@db.example.test",
            "opaque-value",
            "opaque-json",
            "opaque-material",
        ] {
            assert!(!redacted.contains(secret), "secret leaked: {secret}");
        }
        assert!(redacted.contains("[REDACTED]"));
    }
}
