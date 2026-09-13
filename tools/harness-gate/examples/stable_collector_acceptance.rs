//! Repository-only acceptance of the real stable collector through the shipped
//! Core CLI and evidence validator. The deterministic key is TEST ONLY.
use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use ed25519_dalek::{Signer, SigningKey};
use harness_gate::quality::{evidence, project};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn read(path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
fn write(path: &Path, value: &Value) -> Result<()> {
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}
fn sign(request: &mut Value) -> Result<()> {
    // Core protocol v2 serializes this ordered struct, with nested maps sorted.
    let mut declaration = request["adapter"].clone();
    declaration["signature"]
        .as_object_mut()
        .unwrap()
        .remove("value");
    let adapter = format!(
        "{{{}}}",
        [
            "name",
            "version",
            "executable",
            "source_digest",
            "signature"
        ]
        .iter()
        .map(|k| format!("{k:?}:{}", declaration[k]))
        .collect::<Vec<_>>()
        .join(",")
    );
    let mut fields = vec![r#""domain":"harness-gate/adapter-request/v2""#.into()];
    for key in [
        "protocol_version",
        "result_schema_version",
        "adapter",
        "invocation_id",
        "step_id",
        "timeout_ms",
        "config_digest",
        "artifact_root",
        "nonce",
        "issued_at_ms",
        "expires_at_ms",
        "args",
        "environment",
        "capabilities",
        "input",
    ] {
        let value = if key == "adapter" {
            adapter.clone()
        } else if key == "capabilities" {
            format!(
                "{{{}}}",
                ["network", "resources", "environment"]
                    .iter()
                    .map(|k| format!("{k:?}:{}", request[key][k]))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        } else {
            request[key].to_string()
        };
        fields.push(format!("{key:?}:{value}"));
    }
    request["adapter"]["signature"]["value"] = json!(BASE64.encode(
        SigningKey::from_bytes(&[41; 32])
            .sign(format!("{{{}}}", fields.join(",")).as_bytes())
            .to_bytes()
    ));
    Ok(())
}

struct Fixture {
    core: PathBuf,
    root: PathBuf,
    binding: Value,
    request: Value,
    key: PathBuf,
}
impl Fixture {
    fn run(&self, label: &str, request: &Value, success: bool) -> Result<Value> {
        let path = self.root.join(format!("{label}.request.json"));
        write(&path, request)?;
        let mut command = Command::new(&self.core);
        command
            .args(["adapter", "run", "--request"])
            .arg(&path)
            .arg("--trusted-key")
            .arg(&self.key);
        for name in request["capabilities"]["environment"].as_array().unwrap() {
            command
                .arg("--allow-environment")
                .arg(name.as_str().unwrap());
        }
        let output = command.output()?;
        fs::write(self.root.join(format!("{label}.stdout")), &output.stdout)?;
        fs::write(self.root.join(format!("{label}.stderr")), &output.stderr)?;
        ensure!(
            output.status.success() == success,
            "{label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        if success {
            Ok(serde_json::from_slice(&output.stdout)?)
        } else {
            Ok(json!({"blocked":true,"error":String::from_utf8_lossy(&output.stderr)}))
        }
    }
    fn altered_binding(&self, label: &str, mutate: impl FnOnce(&mut Value)) -> Result<Value> {
        let mut binding = self.binding.clone();
        let output = self.root.join(label);
        fs::create_dir(&output)?;
        binding["input"]["output_root"] = json!(output);
        binding["invocation_id"] = json!(label);
        mutate(&mut binding);
        let path = self.root.join(format!("{label}.binding.json"));
        write(&path, &binding)?;
        let mut request = self.request.clone();
        request["input"] = binding["input"].clone();
        request["artifact_root"] = json!(output);
        request["invocation_id"] = binding["invocation_id"].clone();
        request["nonce"] = json!(label);
        request["args"] = json!([
            "adapter",
            "--binding",
            path,
            "--binding-sha256",
            hash(&fs::read(&path)?)
        ]);
        sign(&mut request)?;
        self.run(label, &request, false)
    }
}

fn run() -> Result<()> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    ensure!(
        args.len() == 5,
        "usage: stable_collector_acceptance COLLECTOR CORE CAPTURE_ACCEPTANCE NEW_OUTPUT plain|boundaries|features|registry"
    );
    let binary = Path::new(&args[0]).canonicalize()?;
    let core = Path::new(&args[1]).canonicalize()?;
    let capture_root = Path::new(&args[2]).canonicalize()?;
    fs::create_dir(&args[3])?;
    let root = Path::new(&args[3]).canonicalize()?;
    let case = args[4].to_str().context("fixture name")?;
    ensure!(
        ["plain", "boundaries", "features", "registry"].contains(&case),
        "unknown fixture"
    );
    let anchor = read(&capture_root.join(format!("{case}.stdout")))?;
    let capture = capture_root.join(format!("capture-{case}"));
    let described = Command::new(&binary)
        .arg("describe")
        .arg(&capture)
        .arg(anchor["manifest_sha256"].as_str().unwrap())
        .arg(anchor["request_sha256"].as_str().unwrap())
        .output()?;
    ensure!(
        described.status.success(),
        "describe: {}",
        String::from_utf8_lossy(&described.stderr)
    );
    let description: Value = serde_json::from_slice(&described.stdout)?;
    write(&root.join("description.json"), &description)?;
    let output = root.join("evidence");
    fs::create_dir(&output)?;
    let context = json!({"target":description["series"]["target"],"run":"stable-core-acceptance","commit":"a".repeat(40),"base_commit":"b".repeat(40)});
    let mut model = json!({"schema":"harness-project/v1","id":"stable-fixture","metadata":{},"relationships":[],"components":[{"id":"rust","path":".","metadata":{},"targets":[{"id":description["series"]["target"],"boundaries":["production"],"metadata":{}}],"source_boundaries":[{"id":"production","path":".","role":"production","metadata":{}}]}],"subjects":[]});
    for owner in description["owners"].as_array().unwrap() {
        let mut subject = json!({"id":format!("subject-identity/v1:{}","0".repeat(64)),"identity_version":"subject-identity/v1","component":"rust","target":description["series"]["target"],"boundary":"production","kind":"function/v1","path":owner["path"],"discriminator":owner["discriminator"],"source_sha256":owner["source_sha256"],"metadata":{}});
        subject["id"] = json!(project::subject_id("stable-fixture", &subject)?);
        model["subjects"].as_array_mut().unwrap().push(subject);
    }
    let claims: Vec<_> = model["subjects"].as_array().unwrap().iter().flat_map(|s|description["series"]["metrics"].as_array().unwrap().iter().map(|m|json!({"subject":s["id"],"capability":m["name"],"series":description["series"]["id"]}))).collect();
    let inner = json!({"schema":"harness-project-collector-request/v1","project":model["id"],"collector":description["series"]["collector"],"context":context,"workspace_root":description["workspace_root"],"output_root":output,"selection":{},"bindings":claims});
    let binding = json!({"schema":"rust-stable-core-binding/v1","input":inner,"invocation_id":"stable-positive","config_digest":"c".repeat(64),"capture":{"path":capture,"manifest_sha256":anchor["manifest_sha256"],"request_sha256":anchor["request_sha256"]},"project":model,"series":description["series"]});
    let binding_path = root.join("binding.json");
    write(&binding_path, &binding)?;
    let mut environment = serde_json::Map::new();
    for name in [
        "PATH",
        "HOME",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "RUSTUP_TOOLCHAIN",
    ] {
        if let Ok(value) = env::var(name) {
            environment.insert(name.into(), json!(value));
        }
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    let mut request = json!({"protocol_version":2,"result_schema_version":"1","adapter":{"name":inner["collector"]["name"],"version":inner["collector"]["version"],"executable":binary,"source_digest":hash(&fs::read(&binary)?),"signature":{"algorithm":"ed25519","key_id":"acceptance-test-only","value":""}},"invocation_id":binding["invocation_id"],"step_id":"stable","timeout_ms":120000,"config_digest":binding["config_digest"],"artifact_root":output,"nonce":"positive","issued_at_ms":now,"expires_at_ms":now+300000,"args":["adapter","--binding",binding_path,"--binding-sha256",hash(&fs::read(&binding_path)?)],"environment":environment,"capabilities":{"network":[],"resources":[],"environment":environment.keys().collect::<Vec<_>>()},"input":inner});
    sign(&mut request)?;
    let key = root.join("test-only-key.json");
    write(
        &key,
        &json!({"key_id":"acceptance-test-only","public_key":BASE64.encode(SigningKey::from_bytes(&[41;32]).verifying_key().to_bytes())}),
    )?;
    let fixture = Fixture {
        core,
        root: root.clone(),
        binding,
        request,
        key,
    };
    let response = fixture.run("positive", &fixture.request, true)?;
    let records = &response["collection"]["evidence"];
    let source_root = Path::new(description["workspace_root"].as_str().unwrap());
    let validation = evidence::ValidationContext {
        project: &model,
        expected: &context,
        source_root,
        artifact_root: &output,
    };
    evidence::validate_evidence(records, &validation)
        .context("Core normalized evidence validation")?;
    ensure!(
        records.as_array().unwrap().len() == model["subjects"].as_array().unwrap().len(),
        "owner count"
    );
    let counts: Vec<_> = records
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|r| r["metrics"].as_array().unwrap().iter())
        .filter(|m| m["name"] == "complexity.cyclomatic")
        .map(|m| m["value"]["value"].clone())
        .collect();
    if case == "plain" {
        ensure!(
            counts == vec![json!(3), json!(1)],
            "real lexical counts: {counts:?}"
        );
        let coverage: Vec<_> = records
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|r| r["metrics"].as_array().unwrap())
            .filter(|m| m["name"] == "coverage.function")
            .map(|m| m["value"].clone())
            .collect();
        ensure!(
            coverage
                == vec![
                    json!({"type":"ratio","covered":1,"total":1}),
                    json!({"type":"ratio","covered":0,"total":1})
                ],
            "real function coverage: {coverage:?}"
        );
    } else if case == "registry" {
        ensure!(counts == vec![json!(1)], "registry lexical count");
        let metric = records[0]["metrics"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["name"] == "coverage.function")
            .context("registry function coverage missing")?;
        ensure!(
            metric["value"] == json!({"type":"ratio","covered":1,"total":1}),
            "registry function execution coverage"
        );
    } else {
        ensure!(
            !records.as_array().unwrap().is_empty(),
            "boundary owners missing"
        );
        ensure!(
            counts.is_empty()
                && records
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|r| r["status"] == "unavailable"
                        && r["capabilities"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .all(|c| c["state"] == "unsupported")),
            "uncertified boundary must remain unavailable"
        );
    }
    let requirements = json!({"schema":"capability-requirements/v1","requirements":[{"component":"rust","metric":"risk.crap","mode":"required","on_unavailable":"blocked"}]});
    let required = evidence::evaluate_requirements(records, &requirements, &validation)?;
    ensure!(
        required
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["status"] == "blocked"),
        "required CRAP must block"
    );
    let mut checks = json!({"fixture":case,"authenticated_capture":{"accepted":true,"records":records.as_array().unwrap().len(),"counts":counts},"required_crap":required});
    checks["nonce_replay"] = fixture.run("replay", &fixture.request, false)?;
    let mut bad = fixture.request.clone();
    bad["input"]["context"]["run"] = json!("tampered");
    checks["invalid_signature"] = fixture.run("signature", &bad, false)?;
    let mut bad = fixture.request.clone();
    bad["nonce"] = json!("expired");
    bad["issued_at_ms"] = json!(now - 120000);
    bad["expires_at_ms"] = json!(now - 60000);
    sign(&mut bad)?;
    checks["expired_request"] = fixture.run("expired", &bad, false)?;
    for (label, key, value) in [
        ("changed-series", "series", json!({})),
        (
            "changed-project",
            "project",
            json!({"id":"wrong","subjects":[]}),
        ),
    ] {
        checks[label] = fixture.altered_binding(label, |b| b[key] = value)?;
    }
    checks["unknown-owner"] = fixture.altered_binding("unknown-owner", |b| {
        b["project"]["subjects"][0]["discriminator"] = json!("unknown")
    })?;
    checks["wrong-target"] = fixture.altered_binding("wrong-target", |b| {
        b["input"]["context"]["target"] = json!("native")
    })?;
    checks["wrong-source"] = fixture.altered_binding("wrong-source", |b| {
        b["project"]["subjects"][0]["source_sha256"] = json!("0".repeat(64))
    })?;
    checks["missing-claim"] = fixture.altered_binding("missing-claim", |b| {
        b["input"]["bindings"].as_array_mut().unwrap().pop();
    })?;
    checks["wrong-capture-anchor"] = fixture.altered_binding("wrong-capture-anchor", |b| {
        b["capture"]["manifest_sha256"] = json!("0".repeat(64))
    })?;
    checks["stale-config-binding"] = fixture.altered_binding("stale-config-binding", |b| {
        b["config_digest"] = json!("d".repeat(64))
    })?;
    let mut stale = records.clone();
    stale[0]["context"]["run"] = json!("stale");
    ensure!(
        evidence::validate_evidence(&stale, &validation).is_err(),
        "Core stale context"
    );
    checks["core-stale-context"] = json!({"blocked":true});
    let artifact = output.join(records[0]["artifacts"][0]["path"].as_str().unwrap());
    let original = fs::read(&artifact)?;
    fs::write(&artifact, b"corrupt")?;
    let rejected = evidence::validate_evidence(records, &validation).is_err();
    fs::write(&artifact, original)?;
    ensure!(rejected, "Core artifact corruption");
    checks["core-corrupt-artifact"] = json!({"blocked":true});
    evidence::validate_evidence(records, &validation)?;
    write(
        &root.join("summary.json"),
        &json!({"schema":"rust-stable-core-acceptance/v1","state":"candidate-only","production_signature":false,"core_sha256":hash(&fs::read(&fixture.core)?),"collector_sha256":hash(&fs::read(&binary)?),"capture":fixture.binding["capture"],"series":description["series"],"checks":checks}),
    )?;
    println!("Core authenticated transport and normalized evidence acceptance passed; required CRAP remains blocked");
    Ok(())
}
fn main() -> Result<()> {
    run()
}
