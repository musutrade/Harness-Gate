use anyhow::Result;
use serde_json::Value;
use std::collections::VecDeque;
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
    let mut error_trace_id = String::new();
    let mut last_trace_id = String::new();
    let file = File::open(input_path)?;
    for line in BufReader::new(file).lines() {
        let line = line?;
        let Ok(json) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if let Some(tid) = extract_trace_id(&json) {
            last_trace_id = tid.clone();
            // 优先取第一条 ERROR 日志所在的 trace_id（比"最后一条"可靠）
            if error_trace_id.is_empty() && level_of(&json) == "ERROR" {
                error_trace_id = tid;
            }
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

    let mut before = VecDeque::with_capacity(20);
    let mut selected = Vec::with_capacity(30);
    let mut tail = VecDeque::with_capacity(30);
    let mut error_seen = false;
    let file = File::open(input_path)?;
    for line in BufReader::new(file).lines() {
        let line = line?;
        let Ok(log) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if extract_trace_id(&log).as_deref() != Some(target_trace_id.as_str()) {
            continue;
        }
        let compact = compact_log(&log, &target_trace_id);
        if error_seen {
            if selected.len() < 30 {
                selected.push(compact);
            }
            continue;
        }
        if level_of(&log) == "ERROR" {
            error_seen = true;
            selected = before.drain(..).collect::<Vec<_>>();
            selected.push(compact);
        } else {
            if before.len() == 20 {
                before.pop_front();
            }
            before.push_back(compact.clone());
            if tail.len() == 30 {
                tail.pop_front();
            }
            tail.push_back(compact);
        }
    }

    let mut output = if error_seen {
        selected
    } else {
        tail.into_iter().collect()
    };
    output.truncate(30);

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

fn compact_log(log: &Value, trace_id: &str) -> Value {
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
            v.as_str()
                .map(str::to_string)
                .unwrap_or_else(|| v.to_string())
        })
        .unwrap_or_default();
    serde_json::json!({
        "timestamp": timestamp,
        "level": level,
        "target": target,
        "msg": msg,
        "error": error,
        "trace_id": trace_id,
    })
}

fn get_last_n_lines(path: &str, n: usize) -> Result<String> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut lines = VecDeque::with_capacity(n);
    for line in reader.lines() {
        if lines.len() == n {
            lines.pop_front();
        }
        lines.push_back(line?);
    }
    Ok(lines.into_iter().collect::<Vec<_>>().join("\n"))
}
