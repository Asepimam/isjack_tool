use serde_json::Value;

pub struct JsonResult {
    pub output:      String,
    pub error:       Option<String>,
    pub field_count: usize,
    pub depth:       usize,
}

/// Beautify (pretty-print) JSON input.
/// Returns a valid result for empty input and bare values like `{}` / `[]`.
pub fn beautify(input: &str) -> JsonResult {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return JsonResult {
            output:      "  (empty — paste JSON here, then press Space)".to_string(),
            error:       None,
            field_count: 0,
            depth:       0,
        };
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(val) => {
            let count = count_fields(&val);
            let depth = max_depth(&val);
            let out   = serde_json::to_string_pretty(&val)
                .unwrap_or_else(|e| format!("Serialization error: {}", e));
            JsonResult { output: out, error: None, field_count: count, depth }
        }
        Err(e) => {
            // Give a friendly hint with the position
            let hint = format!(
                "Parse error at {}: {}\n\nTip: check for missing quotes, trailing commas, or unmatched brackets.",
                e.line(), e
            );
            JsonResult { output: String::new(), error: Some(hint), field_count: 0, depth: 0 }
        }
    }
}

/// Minify JSON input.
pub fn minify(input: &str) -> JsonResult {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return JsonResult {
            output:      "  (empty — paste JSON here, then press Space)".to_string(),
            error:       None,
            field_count: 0,
            depth:       0,
        };
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(val) => {
            let count = count_fields(&val);
            let depth = max_depth(&val);
            let out   = serde_json::to_string(&val)
                .unwrap_or_else(|e| format!("Serialization error: {}", e));
            JsonResult { output: out, error: None, field_count: count, depth }
        }
        Err(e) => {
            let hint = format!(
                "Parse error at {}: {}\n\nTip: check for missing quotes, trailing commas, or unmatched brackets.",
                e.line(), e
            );
            JsonResult { output: String::new(), error: Some(hint), field_count: 0, depth: 0 }
        }
    }
}

fn count_fields(val: &Value) -> usize {
    match val {
        Value::Object(m) => m.len() + m.values().map(count_fields).sum::<usize>(),
        Value::Array(a)  => a.len() + a.iter().map(count_fields).sum::<usize>(),
        _                => 0,
    }
}

fn max_depth(val: &Value) -> usize {
    match val {
        Value::Object(m) => 1 + m.values().map(max_depth).max().unwrap_or(0),
        Value::Array(a)  => 1 + a.iter().map(max_depth).max().unwrap_or(0),
        _                => 0,
    }
}

#[allow(dead_code)]
pub fn summarize(input: &str) -> String {
    match serde_json::from_str::<Value>(input.trim()) {
        Ok(val) => build_summary(&val, 0),
        Err(e)  => format!("Error: {}", e),
    }
}

fn build_summary(val: &Value, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    match val {
        Value::Object(map) => {
            let mut lines = vec![format!("{}{{", pad)];
            for (k, v) in map {
                match v {
                    Value::Object(_) | Value::Array(_) => {
                        lines.push(format!("{}  \"{}\": {} →", pad, k, type_of(v)));
                        lines.push(build_summary(v, indent + 2));
                    }
                    _ => lines.push(format!("{}  \"{}\": {} = {}", pad, k, type_of(v), short_repr(v))),
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
            if arr.len() > 3 { lines.push(format!("{}  … {} more", pad, arr.len() - 3)); }
            lines.push(format!("{}]", pad));
            lines.join("\n")
        }
        _ => format!("{}{}", pad, short_repr(val)),
    }
}

fn type_of(val: &Value) -> &'static str {
    match val {
        Value::Null     => "null",   Value::Bool(_)   => "bool",
        Value::Number(_)=> "number", Value::String(_) => "string",
        Value::Array(_) => "array",  Value::Object(_) => "object",
    }
}

fn short_repr(val: &Value) -> String {
    match val {
        Value::String(s) => if s.len() > 40 { format!("\"{}…\"", &s[..40]) } else { format!("\"{}\"", s) },
        _ => { let s = val.to_string(); if s.len() > 50 { format!("{}…", &s[..50]) } else { s } }
    }
}
