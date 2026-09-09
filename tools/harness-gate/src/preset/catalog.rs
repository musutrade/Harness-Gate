pub(super) struct Preset {
    pub(super) name: &'static str,
    pub(super) description: &'static str,
    pub(super) flow: &'static str,
    pub(super) recipe: Option<&'static str>,
}

pub(super) const AUDIT_TEMPLATE: &str = include_str!("../../presets/empty.audit.toml");
pub(super) const SECRETS_TEMPLATE: &str = include_str!("../../presets/default.secrets.toml");
pub(super) const GITIGNORE_TEMPLATE: &str = "reports/\n";
pub(super) const PRESETS: &[Preset] = &[
    Preset {
        name: "generic",
        description: "Git-based project with a minimal diff check",
        flow: include_str!("../../presets/generic.flow.toml"),
        recipe: None,
    },
    Preset {
        name: "rust-api",
        description: "Single Rust crate with fmt, Clippy, check, and tests",
        flow: include_str!("../../presets/rust-api.flow.toml"),
        recipe: Some(include_str!("../../presets/rust-api.packs.json")),
    },
    Preset {
        name: "angular-only",
        description: "Angular/npm project with lint, tests, and build",
        flow: include_str!("../../presets/angular-only.flow.toml"),
        recipe: Some(include_str!("../../presets/angular-only.packs.json")),
    },
    Preset {
        name: "angular-rust-postgres",
        description: "Angular frontend, Rust backend, and temporary PostgreSQL",
        flow: include_str!("../../presets/angular-rust-postgres.flow.toml"),
        recipe: Some(include_str!(
            "../../presets/angular-rust-postgres.packs.json"
        )),
    },
];

pub fn print_presets() {
    for preset in PRESETS {
        println!("{:<24} {}", preset.name, preset.description);
    }
}

pub(super) fn find(name: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|preset| preset.name == name)
}

pub(super) const PACKS: &[(&str, &str)] = &[
    ("rust", include_str!("../../presets/packs/rust.json")),
    (
        "typescript",
        include_str!("../../presets/packs/typescript.json"),
    ),
    (
        "api-contract",
        include_str!("../../presets/packs/api-contract.json"),
    ),
];
