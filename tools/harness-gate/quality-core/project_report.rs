//! Lossless project gate tables and indexes over evaluated generic policy results.
use super::{array, error, policy, string, Result};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Build the stable shadow report from a validated project/policy and its result.
/// Gate order and evidence links are preserved; participant indexes may share gates.
pub fn report(policy_result: &Value, project: &Value, policy: &Value) -> Result<Value> {
    let mut gates = Map::new();
    let mut indexes: BTreeMap<&str, BTreeMap<String, Vec<String>>> =
        ["component", "subject", "policy", "status", "relationship"]
            .into_iter()
            .map(|name| (name, BTreeMap::new()))
            .collect();
    for (position, result) in array(&policy_result["results"]).iter().enumerate() {
        let id = format!("gate-{position:06}");
        gates.insert(id.clone(), result.clone());
        let record = &result["record"];
        let link = record
            .get("relationship")
            .filter(|v| v.is_object() && !v.as_object().unwrap().is_empty());
        let components = if let Some(link) = link {
            vec![&link["producer"], &link["consumer"]]
        } else if record["component"].as_str().is_some_and(|s| !s.is_empty()) {
            vec![&record["component"]]
        } else {
            vec![]
        };
        for (dimension, keys) in [
            ("component", components),
            ("subject", vec![&result["subject"]]),
            ("policy", vec![&result["policy"]]),
            ("status", vec![&result["state"]]),
            (
                "relationship",
                link.map_or_else(Vec::new, |l| vec![&l["id"]]),
            ),
        ] {
            for key in keys.into_iter().filter(|k| !k.is_null()) {
                indexes
                    .get_mut(dimension)
                    .unwrap()
                    .entry(string(key).into())
                    .or_default()
                    .push(id.clone());
            }
        }
    }
    let aggregate = |ids: &[String]| -> Result<Value> {
        let results: Vec<policy::GateResult> = ids
            .iter()
            .map(|id| {
                serde_json::from_value(gates[id].clone())
                    .map_err(|e| error(format!("invalid gate result: {e}")))
            })
            .collect::<Result<_>>()?;
        let policies: BTreeSet<_> = results.iter().map(|r| r.policy.as_str()).collect();
        if policies.is_empty() {
            return Ok(json!({"state":"not_applicable", "blockers":[]}));
        }
        let mut subset = policy.clone();
        subset["rules"] = array(&policy["rules"])
            .iter()
            .filter(|r| policies.contains(string(&r["id"])))
            .cloned()
            .collect();
        policy::aggregate(&subset, &results)
    };
    let mut components = Map::new();
    for component in array(&project["components"]) {
        let id = string(&component["id"]);
        let ids = indexes["component"].get(id).cloned().unwrap_or_default();
        let (cross, local): (Vec<_>, Vec<_>) = ids
            .iter()
            .cloned()
            .partition(|i| gates[i]["record"].get("relationship").is_some());
        components.insert(
            id.into(),
            json!({"gates":ids, "aggregate":aggregate(&ids)?,
            "local":aggregate(&local)?, "cross_component":aggregate(&cross)?}),
        );
    }
    Ok(
        json!({"schema":"harness-project-report/v1", "mode":"shadow", "project":project["id"],
        "aggregate":policy_result["aggregate"], "components":components, "gates":gates,
        "indexes":indexes, "policy_result":policy_result}),
    )
}
