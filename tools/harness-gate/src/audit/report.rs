use super::{ArchViolation, Config, Violation};
use serde::Serialize;

pub(super) fn generate_markdown(
    config: &Config,
    hard_violations: &[Violation],
    arch_violations: &[ArchViolation],
) -> String {
    let mut output = String::new();
    let occurrence_limit = config.engine.markdown_occurrences_per_rule;

    output.push_str("=== 【自动化硬性约束扫描结果】 ===\n\n");

    for rule in &config.hard_rules {
        let rule_violations: Vec<&Violation> = hard_violations
            .iter()
            .filter(|v| v.rule_name.starts_with(&rule.name))
            .collect();

        let count = rule_violations.len();
        output.push_str(&format!(">> {}: 违规数量 {}\n", rule.name, count));

        if count > 0 {
            for v in rule_violations.iter().take(occurrence_limit) {
                output.push_str(&format!(
                    "    {}:{}: {}\n",
                    v.file.display(),
                    v.line,
                    v.content
                ));
            }
            if count > occurrence_limit {
                output.push_str(&format!("    ... 剩余 {} 处\n", count - occurrence_limit));
            }
        } else {
            output.push_str("  ✅ 未发现\n");
        }
        output.push('\n');
    }

    output.push_str("=== 【架构分层违规预扫描】 ===\n\n");

    for rule in &config.arch_rules {
        let violations: Vec<&ArchViolation> = arch_violations
            .iter()
            .filter(|v| v.rule_name == rule.name)
            .collect();
        let count = violations.len();

        output.push_str(&format!(">> {}: 违规数量 {}\n", rule.name, count));

        if count > 0 {
            for v in violations.iter().take(occurrence_limit) {
                output.push_str(&format!(
                    "    {}:{}: {}\n",
                    v.file.display(),
                    v.line,
                    v.content
                ));
            }
            if count > occurrence_limit {
                output.push_str(&format!("    ... 剩余 {} 处\n", count - occurrence_limit));
            }
            output.push_str(&format!("  💡 建议: {}\n", rule.suggestion));
        } else {
            output.push_str("  ✅ 未发现违规\n");
        }
        output.push('\n');
    }

    output
}

#[derive(Debug, Serialize)]
struct JsonOccurrence {
    file: String,
    line: usize,
    content: String,
}

#[derive(Debug, Serialize)]
struct JsonViolation {
    rule: String,
    severity: String,
    count: usize,
    occurrences: Vec<JsonOccurrence>,
}

#[derive(Debug, Serialize)]
struct JsonArchViolation {
    rule: String,
    layer: String,
    count: usize,
    suggestion: String,
    occurrences: Vec<JsonOccurrence>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct JsonSummary {
    pub(super) total_violations: usize,
    pub(super) blocker_count: usize,
    pub(super) error_count: usize,
    pub(super) warning_count: usize,
}

#[derive(Debug, Serialize)]
pub(super) struct JsonReport {
    timestamp: String,
    hard_violations: Vec<JsonViolation>,
    arch_violations: Vec<JsonArchViolation>,
    pub(super) summary: JsonSummary,
}

pub(super) fn generate_report(
    config: &Config,
    hard_violations: &[Violation],
    arch_violations: &[ArchViolation],
) -> JsonReport {
    let mut hard_json = Vec::new();
    for rule in &config.hard_rules {
        let rule_violations: Vec<&Violation> = hard_violations
            .iter()
            .filter(|v| v.rule_name.starts_with(&rule.name))
            .collect();

        let occurrences: Vec<JsonOccurrence> = rule_violations
            .iter()
            .map(|v| JsonOccurrence {
                file: v.file.to_string_lossy().to_string(),
                line: v.line,
                content: v.content.clone(),
            })
            .collect();

        hard_json.push(JsonViolation {
            rule: rule.name.clone(),
            severity: rule.severity.clone(),
            count: occurrences.len(),
            occurrences,
        });
    }

    let mut arch_json = Vec::new();
    for rule in &config.arch_rules {
        let rule_violations: Vec<&ArchViolation> = arch_violations
            .iter()
            .filter(|v| v.rule_name == rule.name)
            .collect();

        let occurrences: Vec<JsonOccurrence> = rule_violations
            .iter()
            .map(|v| JsonOccurrence {
                file: v.file.to_string_lossy().to_string(),
                line: v.line,
                content: v.content.clone(),
            })
            .collect();

        arch_json.push(JsonArchViolation {
            rule: rule.name.clone(),
            layer: rule.layer.clone(),
            count: occurrences.len(),
            suggestion: rule.suggestion.clone(),
            occurrences,
        });
    }

    let total: usize = hard_json.iter().map(|v| v.count).sum::<usize>()
        + arch_json.iter().map(|v| v.count).sum::<usize>();
    let blocker_count: usize = hard_json
        .iter()
        .filter(|v| v.severity == "blocker")
        .map(|v| v.count)
        .sum();
    let error_count: usize = hard_json
        .iter()
        .filter(|v| v.severity == "error")
        .map(|v| v.count)
        .sum();
    let warning_count: usize = hard_json
        .iter()
        .filter(|v| v.severity == "warning")
        .map(|v| v.count)
        .sum();

    JsonReport {
        timestamp: chrono::Utc::now().to_rfc3339(),
        hard_violations: hard_json,
        arch_violations: arch_json,
        summary: JsonSummary {
            total_violations: total,
            blocker_count,
            error_count,
            warning_count,
        },
    }
}
