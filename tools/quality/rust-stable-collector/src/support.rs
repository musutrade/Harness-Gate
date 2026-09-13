//! Signed metadata describes a candidate, never authorizes a measurement migration.
use crate::{artifact::FileIdentity, source};
use anyhow::{ensure, Result};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Support {
    schema: String,
    release_version: String,
    target: String,
    program: FileIdentity,
    license: FileIdentity,
    release_status: String,
    complexity_series: String,
    coverage_series: String,
    function_coverage: String,
    function_crap: String,
    build_rust: String,
    observations: Vec<Observation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    rust: String,
    llvm: String,
    cargo_llvm_cov: String,
    target: String,
    acceptance_sha256: String,
}

impl Support {
    pub fn validate(
        &self,
        version: &str,
        target: &str,
        program: &FileIdentity,
        license: &FileIdentity,
    ) -> Result<()> {
        ensure!(
            self.schema == "rust-stable-support/v1",
            "unsupported support schema"
        );
        ensure!(
            self.release_version == version
                && self.target == target
                && self.program == *program
                && self.license == *license,
            "support metadata does not match release identity"
        );
        ensure!(
            self.release_status == "candidate-review-required"
                && self.complexity_series == source::SERIES
                && self.coverage_series == "rust-llvm-source-coverage/v1-candidate"
                && self.function_coverage == "unsupported"
                && self.function_crap == "unsupported",
            "support metadata overstates candidate capabilities or release status"
        );
        ensure!(
            ["1.97.1", "1.98.1"].contains(&self.build_rust.as_str()),
            "unsupported stable build toolchain"
        );
        ensure!(
            !self.observations.is_empty() && self.observations.len() <= 16,
            "support metadata needs bounded candidate observations"
        );
        let mut anchors = std::collections::BTreeSet::new();
        for observation in &self.observations {
            ensure!(
                ["1.97.1", "1.98.1"].contains(&observation.rust.as_str())
                    && observation.target == target
                    && observation.llvm.len() <= 32
                    && observation.llvm.split('.').count() == 3
                    && observation
                        .llvm
                        .split('.')
                        .all(|part| !part.is_empty() && part.bytes().all(|c| c.is_ascii_digit()))
                    && observation.cargo_llvm_cov == "0.9.0"
                    && observation.acceptance_sha256.len() == 64
                    && observation
                        .acceptance_sha256
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                    && anchors.insert(&observation.acceptance_sha256),
                "unsupported or duplicate candidate observation"
            );
        }
        Ok(())
    }
}
