//! Runtime support for LLM tools.

use serde::Serialize;
use serde_json::Value;
use regex::Regex;
use thiserror::Error;

// ============ Rig Framework Integration ============

/// Distributed slice for auto-registering Rig-enabled tools.
///
/// Any tool defined with `#[tool(embedding_doc = "...")]` will automatically
/// be added to this slice via the `linkme` crate.
///
/// Note: We use ToolDyn for dynamic dispatch since ToolEmbedding is not dyn-compatible.
#[linkme::distributed_slice]
pub static RIG_TOOLS: [fn() -> Box<dyn rig::tool::ToolDyn>];

/// Get all tools that support Rig's ToolEmbedding trait.
///
/// This returns only tools that were defined with the `embedding_doc` attribute
/// in the `#[tool(...)]` macro.
pub fn get_tools() -> Vec<Box<dyn rig::tool::ToolDyn>> {
    RIG_TOOLS.iter().map(|f| f()).collect()
}

// ============ Core Tool Traits ============

/// Complete tool definition for Ollama API.
#[derive(Serialize, Clone, Debug)]
pub struct ToolDefinition {
    pub r#type: &'static str,
    pub function: FunctionDefinition,
}

/// Function-specific definition.
#[derive(Serialize, Clone, Debug)]
pub struct FunctionDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_description: Option<&'static str>,
}

/// Output from tool execution.
#[derive(Debug, Clone, Serialize)]
pub struct ToolOutput {
    pub tool_name: String,
    pub value: Value,
    pub description: String,
}

/// Errors during tool execution.
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("invalid parameters: {0}")]
    InvalidParams(String),
    #[error("execution failed: {0}")]
    Execution(String),
    #[error("serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Trait for tool implementations.
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn parameters_schema(&self) -> Value;
    fn result_description(&self) -> Option<&'static str>;
    fn execute(&self, args: Value) -> Result<ToolOutput, ToolError>;
}

/// Formats a value using a template string.
pub fn format_template<T: Serialize>(value: &T, template: &str) -> String {
    let json = serde_json::to_value(value).unwrap_or_default();
    let mut placeholders = Vec::new();
    let mut depth = 0;
    let mut start = None;
    let chars: Vec<(usize, char)> = template.char_indices().collect();

    for (byte_idx, c) in chars.iter() {
        if *c == '{' {
            if depth == 0 {
                start = Some(*byte_idx);
            }
            depth += 1;
        } else if *c == '}' {
            depth -= 1;
            if depth == 0 {
                if let Some(s) = start {
                    placeholders.push((s, byte_idx + c.len_utf8()));
                }
                start = None;
            }
        }
    }

    let mut result = template.to_string();
    for (start, end) in placeholders.iter().rev() {
        let full = &template[*start..*end];
        let expr = &template[*start + 1..*end - 1];
        let replacement = eval_expr(&json, expr);
        result = result.replacen(full, &replacement, 1);
    }

    result
}

fn eval_expr(json: &Value, expr: &str) -> String {
    if let Some((field, inner)) = parse_optional(expr) {
        return eval_optional(json, &field, &inner);
    }
    if let Some((field, cases)) = parse_switch(expr) {
        return eval_switch(json, &field, &cases);
    }
    if let Some((path, default)) = parse_default(expr) {
        return eval_default(json, &path, &default);
    }
    if let Some((path, fmt)) = parse_formatted(expr) {
        let val = get_nested(json, &path);
        return format_value(val, Some(&fmt));
    }
    let path: Vec<String> = expr.split('.').map(|s| s.to_string()).collect();
    let val = get_nested(json, &path);
    format_value(val, None)
}

fn parse_optional(expr: &str) -> Option<(String, String)> {
    let re = Regex::new(r"^([.\w]+)\?:\s*(.+)$").ok()?;
    let cap = re.captures(expr)?;
    Some((cap[1].to_string(), cap[2].to_string()))
}

fn parse_switch(expr: &str) -> Option<(String, Vec<(String, String)>)> {
    let re = Regex::new(r"^([.\w]+):switch\|(.+)$").ok()?;
    let cap = re.captures(expr)?;
    let field = cap[1].to_string();
    let cases_str = cap[2].to_string();
    let cases: Vec<(String, String)> = cases_str
        .split('|')
        .filter_map(|case| {
            let parts: Vec<&str> = case.splitn(2, ':').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), parts[1].to_string()))
            } else {
                None
            }
        })
        .collect();
    Some((field, cases))
}

fn parse_default(expr: &str) -> Option<(Vec<String>, String)> {
    let re = Regex::new(r"^([.\w]+):default\|(.+)$").ok()?;
    let cap = re.captures(expr)?;
    let path: Vec<String> = cap[1].split('.').map(|s| s.to_string()).collect();
    let default = cap[2].to_string();
    Some((path, default))
}

fn parse_formatted(expr: &str) -> Option<(Vec<String>, String)> {
    let re = Regex::new(r"^([.\w]+):([a-z_]+)$").ok()?;
    let cap = re.captures(expr)?;
    let path: Vec<String> = cap[1].split('.').map(|s| s.to_string()).collect();
    let fmt = cap[2].to_string();
    if ["switch", "default"].contains(&fmt.as_str()) {
        return None;
    }
    Some((path, fmt))
}

fn get_nested<'a>(json: &'a Value, path: &[String]) -> Option<&'a Value> {
    let mut current = json;
    for part in path {
        current = current.get(part)?;
    }
    Some(current)
}

fn format_value(val: Option<&Value>, fmt: Option<&str>) -> String {
    let v = match val {
        Some(v) => v,
        None => return "[missing]".to_string(),
    };
    match (v, fmt) {
        (Value::Number(n), Some("c")) => format!("{}°C", n),
        (Value::Number(n), Some("f")) => format!("{}°F", n),
        (Value::Number(n), Some("k")) => format!("{}K", n),
        (Value::Number(n), Some("pct")) => format!("{}%", n),
        (Value::Number(n), Some("round")) => format!("{:.0}", n.as_f64().unwrap_or(0.0)),
        (Value::Number(n), Some("prec")) => format!("{:.2}", n.as_f64().unwrap_or(0.0)),
        (Value::String(s), _) => s.clone(),
        (Value::Number(n), _) => n.to_string(),
        (Value::Bool(b), _) => if *b { "true" } else { "false" }.to_string(),
        (Value::Null, _) => "null".to_string(),
        (Value::Array(arr), _) => {
            if arr.len() <= 3 {
                let items: Vec<_> = arr.iter().map(|v| format_value(Some(v), None)).collect();
                format!("[{}]", items.join(", "))
            } else {
                format!("{} items", arr.len())
            }
        }
        (Value::Object(_), _) => "[object]".to_string(),
    }
}

fn eval_optional(json: &Value, field: &str, inner: &str) -> String {
    let path: Vec<&str> = field.split('.').collect();
    let val = get_nested_str(json, &path);
    match val {
        Some(Value::Null) | None => String::new(),
        Some(Value::Bool(false)) => String::new(),
        Some(Value::String(s)) if s.is_empty() => String::new(),
        Some(v) => {
            let re = Regex::new(r"\{([.\w]+)\}").unwrap();
            let mut result = inner.to_string();
            for cap in re.captures_iter(inner) {
                let full = &cap[0];
                let f = &cap[1];
                if f == field || f.ends_with(field) {
                    result = result.replace(full, &format_value(Some(v), None));
                }
            }
            result
        }
    }
}

fn get_nested_str<'a>(json: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = json;
    for part in path {
        current = current.get(part)?;
    }
    Some(current)
}

fn eval_switch(json: &Value, field: &str, cases: &[(String, String)]) -> String {
    let path: Vec<&str> = field.split('.').collect();
    let val = get_nested_str(json, &path);
    let key = match val {
        Some(Value::String(s)) => s.as_str(),
        Some(Value::Number(n)) => return n.to_string(),
        Some(Value::Bool(b)) => if *b { "true" } else { "false" },
        _ => return String::new(),
    };
    cases
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| key.to_string())
}

fn eval_default(json: &Value, path: &[String], default: &str) -> String {
    match get_nested(json, path) {
        Some(Value::Null) => default.to_string(),
        Some(v) => format_value(Some(v), None),
        None => default.to_string(),
    }
}

// ============ Built-in Tools ============

pub mod builtin_tools;
