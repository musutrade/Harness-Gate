//! Bounded experiment: one dependency-free generator that emits Rust source text
//! shared by a build-time writer and a source observer.
//!
//! Unlike a proc-macro token stream, the emitted text is written to a real file
//! and pulled in with `include!`, so stable source coverage can own each
//! generated function. Only two fixed templates are supported; this is not a
//! general expander and does not certify the token-stream macro path.

/// The one function this generator can emit, plus its source-level cyclomatic
/// count (1 plus the number of `if` decisions in the template).
pub struct Generated {
    pub owner: String,
    pub cyclomatic: u64,
    pub source: String,
}

// A single `{name}` placeholder is substituted; literal braces are doubled.
const IDENTITY: &str = "pub fn {name}(x: i32) -> i32 {{ x }}";
const SINGLE_IF: &str = "pub fn {name}(x: i32) -> i32 {{ if x > 0 {{ 1 }} else {{ 0 }} }}";

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The build-time writer and the observer use this same function.
pub fn generate(name: &str, branching: bool) -> Option<Generated> {
    if !is_identifier(name) {
        return None;
    }
    let (template, cyclomatic) = if branching {
        (SINGLE_IF, 2)
    } else {
        (IDENTITY, 1)
    };
    Some(Generated {
        owner: name.to_owned(),
        cyclomatic,
        source: template
            .replace("{name}", name)
            .replace("{{", "{")
            .replace("}}", "}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invocation_changes_generated_control_flow() {
        let plain = generate("plain", false).unwrap();
        let branch = generate("branch", true).unwrap();
        assert_eq!((plain.owner.as_str(), plain.cyclomatic), ("plain", 1));
        assert_eq!((branch.owner.as_str(), branch.cyclomatic), ("branch", 2));
        assert_ne!(plain.source, branch.source);
    }

    #[test]
    fn generator_rejects_invalid_identifiers() {
        for name in ["", "1bad", "has space", "nested!()", "a-b"] {
            assert!(generate(name, true).is_none(), "{name}");
        }
    }
}
