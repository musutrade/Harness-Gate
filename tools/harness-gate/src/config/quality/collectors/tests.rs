use super::*;
use crate::process::adapter::{
    AdapterCapabilities, AdapterDeclaration, AdapterSignature, TrustedKey,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::{Signer, SigningKey};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::{tempdir, TempDir};

struct Fixture {
    dir: TempDir,
    state: TrustedState,
    request: AdapterRequest,
    policy: HostPolicy,
    payload: Value,
}

fn write(path: &Path, value: &impl Serialize) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

impl Fixture {
    fn new(mode: &str, custom: bool) -> Self {
        Self::in_dir(mode, custom, tempdir().unwrap())
    }

    fn in_dir(mode: &str, custom: bool, dir: TempDir) -> Self {
        let root = dir.path();
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../quality/fixtures/workflow");
        fs::create_dir(root.join(".harness-gate")).unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::create_dir_all(root.join("target/evidence")).unwrap();
        fs::copy(
            fixtures.join("compiler/source.txt"),
            root.join("src/lib.rs"),
        )
        .unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("presets/rust-api.flow.toml"),
            root.join(DEFAULT_CONFIG_PATH),
        )
        .unwrap();
        let mut state: TrustedState = serde_json::from_str(
            &fs::read_to_string(fixtures.join("compiler/state.json")).unwrap(),
        )
        .unwrap();
        state.artifacts.clear();
        let direct: Value = serde_json::from_str(
            &fs::read_to_string(fixtures.join("compiler/direct.json")).unwrap(),
        )
        .unwrap();
        let mut record = direct["records"][0].clone();
        let mut rules = direct["policy"].clone();
        let mut quality = fs::read_to_string(fixtures.join("compiler/quality.toml")).unwrap();
        if custom {
            let metadata: Value = serde_json::from_str(
                &fs::read_to_string(fixtures.join("collectors/unknown-ecosystem.json")).unwrap(),
            )
            .unwrap();
            state.components.get_mut("app").unwrap().metadata = BTreeMap::from([(
                "ecosystem".into(),
                metadata["ecosystem"].as_str().unwrap().into(),
            )]);
            let series = state.series.get_mut("coverage").unwrap();
            let old = series.id.clone();
            series.collector.name = metadata["collector"].as_str().unwrap().into();
            series.name = metadata["series"].as_str().unwrap().into();
            series.tool.name = metadata["tool"].as_str().unwrap().into();
            series.runtime.name = "quasar-vm".into();
            let capability = &metadata[if mode == "custom-capability" {
                "capability"
            } else {
                "normalized_capability"
            }];
            let mut value = serde_json::to_value(&series).unwrap();
            value["metrics"] = json!([{"name":capability,"type":"size"}]);
            value["id"] = json!(evidence::series_id(&value).unwrap());
            *series = serde_json::from_value(value).unwrap();
            quality = quality
                .replace(&old, &series.id)
                .replace("coverage.line", capability.as_str().unwrap());
            record["capabilities"][0]["metric"] = capability.clone();
            record["metrics"][0]["name"] = capability.clone();
            record["metrics"][0]["value"] = json!({"type":"size","value":5,"unit":"bytes"});
            rules["rules"][0]["metric"] = capability.clone();
            rules["rules"][0]["limit"] = json!({"type":"size","value":10,"unit":"bytes"});
            rules["rules"][0]["operator"] = json!("le");
        }
        record["series"] = serde_json::to_value(&state.series["coverage"]).unwrap();
        record["collector"] = record["series"]["collector"].clone();
        match mode {
            "unsupported" | "not_collected" | "measurement_error" | "not_configured"
            | "not_applicable" => {
                record["capabilities"][0]["state"] = json!(mode);
                record["metrics"] = json!([]);
                record["status"] = json!(if mode == "measurement_error" {
                    "measurement_error"
                } else {
                    "unavailable"
                });
            }
            "dishonest" => record["capabilities"][0]["state"] = json!("unsupported"),
            "missing-capability" => record["capabilities"] = json!([]),
            "wrong-tool" => record["series"]["tool"]["version"] = json!("wrong"),
            "stale-context" => record["context"]["run"] = json!("stale"),
            "wrong-subject" => record["subject"]["id"] = json!("unknown"),
            "malformed-evidence" => {
                record["metrics"][0]["value"] = json!({"type":"ratio","covered":9,"total":1})
            }
            "artifact-identity" => record["artifacts"][0]["context"]["run"] = json!("other"),
            _ => {}
        }
        if mode == "custom-capability" {
            let mut config: QualityConfig = toml::from_str(&quality).unwrap();
            config
                .policies
                .get_mut("coverage")
                .unwrap()
                .expectation
                .capability = "bundle.size".into();
            rules["rules"][0]["metric"] = json!("bundle.size");
            rules["rules"][0]["required"] = json!(false);
            quality = toml::to_string(&config).unwrap();
        }
        fs::write(root.join(".harness-gate/quality.toml"), quality).unwrap();
        write(&root.join(".harness-gate/policy.json"), &rules);
        write(
            &root.join(".harness-gate/coverage-request.json"),
            &json!({}),
        );
        let mut response = json!({"schema_version":"1", "status":"PASS", "invocation_id":state.expected.run,
            "artifacts":record["artifacts"], "collection":{"schema":"harness-project-collector-response/v1", "evidence":[record.clone()], "error":null}});
        match mode {
            "duplicate-evidence" => {
                response["collection"]["evidence"] = json!([record.clone(), record])
            }
            "approval" => response["collection"]["approved"] = json!(true),
            "missing-evidence" => response["collection"]["evidence"] = json!([]),
            "response-version" => {
                response["collection"]["schema"] = json!("harness-project-collector-response/v2")
            }
            "transport-fail" => response["status"] = json!("FAIL"),
            "measurement-fail" => {
                response["collection"]["error"] = json!({"code":"measurement_error"})
            }
            "missing-inventory" => response["artifacts"] = json!([]),
            _ => {}
        }
        let payload = json!({"mode":mode,"response":response,"raw":fs::read_to_string(fixtures.join("compiler/artifact.txt")).unwrap()});
        let script = fs::read_to_string(fixtures.join("collectors/collector.py"))
            .unwrap()
            .replace(
                "'PAYLOAD_PLACEHOLDER'",
                &serde_json::to_string(&serde_json::to_string(&payload).unwrap()).unwrap(),
            );
        let executable = root.join("collector.py");
        fs::write(&executable, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let request = AdapterRequest {
            protocol_version: 2,
            result_schema_version: "1".into(),
            adapter: AdapterDeclaration {
                name: state.series["coverage"].collector.name.clone(),
                version: "1".into(),
                executable: executable.clone(),
                source_digest: format!("{:x}", Sha256::digest(fs::read(executable).unwrap())),
                signature: AdapterSignature {
                    algorithm: "ed25519".into(),
                    key_id: "fixture".into(),
                    value: String::new(),
                },
            },
            invocation_id: state.expected.run.clone(),
            step_id: "coverage".into(),
            timeout_ms: if mode == "timeout" { 50 } else { 5000 },
            config_digest: String::new(),
            artifact_root: root.join("target/evidence"),
            nonce: "fixture-nonce".into(),
            issued_at_ms: now,
            expires_at_ms: now + 60000,
            args: vec![],
            environment: BTreeMap::new(),
            capabilities: AdapterCapabilities::default(),
            input: Value::Null,
        };
        let policy = HostPolicy {
            trusted_keys: vec![TrustedKey {
                key_id: "fixture".into(),
                public_key: BASE64
                    .encode(SigningKey::from_bytes(&[7; 32]).verifying_key().as_bytes()),
            }],
            max_timeout: Some(Duration::from_secs(10)),
            ..HostPolicy::default()
        };
        let mut fixture = Self {
            dir,
            state,
            request,
            policy,
            payload,
        };
        fixture.pin();
        let mut config = fixture.config();
        let id = if custom {
            let metadata: Value = serde_json::from_str(
                &fs::read_to_string(fixtures.join("collectors/unknown-ecosystem.json")).unwrap(),
            )
            .unwrap();
            metadata["binding"].as_str().unwrap().to_owned()
        } else {
            "coverage".into()
        };
        if custom {
            let binding = config.collectors.remove("coverage").unwrap();
            config.collectors.insert(id.clone(), binding);
            for profile in config.profiles.values_mut() {
                if profile.collectors.remove("coverage") {
                    profile.collectors.insert(id.clone());
                }
            }
            let series = fixture.state.series.remove("coverage").unwrap();
            fixture.state.series.insert(id.clone(), series);
            fs::write(
                fixture.dir.path().join(".harness-gate/quality.toml"),
                toml::to_string(&config).unwrap(),
            )
            .unwrap();
            fixture.pin();
        }
        let inputs = compiler::compile(fixture.dir.path(), &fixture.state).unwrap();
        // Bind the canonical root even when the temp directory uses a symlink
        // (for example, /var -> /private/var on macOS).
        fixture.request.artifact_root = inputs.artifact_root.clone();
        fixture.request.step_id = id.clone();
        fixture.request.config_digest = binding_digest(&config, &fixture.state, &inputs).unwrap();
        fixture.request.input = input(
            &inputs,
            &fixture.state,
            &id,
            &claims(&config, &fixture.state, &id).unwrap(),
        );
        fixture.sign();
        fixture
    }

    fn config(&self) -> QualityConfig {
        toml::from_str(
            &fs::read_to_string(self.dir.path().join(".harness-gate/quality.toml")).unwrap(),
        )
        .unwrap()
    }

    fn pin(&mut self) {
        let mut files: BTreeSet<String> = [DEFAULT_CONFIG_PATH, ".harness-gate/quality.toml"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        files.extend(self.config().collectors.values().map(|c| c.request.clone()));
        files.extend(
            self.config()
                .policies
                .values()
                .map(|p| p.policy_file.clone()),
        );
        self.state.config_files = files
            .into_iter()
            .map(|p| {
                (
                    p.clone(),
                    format!(
                        "{:x}",
                        Sha256::digest(fs::read(self.dir.path().join(p)).unwrap())
                    ),
                )
            })
            .collect();
    }

    fn sign(&mut self) {
        self.request.adapter.signature.value = BASE64.encode(
            SigningKey::from_bytes(&[7; 32])
                .sign(&adapter::signing_payload(&self.request).unwrap())
                .to_bytes(),
        );
        self.save();
    }

    fn save(&mut self) {
        write(
            &self.dir.path().join(".harness-gate/coverage-request.json"),
            &self.request,
        );
        self.pin();
    }

    fn collect(&self) -> Result<Collection> {
        collect(self.dir.path(), &self.state, &self.policy)
    }
}

#[test]
fn configured_multiple_producers_combine_distinct_series_and_reject_overlap() {
    for overlap in [false, true] {
        let mut fixture = Fixture::new("pass", false);
        let root = fixture.dir.path().to_path_buf();
        let mut config = fixture.config();
        let mut series = serde_json::to_value(&fixture.state.series["coverage"]).unwrap();
        series["name"] = json!("independent-measurement");
        if !overlap {
            series["metrics"] = json!([{"name":"bundle.size","type":"size"}]);
        }
        series["id"] = json!(evidence::series_id(&series).unwrap());
        fixture.state.series.insert(
            "second".into(),
            serde_json::from_value(series.clone()).unwrap(),
        );
        let mut binding = config.collectors["coverage"].clone();
        binding.request = ".harness-gate/second-request.json".into();
        binding.produces[0].series = series["id"].as_str().unwrap().into();
        if overlap {
            // Distinct configured targets still resolve to the same subject.
            binding.produces[0].target = super::super::model::Target::Subject {
                id: "module".into(),
            };
        } else {
            binding.produces[0].capability = "bundle.size".into();
        }
        config.collectors.insert("second".into(), binding);
        config
            .profiles
            .get_mut(&fixture.state.profile)
            .unwrap()
            .collectors
            .insert("second".into());
        fs::write(
            root.join(".harness-gate/quality.toml"),
            toml::to_string(&config).unwrap(),
        )
        .unwrap();
        write(&root.join(".harness-gate/second-request.json"), &json!({}));
        fixture.pin();
        let mut payload = fixture.payload.clone();
        let record = &mut payload["response"]["collection"]["evidence"][0];
        record["series"] = series;
        record["id"] = json!("second-evidence");
        record["artifacts"][0]["path"] = json!("second.json");
        if !overlap {
            record["capabilities"][0]["metric"] = json!("bundle.size");
            record["metrics"][0]["name"] = json!("bundle.size");
            record["metrics"][0]["value"] = json!({"type":"size","value":5,"unit":"bytes"});
        }
        payload["response"]["artifacts"] = record["artifacts"].clone();
        let template = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../quality/fixtures/workflow/collectors/collector.py");
        let script = fs::read_to_string(template)
            .unwrap()
            .replace("raw.json", "second.json")
            .replace(
                "'PAYLOAD_PLACEHOLDER'",
                &serde_json::to_string(&serde_json::to_string(&payload).unwrap()).unwrap(),
            );
        let executable = root.join("second.py");
        fs::write(&executable, &script).unwrap();
        fs::set_permissions(
            &executable,
            fs::metadata(&fixture.request.adapter.executable)
                .unwrap()
                .permissions(),
        )
        .unwrap();
        let inputs = compiler::compile(&root, &fixture.state).unwrap();
        fixture.request.config_digest = binding_digest(&config, &fixture.state, &inputs).unwrap();
        let mut second = fixture.request.clone();
        second.step_id = "second".into();
        second.nonce = "second-nonce".into();
        second.adapter.executable = executable;
        second.adapter.source_digest = format!("{:x}", Sha256::digest(script.as_bytes()));
        second.input = input(
            &inputs,
            &fixture.state,
            "second",
            &claims(&config, &fixture.state, "second").unwrap(),
        );
        second.adapter.signature.value = BASE64.encode(
            SigningKey::from_bytes(&[7; 32])
                .sign(&adapter::signing_payload(&second).unwrap())
                .to_bytes(),
        );
        write(&root.join(".harness-gate/second-request.json"), &second);
        fixture.sign();
        let result = fixture.collect();
        if overlap {
            assert!(result.is_err());
            assert!(
                !fixture.request.artifact_root.join("raw.json").exists(),
                "duplicate ownership must fail before launch"
            );
        } else {
            let collection = result.unwrap();
            assert_eq!(collection.evidence.as_array().unwrap().len(), 2);
            assert!(fixture.request.artifact_root.join("second.json").is_file());
        }
    }
}

#[test]
#[cfg(unix)]
fn configured_collector_launches_from_a_symlinked_workspace_root() {
    let parent = tempdir().unwrap();
    let real = parent.path().join("real");
    let alias = parent.path().join("alias");
    fs::create_dir(&real).unwrap();
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let dir = tempfile::tempdir_in(&alias).unwrap();
    assert_ne!(dir.path(), dir.path().canonicalize().unwrap());
    let fixture = Fixture::in_dir("pass", true, dir);
    let collection = fixture.collect().unwrap();
    assert_eq!(collection.evidence.as_array().unwrap().len(), 1);
    assert!(fixture.request.artifact_root.join("raw.json").is_file());
}

#[test]
#[cfg(unix)]
fn configured_ecosystems_launch_through_same_boundary_and_preserve_capability_states() {
    for custom in [false, true] {
        for state in [
            "pass",
            "unsupported",
            "not_collected",
            "measurement_error",
            "not_configured",
            "not_applicable",
        ] {
            let fixture = Fixture::new(state, custom);
            let collection = fixture
                .collect()
                .unwrap_or_else(|e| panic!("{custom}/{state}: {e:#}"));
            assert_eq!(
                collection.evidence[0]["capabilities"][0]["state"],
                if state == "pass" { "supported" } else { state }
            );
            assert!(collection.evidence[0]["metrics"]
                .as_array()
                .unwrap()
                .iter()
                .all(|m| m["name"] != "risk.crap"));
        }
    }
    // Arbitrary capability IDs are resolved and launched without core language
    // dispatch; the released evidence validator still owns metric certification.
    let fixture = Fixture::new("custom-capability", true);
    let error = fixture.collect().unwrap_err();
    assert!(format!("{error:#}").contains("unknown generic metric: quasar.payload_bytes"));
    assert!(fixture.request.artifact_root.join("raw.json").is_file());
}

#[test]
#[cfg(unix)]
fn collector_protocol_and_evidence_fail_closed() {
    for mode in [
        "crash",
        "timeout",
        "malformed",
        "duplicate-json",
        "tamper-artifact",
        "extra-artifact",
        "source-change",
        "config-change",
        "symlink",
        "dishonest",
        "missing-capability",
        "wrong-tool",
        "stale-context",
        "wrong-subject",
        "malformed-evidence",
        "artifact-identity",
        "duplicate-evidence",
        "approval",
        "missing-evidence",
        "response-version",
        "transport-fail",
        "measurement-fail",
        "missing-inventory",
    ] {
        let fixture = Fixture::new(mode, false);
        assert!(fixture.collect().is_err(), "{mode} must fail");
    }
}

#[test]
#[cfg(unix)]
fn collector_signed_binding_failures() {
    for mode in [
        "duplicate-request",
        "signature",
        "protocol",
        "package",
        "config",
        "selection",
        "roots",
        "executable",
        "launch",
        "timeout-limit",
        "output-limit",
        "artifact-limit",
        "duplicate-producer",
        "stale-root",
    ] {
        let mut fixture = Fixture::new("pass", false);
        match mode {
            "duplicate-request" => {}
            "signature" => fixture.request.adapter.signature.value = BASE64.encode([0; 64]),
            "protocol" => fixture.request.protocol_version = 1,
            "package" => fixture.request.adapter.name = "other".into(),
            "config" => fixture.request.config_digest = "0".repeat(64),
            "selection" => fixture.request.input["bindings"] = json!([]),
            "roots" => fixture.request.artifact_root = fixture.dir.path().to_owned(),
            "executable" => fs::write(&fixture.request.adapter.executable, "changed").unwrap(),
            "launch" => {
                fs::write(
                    &fixture.request.adapter.executable,
                    "#!/missing/interpreter\n",
                )
                .unwrap();
                fixture.request.adapter.source_digest = format!(
                    "{:x}",
                    Sha256::digest(fs::read(&fixture.request.adapter.executable).unwrap())
                );
            }
            "timeout-limit" => fixture.policy.max_timeout = Some(Duration::from_millis(1)),
            "output-limit" => fixture.policy.max_stdout_bytes = 10,
            "artifact-limit" => fixture.policy.max_artifact_bytes = Some(1),
            "duplicate-producer" => {
                let mut config = fixture.config();
                let duplicate = config.collectors["coverage"].produces[0].clone();
                config
                    .collectors
                    .get_mut("coverage")
                    .unwrap()
                    .produces
                    .push(duplicate);
                fs::write(
                    fixture.dir.path().join(".harness-gate/quality.toml"),
                    toml::to_string(&config).unwrap(),
                )
                .unwrap();
            }
            "stale-root" => {
                fs::write(fixture.request.artifact_root.join("stale"), "stale").unwrap()
            }
            _ => unreachable!(),
        }
        if mode == "signature" {
            fixture.save();
        } else {
            fixture.sign();
        }
        if mode == "duplicate-request" {
            let path = fixture
                .dir
                .path()
                .join(".harness-gate/coverage-request.json");
            let text =
                fs::read_to_string(&path)
                    .unwrap()
                    .replacen('{', "{\"protocol_version\":2,", 1);
            fs::write(path, text).unwrap();
            fixture.pin();
        }
        assert!(fixture.collect().is_err(), "{mode} must fail");
    }
}

#[test]
fn generic_orchestration_has_no_closed_language_dispatch() {
    // Block language/framework enums, metadata inspection and named ecosystem
    // cases in the generic path. Runtime coverage above proves arbitrary IDs.
    for forbidden in [
        "enum ",
        "match ",
        ".metadata",
        "Ecosystem",
        "Language",
        "Framework",
    ] {
        let code = include_str!("../collectors.rs")
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains(forbidden),
            "collector dispatch guard: {forbidden}"
        );
    }
    for source in [
        include_str!("../collectors.rs"),
        include_str!("../compiler.rs"),
    ] {
        let code = source
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for forbidden in [
            "enum Language",
            "enum Ecosystem",
            "enum Framework",
            "Language::",
            "Ecosystem::",
            "Framework::",
            "\"rust\"",
            "\"angular\"",
            "\"go\"",
            "\"vue\"",
            "[\"ecosystem\"]",
            "[\"language\"]",
            "get(\"ecosystem\")",
            "get(\"language\")",
        ] {
            assert!(
                !code.contains(forbidden),
                "generic dispatch guard: {forbidden}"
            );
        }
    }
}

#[test]
fn collection_cli_publishes_measurements_and_removes_stale_output_on_failure() {
    use clap::Parser;
    #[derive(Parser)]
    struct Command {
        #[command(subcommand)]
        action: crate::app::quality::QualityAction,
    }
    for mode in ["pass", "malformed"] {
        let fixture = Fixture::new(mode, true);
        let root = fixture.dir.path();
        let state = root.join("state.json");
        let keys = root.join("keys.json");
        let output = root.join("collection.json");
        write(&state, &fixture.state);
        write(
            &keys,
            &json!([{"key_id":fixture.policy.trusted_keys[0].key_id,"public_key":fixture.policy.trusted_keys[0].public_key}]),
        );
        fs::write(&output, "stale success").unwrap();
        let command = Command::try_parse_from([
            "quality",
            "collect",
            "--repository-root",
            root.to_str().unwrap(),
            "--state",
            state.to_str().unwrap(),
            "--trusted-keys",
            keys.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ])
        .unwrap();
        let result = crate::app::quality::run(&command.action);
        if mode == "pass" {
            assert!(result.unwrap());
            let value: Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
            assert_eq!(value["schema"], "quality-collection/v1");
            assert!(value.get("approval").is_none());
            assert!(root.join(".harness-gate/collector-nonces").is_dir());
        } else {
            assert!(result.is_err());
            assert!(!output.exists());
        }
    }
}
