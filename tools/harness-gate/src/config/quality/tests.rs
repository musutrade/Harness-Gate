use super::{model::*, *};
use crate::test_support::TestWorkspace;
use std::fs;

const QUALITY: &str = include_str!("../../../../quality/fixtures/workflow/quality.toml");
const POLICY: &str = include_str!("../../../../quality/fixtures/workflow/policy.json");

fn fixture() -> (TestWorkspace, FlowConfig, QualityConfig) {
    let root = TestWorkspace::new("quality-config");
    root.child(".harness-gate");
    fs::write(root.join(".harness-gate/policy.json"), POLICY).unwrap();
    let flow = toml::from_str(include_str!("../../../presets/rust-api.flow.toml")).unwrap();
    (root, flow, toml::from_str(QUALITY).unwrap())
}

fn invalid(change: impl FnOnce(&mut QualityConfig), expected: &str) {
    let (root, flow, mut quality) = fixture();
    change(&mut quality);
    let error = quality.validate(&flow, &root).unwrap_err().to_string();
    assert!(error.contains(expected), "expected {expected}: {error}");
}

#[test]
fn quality_validates_and_round_trips_without_enabling_legacy_flow() {
    let (root, flow, quality) = fixture();
    assert!(QualityConfig::load_optional(&root, &flow)
        .unwrap()
        .is_none());
    quality.validate(&flow, &root).unwrap();
    let source = toml::to_string_pretty(&quality).unwrap();
    fs::write(root.join(QUALITY_CONFIG_PATH), source).unwrap();
    assert!(QualityConfig::load_optional(&root, &flow)
        .unwrap()
        .is_some());
    let schema: serde_json::Value = serde_json::from_str(&schema_json().unwrap()).unwrap();
    let committed: serde_json::Value =
        serde_json::from_str(include_str!("../../../../../schema/quality.schema.json")).unwrap();
    assert_eq!(schema, committed);
}

#[test]
fn quality_rejects_authority_at_every_collector_nesting_level() {
    for field in [
        "required",
        "threshold",
        "thresholds",
        "ratchet",
        "aggregate",
        "pass",
        "fail",
        "release_authority",
        "policy",
    ] {
        for pointer in [
            "/collectors/coverage",
            "/collectors/coverage/produces/0",
            "/collectors/coverage/produces/0/target",
        ] {
            let (_, _, quality) = fixture();
            let mut value = serde_json::to_value(quality).unwrap();
            value
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(field.into(), true.into());
            assert!(
                serde_json::from_value::<QualityConfig>(value).is_err(),
                "{pointer}/{field}"
            );
        }
    }
}

#[test]
fn quality_rejects_unresolved_references_and_boundaries() {
    invalid(|q| q.version = 2, "version");
    invalid(|q| q.project.name = "wrong".into(), "project.name");
    invalid(
        |q| {
            q.components
                .get_mut("app")
                .unwrap()
                .flow_components
                .insert("missing".into())
                .then_some(())
                .unwrap()
        },
        "flow components",
    );
    invalid(
        |q| q.subjects.get_mut("module").unwrap().component = "missing".into(),
        "unknown component",
    );
    invalid(
        |q| {
            q.subjects.get_mut("module").unwrap().selection = SubjectSelection::Explicit {
                paths: vec!["outside/lib.rs".into()],
            }
        },
        "escapes component",
    );
    invalid(
        |q| {
            q.subjects.get_mut("module").unwrap().selection = SubjectSelection::Discovery {
                collector: "missing".into(),
                query: "functions".into(),
            }
        },
        "unknown discovery",
    );
    invalid(
        |q| {
            q.collectors.get_mut("coverage").unwrap().produces[0].target = Target::Subject {
                id: "missing".into(),
            }
        },
        "unresolved quality target",
    );
    invalid(
        |q| {
            q.policies.get_mut("coverage").unwrap().expectation.target = Target::Relationship {
                id: "missing".into(),
            }
        },
        "unresolved quality target",
    );
    invalid(|q| q.reporting.output = "../escape".into(), "escape");
    invalid(
        |q| q.components.get_mut("app").unwrap().artifact_root = "C:\\escape".into(),
        "portable",
    );
    invalid(
        |q| q.collectors.get_mut("coverage").unwrap().request = "${REQUEST}".into(),
        "literal",
    );
}

#[test]
fn quality_profiles_enforce_single_compatible_required_producer() {
    invalid(
        |q| {
            q.profiles.insert("ci".into(), q.profiles["full"].clone());
        },
        "not declared",
    );
    invalid(
        |q| {
            q.profiles.get_mut("full").unwrap().collectors.clear();
        },
        "no producer",
    );
    invalid(
        |q| {
            q.profiles
                .get_mut("full")
                .unwrap()
                .collectors
                .insert("missing".into());
        },
        "unknown collector",
    );
    invalid(
        |q| {
            q.profiles
                .get_mut("full")
                .unwrap()
                .policies
                .insert("missing".into());
        },
        "unknown policy",
    );
    invalid(
        |q| {
            q.collectors
                .insert("duplicate".into(), q.collectors["coverage"].clone());
            q.profiles
                .get_mut("full")
                .unwrap()
                .collectors
                .insert("duplicate".into());
        },
        "conflicting authoritative",
    );
    invalid(
        |q| {
            q.collectors.get_mut("coverage").unwrap().produces[0].series =
                format!("measurement-series/v1:{}", "b".repeat(64))
        },
        "incompatible series",
    );
    invalid(
        |q| q.collectors.get_mut("coverage").unwrap().produces[0].series = "invented".into(),
        "measurement series",
    );
    invalid(
        |q| {
            q.collectors.get_mut("coverage").unwrap().produces[0].capability =
                "coverage..line".into()
        },
        "invalid capability",
    );
    invalid(
        |q| {
            q.profiles
                .get_mut("hook")
                .unwrap()
                .policies
                .insert("coverage".into());
        },
        "no producer",
    );
}

#[test]
fn quality_policy_and_baseline_contradictions_fail_closed() {
    invalid(
        |q| q.policies.get_mut("coverage").unwrap().rule = "missing".into(),
        "unknown policy rule",
    );
    invalid(
        |q| q.policies.get_mut("coverage").unwrap().policy_file = "missing.json".into(),
        "missing",
    );
    invalid(
        |q| {
            q.policies
                .get_mut("coverage")
                .unwrap()
                .expectation
                .capability = "risk.crap".into()
        },
        "capability does not match",
    );
    invalid(
        |q| {
            q.policies.get_mut("coverage").unwrap().expectation.target = Target::Subject {
                id: "module".into(),
            }
        },
        "scope does not match",
    );
    invalid(
        |q| q.baseline.provider = BaselineProvider::None,
        "no provider",
    );
    invalid(|q| q.baseline.required = false, "ratchet policy requires");
    invalid(
        |q| {
            q.baseline.provider = BaselineProvider::Git {
                reference: "".into(),
                merge_base: true,
            }
        },
        "Git reference",
    );
    invalid(
        |q| {
            q.baseline.provider = BaselineProvider::RetainedArtifact {
                manifest: "../stale.json".into(),
            }
        },
        "escape",
    );
    let (root, flow, mut q) = fixture();
    q.baseline.provider = BaselineProvider::RetainedArtifact {
        manifest: "target/base-manifest.json".into(),
    };
    q.validate(&flow, &root).unwrap();
    fs::write(
        root.join(".harness-gate/policy.json"),
        POLICY
            .replace("\"required\": true", "\"required\": false")
            .replace("\"deny_regression\": true", "\"deny_regression\": false")
            .replace(
                "\"allow_legacy_debt\": true",
                "\"allow_legacy_debt\": false",
            ),
    )
    .unwrap();
    q.baseline = Baseline {
        required: false,
        provider: BaselineProvider::None,
    };
    q.profiles.get_mut("full").unwrap().collectors.clear();
    q.validate(&flow, &root).unwrap();
}

#[test]
fn quality_relationships_and_discovery_resolve_in_active_profiles() {
    let (root, flow, mut q) = fixture();
    q.subjects.get_mut("module").unwrap().selection = SubjectSelection::Discovery {
        collector: "coverage".into(),
        query: "modules".into(),
    };
    q.relationships.insert(
        "api".into(),
        Relationship {
            kind: "consumes".into(),
            from: Target::Component { id: "app".into() },
            to: Target::Subject {
                id: "module".into(),
            },
        },
    );
    q.validate(&flow, &root).unwrap();
    q.relationships.get_mut("api").unwrap().to = Target::Subject {
        id: "missing".into(),
    };
    assert!(q
        .validate(&flow, &root)
        .unwrap_err()
        .to_string()
        .contains("unresolved"));
    q.relationships.get_mut("api").unwrap().to = Target::Relationship { id: "api".into() };
    assert!(q
        .validate(&flow, &root)
        .unwrap_err()
        .to_string()
        .contains("endpoints"));
    q.relationships.clear();
    q.collectors
        .insert("discovery".into(), q.collectors["coverage"].clone());
    q.subjects.get_mut("module").unwrap().selection = SubjectSelection::Discovery {
        collector: "discovery".into(),
        query: "modules".into(),
    };
    assert!(q
        .validate(&flow, &root)
        .unwrap_err()
        .to_string()
        .contains("omits discovery"));
}

#[cfg(unix)]
#[test]
fn quality_broken_links_and_boundary_symlinks_are_not_opt_out() {
    let (root, flow, q) = fixture();
    std::os::unix::fs::symlink("missing.toml", root.join(QUALITY_CONFIG_PATH)).unwrap();
    assert!(QualityConfig::load_optional(&root, &flow).is_err());
    let outside = TestWorkspace::new("quality-outside");
    std::os::unix::fs::symlink(&outside.root, root.join("src")).unwrap();
    assert!(q
        .validate(&flow, &root)
        .unwrap_err()
        .to_string()
        .contains("escapes"));
}

#[test]
fn quality_schema_closes_collector_authority_and_tagged_variants() {
    let schema: serde_json::Value = serde_json::from_str(&schema_json().unwrap()).unwrap();
    let definitions = &schema["definitions"];
    for (name, fields) in [
        ("Collector", vec!["produces", "protocol", "request"]),
        ("Expectation", vec!["capability", "series", "target"]),
        ("PolicyBinding", vec!["expectation", "policy_file", "rule"]),
    ] {
        assert_eq!(definitions[name]["additionalProperties"], false);
        let actual: Vec<_> = definitions[name]["properties"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(actual, fields);
    }
    for name in ["Target", "SubjectSelection", "BaselineProvider"] {
        for variant in definitions[name]["oneOf"].as_array().unwrap() {
            assert_eq!(variant["additionalProperties"], false);
        }
    }
}

#[test]
fn quality_accepts_exact_subject_and_relationship_policy_bindings() {
    let (root, flow, mut q) = fixture();
    for target in [
        Target::Subject {
            id: "module".into(),
        },
        Target::Relationship { id: "api".into() },
    ] {
        q.relationships.insert(
            "api".into(),
            Relationship {
                kind: "consumes".into(),
                from: Target::Component { id: "app".into() },
                to: Target::Subject {
                    id: "module".into(),
                },
            },
        );
        let mut policy: serde_json::Value = serde_json::from_str(POLICY).unwrap();
        let rule = &mut policy["rules"][0];
        rule.as_object_mut().unwrap().remove("ratchet");
        rule["metric"] = "contract.schema_valid".into();
        rule["operator"] = "eq".into();
        rule["limit"] = serde_json::json!({"type": "boolean", "value": true});
        q.policies
            .get_mut("coverage")
            .unwrap()
            .expectation
            .capability = "contract.schema_valid".into();
        q.collectors.get_mut("coverage").unwrap().produces[0].capability =
            "contract.schema_valid".into();
        rule["scope"] = match &target {
            Target::Subject { id } => serde_json::json!({"kind": "subject", "subject": id}),
            Target::Relationship { id } => {
                serde_json::json!({"kind": "relationship", "relationship": id})
            }
            _ => unreachable!(),
        };
        fs::write(root.join(".harness-gate/policy.json"), policy.to_string()).unwrap();
        q.policies.get_mut("coverage").unwrap().expectation.target = target.clone();
        q.collectors.get_mut("coverage").unwrap().produces[0].target = target;
        q.validate(&flow, &root).unwrap();
    }
}

#[test]
fn quality_rejects_malformed_policy_and_empty_intent() {
    let (root, flow, q) = fixture();
    for source in [
        "{}".to_owned(),
        POLICY.replace(
            "\"required\": true",
            "\"required\": true, \"required\": false",
        ),
        POLICY.replace("\"schema\":", "\"unexpected\":"),
    ] {
        fs::write(root.join(".harness-gate/policy.json"), source).unwrap();
        assert!(q.validate(&flow, &root).is_err());
    }
    invalid(|q| q.project.id = "Bad ID".into(), "identifier");
    invalid(|q| q.components.clear(), "must not be empty");
    invalid(
        |q| q.components.get_mut("app").unwrap().source_roots.clear(),
        "source roots",
    );
    invalid(
        |q| {
            q.subjects.get_mut("module").unwrap().selection =
                SubjectSelection::Explicit { paths: vec![] }
        },
        "explicit paths",
    );
    invalid(
        |q| {
            q.subjects.get_mut("module").unwrap().selection = SubjectSelection::Discovery {
                collector: "coverage".into(),
                query: " ".into(),
            }
        },
        "query",
    );
    invalid(
        |q| q.collectors.get_mut("coverage").unwrap().produces.clear(),
        "must not be empty",
    );
    invalid(
        |q| {
            let c = q.collectors.get_mut("coverage").unwrap();
            c.produces.push(c.produces[0].clone());
        },
        "duplicate",
    );
    invalid(|q| q.profiles.clear(), "must not be empty");
    invalid(|q| q.reporting.formats.clear(), "must not be empty");
}
