use serde_json::Value;

pub struct JsonResult {
    pub output: String,
    pub error: Option<String>,
    pub field_count: usize,
    pub depth: usize,
}

/// Beautify (pretty-print) JSON input
pub fn beautify(input: &str) -> JsonResult {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return JsonResult {
            output: String::new(),
            error: Some("Input is empty".to_string()),
            field_count: 0,
            depth: 0,
        };
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(val) => {
            let count = count_fields(&val);
            let depth = max_depth(&val);
            let pretty = serde_json::to_string_pretty(&val)
                .unwrap_or_else(|_| "Serialization error".to_string());
            JsonResult {
                output: pretty,
                error: None,
                field_count: count,
                depth,
            }
        }
        Err(e) => JsonResult {
            output: String::new(),
            error: Some(format!("Parse error: {}", e)),
            field_count: 0,
            depth: 0,
        },
    }
}

/// Minify JSON input
pub fn minify(input: &str) -> JsonResult {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return JsonResult {
            output: String::new(),
            error: Some("Input is empty".to_string()),
            field_count: 0,
            depth: 0,
        };
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(val) => {
            let count = count_fields(&val);
            let depth = max_depth(&val);
            let minified = serde_json::to_string(&val)
                .unwrap_or_else(|_| "Serialization error".to_string());
            JsonResult {
                output: minified,
                error: None,
                field_count: count,
                depth,
            }
        }
        Err(e) => JsonResult {
            output: String::new(),
            error: Some(format!("Parse error: {}", e)),
            field_count: 0,
            depth: 0,
        },
    }
}

/// Count total fields/values in a JSON value
fn count_fields(val: &Value) -> usize {
    match val {
        Value::Object(map) => {
            map.len() + map.values().map(count_fields).sum::<usize>()
        }
        Value::Array(arr) => {
            arr.len() + arr.iter().map(count_fields).sum::<usize>()
        }
        _ => 0,
    }
}

/// Compute maximum nesting depth
fn max_depth(val: &Value) -> usize {
    match val {
        Value::Object(map) => {
            1 + map.values().map(max_depth).max().unwrap_or(0)
        }
        Value::Array(arr) => {
            1 + arr.iter().map(max_depth).max().unwrap_or(0)
        }
        _ => 0,
    }
}

/// Generate a JSON object summary (top-level keys and types)
pub fn summarize(input: &str) -> String {
    let trimmed = input.trim();
    match serde_json::from_str::<Value>(trimmed) {
        Ok(val) => build_summary(&val, 0),
        Err(e) => format!("Error: {}", e),
    }
}

fn build_summary(val: &Value, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    match val {
        Value::Object(map) => {
            let mut lines = vec![format!("{}{{", pad)];
            for (k, v) in map {
                let type_hint = type_of(v);
                match v {
                    Value::Object(_) | Value::Array(_) => {
                        lines.push(format!("{}  \"{}\": {} →", pad, k, type_hint));
                        lines.push(build_summary(v, indent + 2));
                    }
                    _ => {
                        lines.push(format!("{}  \"{}\": {} = {}", pad, k, type_hint, short_repr(v)));
                    }
                }
            }
            lines.push(format!("{}}}", pad));
            lines.join("\n")
        }
        Value::Array(arr) => {
            let mut lines = vec![format!("{}[ ({} items)", pad, arr.len())];
            for (i, v) in arr.iter().take(3).enumerate() {
                lines.push(format!("{}  [{}]: {}", pad, i, short_repr(v)));
            }
            if arr.len() > 3 {
                lines.push(format!("{}  ... {} more", pad, arr.len() - 3));
            }
            lines.push(format!("{}]", pad));
            lines.join("\n")
        }
        _ => format!("{}{}", pad, short_repr(val)),
    }
}

#[allow(dead_code)]
fn type_of(val: &Value) -> &'static str {
    match val {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[allow(dead_code)]
fn short_repr(val: &Value) -> String {
    match val {
        Value::String(s) => {
            if s.len() > 40 {
                format!("\"{}…\"", &s[..40])
            } else {
                format!("\"{}\"", s)
            }
        }
        _ => {
            let s = val.to_string();
            if s.len() > 50 {
                format!("{}…", &s[..50])
            } else {
                s
            }
        }
    }
}
