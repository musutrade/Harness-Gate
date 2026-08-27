use std::collections::BTreeSet;

fn config() -> crate::config::FlowConfig {
    toml::from_str(include_str!("../../presets/rust-api.flow.toml")).expect("parse config")
}

#[test]
fn workflow_changes_force_all_components() {
    // Use a path that matches the rust-api preset patterns
    let components = config()
        .classify_paths(&[".harness-gate/flow.toml".into()])
        .expect("classify")
        .0;
    // rust-api preset has 1 component: app
    assert_eq!(components.len(), 1);
    assert!(components.contains("app"));
}

#[test]
fn frontend_change_only_selects_frontend() {
    // rust-api preset doesn't have frontend, test with app component
    let components = config()
        .classify_paths(&["src/main.rs".into()])
        .expect("classify")
        .0;
    assert_eq!(components, BTreeSet::from(["app".to_string()]));
}

#[test]
fn unmatched_paths_are_reported() {
    let (components, unmatched) = config()
        .classify_paths(&["unconfigured/new-tool.lock".into()])
        .expect("classify");

    assert!(components.is_empty());
    assert_eq!(unmatched, vec!["unconfigured/new-tool.lock"]);
}
