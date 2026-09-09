//! Policy-owned gates and aggregation over validated evidence.
pub use super::comparison::compare;
use super::{
    array, cross_component, error, evidence, index, project, ratchet, require, schema, string,
    Result,
};
use cross_component::{relationship, subjects as contract_subjects};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateState {
    Pass,
    Fail,
    Warning,
    Informational,
    Skipped,
    Unsupported,
    NotApplicable,
    MeasurementError,
    Blocked,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// An evaluator-owned result. Requiredness is always read from policy at aggregation.
pub struct GateResult {
    pub policy: String,
    pub subject: Option<String>,
    pub state: GateState,
    pub reason: String,
    pub record: Value,
}

fn state(value: &str) -> Result<GateState> {
    serde_json::from_value(json!(value)).map_err(|_| error("unknown gate state"))
}

/// Validate policy shape and context-free rules before project inputs are compiled.
pub fn validate_policy_document(policy: &Value) -> Result<()> {
    super::json::domain(policy)?;
    schema::shape(policy, &schema::POLICY, None)?;
    index(&policy["rules"], "id", "policy ID")?;
    for rule in array(&policy["rules"]) {
        validate_rule_metric(rule)?;
        if rule["scope"]["kind"] == "relationship" {
            validate_relationship_rule(rule)?;
        }
    }
    Ok(())
}

fn validate_rule_metric(rule: &Value) -> Result<()> {
    let metric = string(&rule["metric"]);
    let kind = evidence::metric_type(metric);
    require(
        kind.is_some_and(|k| {
            rule["limit"]["type"] == k
                || (metric == "risk.crap" && rule["limit"]["type"] == "rational")
        }),
        "policy metric/limit type mismatch",
    )?;
    if kind == Some("boolean") {
        require(
            matches!(rule["operator"].as_str(), Some("eq" | "ne")),
            "boolean requires eq or ne",
        )?;
    }
    if kind == Some("ratio") {
        // oneOf is not interpreted by the frozen reference schema walker.
        require(
            super::json::integer(&rule["limit"]["covered"])
                && super::json::integer(&rule["limit"]["total"]),
            "policy ratio requires integer counters",
        )?;
        require(
            super::json::integer_cmp(&rule["limit"]["covered"], &rule["limit"]["total"]).is_le(),
            "policy ratio exceeds total",
        )?;
    }
    Ok(())
}

fn validate_relationship_rule(rule: &Value) -> Result<()> {
    let metric = string(&rule["metric"]);
    require(
        metric.starts_with("contract."),
        "relationship scope requires a contract metric",
    )?;
    require(
        rule.get("ratchet").is_none(),
        "contract comparison uses retained baseline provenance, not debt ratchet",
    )?;
    Ok(())
}

pub fn validate_policy(policy: &Value, project: &Value) -> Result<()> {
    super::json::domain(policy)?;
    schema::shape(policy, &schema::POLICY, None)?;
    project::validate_project(project)?;
    index(&policy["rules"], "id", "policy ID")?;
    for rule in array(&policy["rules"]) {
        validate_rule_metric(rule)?;
        let scope = &rule["scope"];
        require(
            scope.is_object() && scope["kind"].is_string(),
            "invalid policy scope",
        )?;
        if scope["kind"] == "subject" {
            require(
                array(&project["subjects"])
                    .iter()
                    .any(|s| s["id"] == scope["subject"]),
                "unknown policy subject",
            )?;
        }
        if scope["kind"] == "relationship" {
            validate_relationship_rule(rule)?;
            contract_subjects(project, &scope["relationship"], None)?;
        }
        if let Some(component) = scope.get("component") {
            let c = array(&project["components"])
                .iter()
                .find(|c| c["id"] == *component)
                .ok_or_else(|| error("unknown policy component"))?;
            if let Some(boundary) = scope.get("boundary") {
                require(
                    array(&c["source_boundaries"])
                        .iter()
                        .any(|b| b["id"] == *boundary),
                    "unknown policy boundary",
                )?;
            }
        } else {
            require(
                scope.get("boundary").is_none(),
                "policy boundary requires component",
            )?;
        }
    }
    Ok(())
}

pub fn select<'a>(
    scope: &Value,
    project: &'a Value,
    selection: &Value,
    target: &Value,
) -> Result<Vec<&'a Value>> {
    let subjects: Vec<_> = array(&project["subjects"])
        .iter()
        .filter(|s| s["target"] == *target)
        .collect();
    let kind = scope["kind"]
        .as_str()
        .ok_or_else(|| error("invalid policy scope"))?;
    match kind {
        "subject" => Ok(subjects
            .into_iter()
            .filter(|s| s["id"] == scope["subject"])
            .collect()),
        "relationship" => contract_subjects(project, &scope["relationship"], Some(target)),
        "changed_subject" | "critical_subject" => {
            let ids = selection
                .get(kind)
                .ok_or_else(|| error(format!("missing caller-owned {kind} selection")))?;
            require(
                ids.as_array()
                    .is_some_and(|ids| ids.iter().all(Value::is_string)),
                "subject selection must be a list of IDs",
            )?;
            let unique: BTreeSet<_> = array(ids).iter().map(string).collect();
            require(
                unique.len() == array(ids).len()
                    && unique
                        .iter()
                        .all(|id| subjects.iter().any(|s| s["id"] == *id)),
                "unknown/duplicate selected subject",
            )?;
            Ok(subjects
                .into_iter()
                .filter(|s| unique.contains(string(&s["id"])))
                .collect())
        }
        _ => Ok(subjects
            .into_iter()
            .filter(|s| {
                ["component", "boundary"]
                    .iter()
                    .all(|key| scope.get(key).is_none_or(|v| s[key] == *v))
            })
            .collect()),
    }
}

/// Aggregate evaluated gates using policy-owned requiredness and failure precedence.
pub fn aggregate(policy: &Value, results: &[GateResult]) -> Result<Value> {
    super::json::domain(policy)?;
    schema::shape(policy, &schema::POLICY, None)?;
    let rules = index(&policy["rules"], "id", "policy ID")?;
    let mut seen = BTreeSet::new();
    let mut blockers = Vec::new();
    for result in results {
        let rule = rules
            .get(result.policy.as_str())
            .ok_or_else(|| error("unknown result policy"))?;
        require(
            seen.insert((result.policy.as_str(), result.subject.as_deref())),
            "duplicate gate result",
        )?;
        if rule["required"] == true
            && !matches!(result.state, GateState::Pass | GateState::Informational)
        {
            blockers.push(
                json!({"policy":result.policy,"subject":result.subject,"state":result.state}),
            );
        }
    }
    // Preserve policy order; the lookup index is sorted only for lookup.
    for rule in array(&policy["rules"]) {
        if rule["required"] == true && !seen.iter().any(|(id, _)| rule["id"] == *id) {
            blockers.push(json!({"policy":rule["id"],"subject":null,"state":"blocked"}));
        }
    }
    let status = if blockers.iter().any(|b| b["state"] == "measurement_error") {
        "measurement_error"
    } else if blockers.iter().any(|b| b["state"] == "fail") {
        "fail"
    } else if !blockers.is_empty() {
        "blocked"
    } else {
        "pass"
    };
    Ok(json!({"state":status,"blockers":blockers}))
}

fn metric<'a>(record: Option<&'a Value>, name: &Value) -> Option<&'a Value> {
    record.and_then(|r| {
        array(&r["metrics"])
            .iter()
            .find(|m| m["name"] == *name)
            .map(|m| &m["value"])
    })
}

fn result(
    rule: &Value,
    subject: Option<&Value>,
    state: GateState,
    reason: String,
    context: &evidence::ValidationContext<'_>,
    head: Option<&Value>,
    base: Option<&Value>,
) -> GateResult {
    let subject = subject.or_else(|| {
        if rule["scope"]["kind"] == "subject" {
            array(&context.project["subjects"])
                .iter()
                .find(|s| s["id"] == rule["scope"]["subject"])
        } else {
            None
        }
    });
    let link =
        |r: Option<&Value>| r.map(|r| json!({"evidence_id":r["id"],"artifacts":r["artifacts"]}));
    let remediation = match state {
        GateState::Pass => json!([]),
        GateState::Fail | GateState::Warning | GateState::Informational => {
            rule["remediation_classes"].clone()
        }
        _ => json!(["repair_measurement"]),
    };
    let mut detail = json!({"component":subject.map_or(&rule["scope"]["component"], |s| &s["component"]),"subject":subject,"metric":rule["metric"],"base":metric(base,&rule["metric"]),"head":metric(head,&rule["metric"]),"context":context.expected,"policy":rule,"measurement_series":{"base":base.map(|b| &b["series"]["id"]),"head":head.map(|h| &h["series"]["id"])},"evidence_links":{"base":link(base),"head":link(head)},"remediation_classes":remediation});
    if rule["scope"]["kind"] == "relationship" {
        detail["relationship"] = relationship(context.project, &rule["scope"]["relationship"])
            .expect("validated policy relationship")
            .clone();
        detail["contract"] = head.map_or(Value::Null, |h| h["contract"].clone());
    }
    GateResult {
        policy: string(&rule["id"]).into(),
        subject: subject.map(|s| string(&s["id"]).into()),
        state,
        reason,
        record: detail,
    }
}

#[derive(Default)]
pub struct EvaluationOptions<'a> {
    pub selection: Option<&'a Value>,
    pub base_records: Option<&'a Value>,
    pub base_context: Option<&'a evidence::ValidationContext<'a>>,
    pub mappings: Option<&'a Value>,
    pub exceptions: Option<&'a Value>,
    pub now: Option<&'a str>,
}

pub fn evaluate(
    policy: &Value,
    records: &Value,
    context: &evidence::ValidationContext<'_>,
    options: &EvaluationOptions<'_>,
) -> Result<Value> {
    validate_policy(policy, context.project)?;
    let validation = || -> Result<Option<ratchet::Lineage>> {
        if records.as_array().is_some_and(Vec::is_empty) {
        } else {
            evidence::validate_evidence(records, context)?;
        }
        if options.base_records.is_some()
            || options.base_context.is_some()
            || options.mappings.is_some()
        {
            ratchet::validate_base(
                options.base_records,
                options.base_context,
                context,
                options.mappings,
            )
            .map(Some)
        } else {
            Ok(None)
        }
    };
    let mut results = Vec::new();
    match validation() {
        Err(e) => {
            for rule in array(&policy["rules"]) {
                results.push(result(
                    rule,
                    None,
                    GateState::MeasurementError,
                    e.message.clone(),
                    context,
                    None,
                    None,
                ));
            }
        }
        Ok(lineage) => {
            for rule in array(&policy["rules"]) {
                let subjects = match select(
                    &rule["scope"],
                    context.project,
                    options.selection.unwrap_or(&Value::Null),
                    &context.expected["target"],
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        results.push(result(
                            rule,
                            None,
                            GateState::MeasurementError,
                            e.message,
                            context,
                            None,
                            None,
                        ));
                        continue;
                    }
                };
                if subjects.is_empty() {
                    results.push(result(
                        rule,
                        None,
                        GateState::NotApplicable,
                        "scope selects no subjects".into(),
                        context,
                        None,
                        None,
                    ));
                }
                for subject in subjects {
                    let (mut head, mut base, mut baseline, mut assessment) =
                        (None, None, None, None);
                    let mut evaluate_subject = || -> Result<(GateState, String)> {
                        let candidates: Vec<_> = array(records)
                            .iter()
                            .filter(|r| {
                                r["subject"]["id"] == subject["id"]
                                    && array(&r["capabilities"])
                                        .iter()
                                        .any(|c| c["metric"] == rule["metric"])
                                    && (rule["scope"]["kind"] != "relationship"
                                        || r["contract"]
                                            .get("relationship")
                                            .unwrap_or(&rule["scope"]["relationship"])
                                            == &rule["scope"]["relationship"])
                            })
                            .collect();
                        require(
                            candidates.len() == 1,
                            "missing or ambiguous subject metric evidence",
                        )?;
                        let h = candidates[0];
                        head = Some(h);
                        if rule.get("ratchet").is_some() {
                            let lineage = lineage
                                .as_ref()
                                .ok_or_else(|| error("missing base for incremental policy"))?;
                            let (b, info) = ratchet::baseline(
                                h,
                                &rule["metric"],
                                options.base_records.unwrap(),
                                options.base_context.unwrap(),
                                lineage,
                            )?;
                            base = b;
                            baseline = Some(info);
                        } else if let Some(records) = options.base_records {
                            let prior: Vec<_> = array(records)
                                .iter()
                                .filter(|r| {
                                    r["subject"]["id"] == subject["id"]
                                        && array(&r["capabilities"])
                                            .iter()
                                            .any(|c| c["metric"] == rule["metric"])
                                })
                                .collect();
                            require(prior.len() <= 1, "ambiguous base evidence")?;
                            base = prior.first().copied();
                            if let Some(b) = base {
                                evidence::require_compatible_series(
                                    Some(&b["series"]),
                                    &h["series"],
                                )?;
                            }
                        }
                        let cap = array(&h["capabilities"])
                            .iter()
                            .find(|c| c["metric"] == rule["metric"])
                            .unwrap();
                        let unavailable = match string(&cap["state"]) {
                            "unsupported" => Some(GateState::Unsupported),
                            "not_applicable" => Some(GateState::NotApplicable),
                            "not_configured" => Some(GateState::Blocked),
                            "not_collected" => Some(GateState::Skipped),
                            "measurement_error" => Some(GateState::MeasurementError),
                            _ => None,
                        };
                        if let Some(state) = unavailable {
                            return Ok((state, string(&cap["state"]).into()));
                        }
                        let value =
                            metric(head, &rule["metric"]).expect("validated supported metric");
                        if rule["scope"]["kind"] == "relationship" {
                            cross_component::validate(h, rule, context.project)?;
                        }
                        if rule.get("ratchet").is_some() {
                            if let Some(b) = base {
                                let cap = array(&b["capabilities"])
                                    .iter()
                                    .find(|c| c["metric"] == rule["metric"])
                                    .unwrap();
                                require(
                                    cap["state"] == "supported",
                                    format!("base metric unavailable: {}", string(&cap["state"])),
                                )?;
                            }
                            let (s, info) =
                                ratchet::decision(rule, value, metric(base, &rule["metric"]))?;
                            let reason = format!(
                                "ratchet {}; debt {}",
                                string(&info["trend"]),
                                string(&info["debt"])
                            );
                            assessment = Some(info);
                            Ok((state(&s)?, reason))
                        } else {
                            let passed = compare(value, string(&rule["operator"]), &rule["limit"])?;
                            Ok((
                                if passed {
                                    GateState::Pass
                                } else {
                                    state(string(&rule["on_violation"]))?
                                },
                                if passed {
                                    "comparison satisfied"
                                } else {
                                    "policy limit violated"
                                }
                                .into(),
                            ))
                        }
                    };
                    let (s, reason) = evaluate_subject()
                        .unwrap_or_else(|e| (GateState::MeasurementError, e.message));
                    let mut r = result(rule, Some(subject), s, reason, context, head, base);
                    if let Some(info) = baseline {
                        r.record["baseline"] = info;
                    }
                    if let Some(info) = assessment {
                        r.record["ratchet"] = info;
                    }
                    results.push(r);
                }
            }
        }
    }
    let review = ratchet::review_exceptions(
        options.exceptions.unwrap_or(&json!([])),
        policy,
        context.project,
        options.now,
    );
    for r in &mut results {
        let matching: Vec<_> = review["exceptions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|e| {
                e.is_object() && e["policy"] == r.policy && e["subject"] == json!(r.subject)
            })
            .cloned()
            .collect();
        r.record["exception_review"] = json!({"state":if matching.is_empty() {json!("none")} else {review["state"].clone()},"exceptions":matching});
    }
    let quality = aggregate(policy, &results)?;
    let mut combined = quality.clone();
    if !array(&review["errors"]).is_empty() {
        combined["state"] = json!("measurement_error");
        combined["blockers"].as_array_mut().unwrap().push(json!({"state":"measurement_error","reason":"invalid exception metadata","errors":review["errors"]}));
    }
    Ok(
        json!({"schema":"harness-policy-results/v1","mode":"shadow","aggregate":combined,"quality_aggregate":quality,"exception_review":review,"results":results,"debt_ledger":results.iter().filter(|r| r.record["ratchet"].get("debt").is_some_and(|d| d != "none")).collect::<Vec<_>>(),"violations":results.iter().filter(|r| r.state != GateState::Pass).collect::<Vec<_>>()}),
    )
}
