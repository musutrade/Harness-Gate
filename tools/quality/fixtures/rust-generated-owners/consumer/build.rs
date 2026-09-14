//! Writes real Rust source from the shared generator into OUT_DIR. The generated
//! file is ordinary crate source, so stable source coverage can own each function.
use std::env;
use std::fs;
use std::path::PathBuf;

fn invocation(name: &str, branching: bool) -> String {
    gate_generated_owner_generator::generate(name, branching)
        .expect("known identifier must generate")
        .source
}

fn main() {
    // One invocation per output so each generated function keeps a distinct owner.
    let branching = env::var_os("CARGO_FEATURE_BRANCHING").is_some();
    let functions = [
        invocation("plain", false),
        invocation("branch", true),
        invocation("unexecuted", true),
        invocation("configured", branching),
    ];
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("generated_owners.rs");
    fs::write(&out, functions.join("\n") + "\n").unwrap();
    if env::var_os("CARGO_FEATURE_DUPLICATE").is_some() {
        fs::write(
            out.with_file_name("duplicate_owners.rs"),
            functions.join("\n") + "\n",
        )
        .unwrap();
    }
    println!("cargo:rerun-if-changed=build.rs");
}
