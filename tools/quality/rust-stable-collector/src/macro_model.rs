//! Source-level observations for our two explicitly identified macro templates.
//! These are not general expansion, certified execution evidence or Core metrics.
use crate::{artifact, collect, strict_json};
use anyhow::{ensure, Context, Result};
use quote::ToTokens;
use serde_json::{json, Value};
use std::{collections::BTreeSet, fs, path::Path};
use syn::{spanned::Spanned, Attribute, Item};

const SCHEMA: &str = "rust-macro-template-observation/v1-candidate";
const SERIES: &str = "rust-shared-template-cyclomatic/v1-candidate";
const TARGET: &str = "x86_64-unknown-linux-gnu";

// Fail closed on implementation, manifest or dependency version drift. Merely
// having a package with the expected name does not identify its generation path.
const MODEL: &[(&str, &[u8])] = &[
    (
        "Cargo.toml",
        include_bytes!("../../fixtures/rust-macro-observation/Cargo.toml"),
    ),
    (
        "Cargo.lock",
        include_bytes!("../../fixtures/rust-macro-observation/Cargo.lock"),
    ),
    (
        "generator/Cargo.toml",
        include_bytes!("../../fixtures/rust-macro-observation/generator/Cargo.toml"),
    ),
    (
        "generator/src/lib.rs",
        include_bytes!("../../fixtures/rust-macro-observation/generator/src/lib.rs"),
    ),
    (
        "macros/Cargo.toml",
        include_bytes!("../../fixtures/rust-macro-observation/macros/Cargo.toml"),
    ),
    (
        "macros/src/lib.rs",
        include_bytes!("../../fixtures/rust-macro-observation/macros/src/lib.rs"),
    ),
    (
        "consumer/Cargo.toml",
        include_bytes!("../../fixtures/rust-macro-observation/consumer/Cargo.toml"),
    ),
];

fn active(attributes: &[Attribute], branching: bool) -> Option<bool> {
    match attributes {
        [] => Some(true),
        [attribute] => match attribute.meta.to_token_stream().to_string().as_str() {
            "cfg (feature = \"branching\")" => Some(branching),
            "cfg (not (feature = \"branching\"))" => Some(!branching),
            _ => None,
        },
        _ => None,
    }
}

fn functions(text: &str, branching: bool) -> Result<Value> {
    let parsed = syn::parse_file(text)?;
    let mut observations = Vec::new();
    let mut exclusions = Vec::new();
    let mut unsupported = BTreeSet::new();
    let mut owners = BTreeSet::new();
    let mut imported = false;
    if !parsed.attrs.is_empty() {
        unsupported.insert("unvalidated consumer crate attributes");
    }
    for item in parsed.items {
        match item {
            Item::Use(item)
                if item.to_token_stream().to_string()
                    == "use gate_observation_macros :: observed_function ;" =>
            {
                ensure!(!imported, "duplicate observed_function import");
                imported = true;
            }
            Item::Mod(item)
                if item.attrs.len() == 1
                    && item.attrs[0].meta.to_token_stream().to_string() == "cfg (test)"
                    && item.content.is_some() =>
            {
                exclusions
                    .push(json!({"module":item.ident.to_string(), "reason":"test-only module"}));
            }
            Item::Macro(item) if item.mac.path.is_ident("observed_function") => {
                let Some(enabled) = active(&item.attrs, branching) else {
                    unsupported.insert("unvalidated invocation cfg or attributes");
                    continue;
                };
                let model = match gate_observation_generator::expand(item.mac.tokens.clone()) {
                    Ok(value) => value,
                    Err(_) => {
                        unsupported.insert(
                            "input outside shared parser: nested expansion or malformed invocation",
                        );
                        continue;
                    }
                };
                let span = item.mac.span();
                let invocation = json!({
                    "owner":model.owner,
                    "source":"consumer/src/lib.rs",
                    "span":[span.start().line, span.start().column + 1, span.end().line, span.end().column + 1],
                    "input":item.mac.tokens.to_string(),
                    "input_sha256":artifact::digest(item.mac.tokens.to_string().as_bytes()),
                    "attributes":item.attrs.iter().map(|a| a.meta.to_token_stream().to_string()).collect::<Vec<_>>(),
                });
                if !enabled {
                    exclusions.push(json!({"invocation":invocation, "reason":"inactive requested feature configuration"}));
                    continue;
                }
                ensure!(
                    owners.insert(model.owner.clone()),
                    "duplicate active generated owner: {}",
                    model.owner
                );
                observations.push(json!({
                    "invocation":invocation,
                    "generated_tokens_sha256":artifact::digest(model.generated.to_string().as_bytes()),
                    "complexity":{"state":"supported", "series":SERIES, "value":model.cyclomatic,
                        "scope":"one function in a known template; not final compiler expansion"},
                    "coverage":{"state":"unsupported", "reason":"no authenticated generated-function execution mapping"},
                    "crap":{"state":"unsupported", "reason":"no certified coverage alignment or accepted migration"},
                }));
            }
            _ => {
                unsupported.insert("consumer item outside reviewed root invocation model; nested/derive ownership unvalidated");
            }
        }
    }
    if !imported {
        unsupported.insert("shared macro import missing or renamed");
    }
    // Do not preserve apparently successful partial measurements when an
    // unrecognized item could change expansion, configuration or name resolution.
    if !unsupported.is_empty() {
        observations.clear();
    }
    Ok(
        json!({"state":if unsupported.is_empty() {"supported"} else {"unsupported"},
        "functions":observations, "excluded":exclusions, "limitations":unsupported}),
    )
}

pub fn observe(project: &Path, configuration: &str) -> Result<Value> {
    ensure!(
        matches!(configuration, "default" | "branching"),
        "unknown macro model configuration"
    );
    let project = project.canonicalize()?;
    let sources = artifact::inventory(&project, true)?;
    let configuration_files = collect::config_files(&project)?;
    for (path, compiled) in MODEL {
        ensure!(
            sources
                .get(*path)
                .is_some_and(|id| id.sha256 == artifact::digest(compiled)),
            "shared macro model source/version identity mismatch: {path}"
        );
    }
    // No build script, Cargo config or second package source can be silently
    // added to the reviewed model. README changes are harmless but still bound.
    for name in sources.keys() {
        ensure!(
            MODEL.iter().any(|(path, _)| path == name)
                || matches!(name.as_str(), "consumer/src/lib.rs" | "README.md"),
            "unreviewed macro model input: {name}"
        );
    }
    let observed = functions(
        &fs::read_to_string(project.join("consumer/src/lib.rs"))?,
        configuration == "branching",
    )?;
    ensure!(
        sources == artifact::inventory(&project, true)?
            && configuration_files == collect::config_files(&project)?,
        "macro model inputs changed during analysis"
    );
    Ok(json!({
        "schema":SCHEMA, "project_root":project,
        "requested_configuration":{"features":if configuration == "branching" {vec!["branching"]} else {vec![]}, "target":TARGET, "name":configuration},
        "source_files":sources, "cargo_configuration_files":configuration_files,
        "model":{"package":"gate-observation-generator", "version":"0.1.0", "templates":["identity", "single-if"],
            "identity_files":MODEL.iter().map(|(name,_)| name).collect::<Vec<_>>()},
        "analysis":observed,
        "execution":{"state":"unsupported", "reason":"source observation only; no compiler invocation or coverage attestation"},
        "core_acceptance":"pending",
    }))
}

pub fn verify(project: &Path, observation: &Path, anchor: &str) -> Result<()> {
    let bytes = fs::read(observation)?;
    ensure!(
        artifact::digest(&bytes) == anchor,
        "macro observation anchor mismatch"
    );
    let recorded = strict_json::parse(&bytes)?;
    let configuration = recorded["requested_configuration"]["name"]
        .as_str()
        .context("macro configuration name")?;
    ensure!(
        recorded == observe(project, configuration)?,
        "macro observation differs from recomputed source facts"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const IMPORT: &str = "use gate_observation_macros::observed_function;";

    #[test]
    fn same_source_distinct_inputs_and_features_use_shared_generation() {
        let source = format!("{IMPORT} observed_function!(plain, false); observed_function!(never, true); #[cfg(feature=\"branching\")] observed_function!(configured,true); #[cfg(not(feature=\"branching\"))] observed_function!(configured,false);");
        let plain = functions(&source, false).unwrap();
        let branch = functions(&source, true).unwrap();
        assert_eq!(plain["functions"][0]["complexity"]["value"], 1);
        assert_eq!(plain["functions"][1]["complexity"]["value"], 2);
        assert_eq!(plain["functions"][2]["complexity"]["value"], 1);
        assert_eq!(branch["functions"][2]["complexity"]["value"], 2);
        assert_eq!(plain["functions"][1]["coverage"]["state"], "unsupported");
        assert!(plain["functions"][1]["coverage"].get("value").is_none());
    }

    #[test]
    fn duplicates_nested_expansion_and_unvalidated_cfg_do_not_succeed() {
        assert!(functions(
            &format!("{IMPORT} observed_function!(a,true); observed_function!(a,false);"),
            false
        )
        .is_err());
        for source in [
            "observed_function!(a,nested!());",
            "mod nested { observed_function!(a,true); }",
            "#[cfg(unix)] observed_function!(a,true);",
            "#[derive(SomeMacro)] struct X;",
        ] {
            let value = functions(
                &format!("{IMPORT} observed_function!(good,false); {source}"),
                false,
            )
            .unwrap();
            assert_eq!(value["state"], "unsupported", "{source}");
            assert_eq!(value["functions"].as_array().unwrap().len(), 0);
        }
    }
}
