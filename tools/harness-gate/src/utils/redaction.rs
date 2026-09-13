//! Shared redaction for every text boundary that can leave an invocation.

use anyhow::{bail, Result};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

pub(crate) const REDACTION_TEXT_LIMIT: usize = 16 * 1024 * 1024;

/// Redact a Core-owned JSON model without treating the entire document as one
/// untrusted text field. Keep JSON structure and numerical evidence intact.
pub(crate) fn redact_json(value: &mut Value) -> Result<()> {
    fn visit(value: &mut Value, secret: bool, depth: usize) -> Result<()> {
        if depth > 128 {
            bail!("JSON redaction depth exceeds limit");
        }
        match value {
            Value::String(text) => {
                if text.len() > REDACTION_TEXT_LIMIT {
                    bail!("JSON text field exceeds redaction limit");
                }
                *text = if secret {
                    "[REDACTED]".into()
                } else {
                    redact_text(text)
                };
            }
            Value::Array(values) => {
                for value in values {
                    visit(value, false, depth + 1)?;
                }
            }
            Value::Object(values) => {
                static KEYS: OnceLock<Regex> = OnceLock::new();
                let keys = KEYS.get_or_init(|| {
                    Regex::new(
                        r"(?i)(?:api[_-]?key|token|password|passwd|secret|authorization|cookie)",
                    )
                    .expect("JSON credential key regex")
                });
                let mut redacted = serde_json::Map::new();
                for (key, mut value) in std::mem::take(values) {
                    if key.len() > REDACTION_TEXT_LIMIT {
                        bail!("JSON key exceeds redaction limit");
                    }
                    visit(&mut value, keys.is_match(&key), depth + 1)?;
                    if redacted.insert(redact_text(&key), value).is_some() {
                        bail!("JSON redaction produced duplicate keys");
                    }
                }
                *values = redacted;
            }
            _ => {}
        }
        Ok(())
    }
    visit(value, false, 0)
}

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
    fn json_redaction_preserves_structure_and_decodes_escaped_values() {
        let mut value = serde_json::json!({
            "api_token": "opaque-json-secret", "count": 1778,
            "nested": [{"enabled": true, "missing": null,
                "message": "-----BEGIN PRIVATE KEY-----\nopaque-key\n-----END PRIVATE KEY-----"}],
            "token-policy": {"state": "fail", "limit": 30},
            "url": "postgres://user:opaque-password@db.example.test/app"
        });
        super::redact_json(&mut value).unwrap();
        assert_eq!(value["api_token"], "[REDACTED]");
        assert_eq!(value["count"], 1778);
        assert_eq!(value["token-policy"]["state"], "fail");
        assert_eq!(value["nested"][0]["enabled"], true);
        assert!(value["nested"][0]["missing"].is_null());
        let encoded = serde_json::to_string(&value).unwrap();
        for secret in ["opaque-json-secret", "opaque-key", "opaque-password"] {
            assert!(!encoded.contains(secret));
        }
        assert!(serde_json::from_str::<serde_json::Value>(&encoded).is_ok());
    }

    #[test]
    fn json_redaction_rejects_oversized_scalars_keys_depth_and_key_collisions() {
        let oversized = "x".repeat(super::REDACTION_TEXT_LIMIT + 1);
        let mut scalar = serde_json::Value::String(oversized.clone());
        assert!(super::redact_json(&mut scalar).is_err());
        let mut key = serde_json::json!({oversized: 1});
        assert!(super::redact_json(&mut key).is_err());
        let mut deep = serde_json::Value::Null;
        for _ in 0..130 {
            deep = serde_json::json!([deep]);
        }
        assert!(super::redact_json(&mut deep).is_err());
        let mut collisions = serde_json::json!({"password=one": 1, "password=two": 2});
        assert!(super::redact_json(&mut collisions).is_err());
    }

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
