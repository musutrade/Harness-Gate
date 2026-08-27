use anyhow::Result;
use serde_json::Value;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};

fn extract_trace_id(json: &Value) -> Option<String> {
    trace_id_field(json)
        .or_else(|| json.get("fields").and_then(trace_id_field))
        .or_else(|| json.get("data").and_then(trace_id_field))
        .or_else(|| json.get("span").and_then(trace_id_field))
        .or_else(|| {
            json.get("spans")
                .and_then(Value::as_array)
                .and_then(|spans| spans.iter().rev().find_map(trace_id_field))
        })
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn trace_id_field(json: &Value) -> Option<&Value> {
    json.get("trace_id").or_else(|| json.get("request_id"))
}

fn level_of(json: &Value) -> String {
    json.get("level")
        .or_else(|| json.get("severity"))
        .and_then(|v| v.as_str())
        .unwrap_or("INFO")
        .to_uppercase()
}

pub(super) fn extract_error_context(input_path: &str, output_path: &str) -> Result<()> {
    let file = File::open(input_path)?;
    let reader = BufReader::new(file);

    let mut error_trace_id = String::new();
    let mut last_trace_id = String::new();
    let mut structured_logs: Vec<Value> = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(json) = serde_json::from_str::<Value>(&line) {
            if let Some(tid) = extract_trace_id(&json) {
                last_trace_id = tid.clone();
                // 优先取第一条 ERROR 日志所在的 trace_id（比"最后一条"可靠）
                if error_trace_id.is_empty() && level_of(&json) == "ERROR" {
                    error_trace_id = tid;
                }
            }
            structured_logs.push(json);
        }
    }

    let target_trace_id = if error_trace_id.is_empty() {
        last_trace_id
    } else {
        error_trace_id
    };

    if target_trace_id.is_empty() {
        eprintln!("⚠️ 未找到 trace_id，降级输出原始日志尾部 30 行");
        let last_lines = get_last_n_lines(input_path, 30)?;
        fs::write(output_path, last_lines)?;
        return Ok(());
    }

    let mut output = Vec::new();
    for log in &structured_logs {
        if extract_trace_id(log).as_deref() != Some(target_trace_id.as_str()) {
            continue;
        }
        let timestamp = log
            .get("timestamp")
            .or_else(|| log.get("time"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let level = level_of(log);
        let target = log
            .get("target")
            .or_else(|| log.get("module"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let fields = log.get("fields").or_else(|| log.get("data"));
        let msg = fields
            .and_then(|f| f.get("message").or_else(|| f.get("msg")))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let error = fields
            .and_then(|f| f.get("error"))
            .or_else(|| log.get("error"))
            .map(|v| {
                if let Some(s) = v.as_str() {
                    s.to_string()
                } else {
                    v.to_string()
                }
            })
            .unwrap_or_default();

        let compact = serde_json::json!({
            "timestamp": timestamp,
            "level": level,
            "target": target,
            "msg": msg,
            "error": error,
            "trace_id": target_trace_id,
        });
        output.push(compact);
    }

    // 以第一条 ERROR 为中心保留上下文，避免长请求把根因截掉。
    if output.len() > 30 {
        let error_index = output
            .iter()
            .position(|entry| entry["level"] == "ERROR")
            .unwrap_or(output.len() - 1);
        let mut start = error_index.saturating_sub(20);
        let end = (start + 30).min(output.len());
        start = end.saturating_sub(30);
        output = output[start..end].to_vec();
    }

    let json_output = serde_json::to_string_pretty(&output)?;
    fs::write(output_path, json_output)?;
    eprintln!(
        "✅ 结构化日志已提取: {} ({} 条, trace_id={})",
        output_path,
        output.len(),
        target_trace_id
    );
    Ok(())
}

fn get_last_n_lines(path: &str, n: usize) -> Result<String> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines().collect::<std::result::Result<Vec<_>, _>>()?;
    let start = lines.len().saturating_sub(n);
    Ok(lines[start..].join("\n"))
}
