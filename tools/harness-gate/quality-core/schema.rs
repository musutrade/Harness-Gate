//! The frozen Python schema walker's subset, with semantic checks in the domains.
use super::{error, json, require, Result};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};

pub(super) static PROJECT: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("schema/project-model.schema.json")).unwrap()
});
pub(super) static EVIDENCE: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("schema/harness-evidence.schema.json")).unwrap()
});
pub(super) static REQUIREMENTS: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("schema/capability-requirements.schema.json")).unwrap()
});
static PATTERNS: LazyLock<Mutex<HashMap<String, regex::Regex>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) fn shape(value: &Value, schema: &Value, definition: Option<&str>) -> Result<()> {
    walk(
        value,
        definition.map_or(schema, |d| &schema["definitions"][d]),
        schema,
        "$",
    )
}

fn walk(v: &Value, s: &Value, root: &Value, path: &str) -> Result<()> {
    if let Some(reference) = s["$ref"].as_str() {
        return walk(
            v,
            root.pointer(reference.trim_start_matches('#'))
                .expect("embedded schema reference"),
            root,
            path,
        );
    }
    let fail = |message: String| error(format!("{path}: {message}"));
    if let Some(kind) = s.get("type") {
        let matches = |kind: &str| match kind {
            "object" => v.is_object(),
            "array" => v.is_array(),
            "string" => v.is_string(),
            "integer" => json::integer(v),
            "boolean" => v.is_boolean(),
            "number" => v.is_number() && !json::integer(v),
            _ => false,
        };
        let valid = kind.as_str().map_or_else(
            || {
                kind.as_array()
                    .is_some_and(|k| k.iter().any(|x| matches(x.as_str().unwrap())))
            },
            matches,
        );
        if !valid {
            return Err(fail(format!("expected type {kind}")));
        }
    }
    if let Some(c) = s.get("const") {
        require(v == c, format!("{path}: expected const {c}"))?;
    }
    if let Some(values) = s["enum"].as_array() {
        require(
            values.contains(v),
            format!("{path}: expected one of {}", s["enum"]),
        )?;
    }
    if let Some(text) = v.as_str() {
        if let Some(pattern) = s["pattern"].as_str() {
            let mut patterns = PATTERNS.lock().unwrap();
            let regex = patterns
                .entry(pattern.to_owned())
                .or_insert_with(|| regex::Regex::new(pattern).expect("embedded schema pattern"));
            // Python's `$` also matches before one final newline. The domain
            // traversal subsequently rejects that control character.
            require(
                regex.is_match(text)
                    || (pattern.ends_with('$')
                        && text.strip_suffix('\n').is_some_and(|s| regex.is_match(s))),
                format!("{path}: value does not match pattern {pattern}"),
            )?;
        }
        for (key, ok) in [("minLength", true), ("maxLength", false)] {
            if let Some(limit) = s[key].as_u64() {
                require(
                    if ok {
                        text.chars().count() as u64 >= limit
                    } else {
                        text.chars().count() as u64 <= limit
                    },
                    format!("{path}: string violates {key}"),
                )?;
            }
        }
    }
    if let Some(minimum) = s.get("minimum") {
        require(
            json::integer(v)
                && !v.to_string().starts_with('-')
                && json::integer_cmp(v, minimum).is_ge(),
            format!("{path}: integer is below minimum {minimum}"),
        )?;
    }
    if let Some(object) = v.as_object() {
        if let Some(required) = s["required"].as_array() {
            for key in required {
                require(
                    object.contains_key(key.as_str().unwrap()),
                    format!("{path}: missing required field: {key}"),
                )?;
            }
        }
        if let Some(properties) = s["properties"].as_object() {
            for (key, child) in properties {
                if let Some(value) = object.get(key) {
                    walk(value, child, root, &format!("{path}.{key}"))?;
                }
            }
        }
        if s["additionalProperties"] == false {
            for key in object.keys() {
                require(
                    s["properties"].get(key).is_some(),
                    format!("{path}: unknown field: {key}"),
                )?;
            }
        }
    }
    if let Some(values) = v.as_array() {
        if let Some(items) = s.get("items") {
            for (i, value) in values.iter().enumerate() {
                walk(value, items, root, &format!("{path}[{i}]"))?;
            }
        }
        for (key, lower) in [("minItems", true), ("maxItems", false)] {
            if let Some(limit) = s[key].as_u64() {
                require(
                    if lower {
                        values.len() as u64 >= limit
                    } else {
                        values.len() as u64 <= limit
                    },
                    format!("{path}: array violates {key}"),
                )?;
            }
        }
    }
    // Value.oneOf is intentionally resolved by the explicit metric discriminator,
    // as in the frozen reference. Metadata gets its own project traversal.
    Ok(())
}
