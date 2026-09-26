use serde_json::{json, Value};

use crate::protocol::canonical::CanonicalTool;
use crate::protocol::tool_choice::{self, CanonicalToolChoice};

use super::history::xml_escape;

// ---------------------------------------------------------------------------
// Part 1: Build XML tool injection prompt
// ---------------------------------------------------------------------------

/// Generate the XML tool definition prompt to inject into the system message.
///
/// Produces a self-contained prompt block that teaches the model the available
/// tools and the exact XML format it should use to call them.  Compatible with
/// the ds2api/Toolify convention.
pub fn build_tool_injection_prompt(tools: &[CanonicalTool], tool_choice: Option<&Value>) -> String {
    let mut prompt = String::from("You have access to these tools:\n\n<tools>\n");

    for tool in tools {
        let name = tool.name.as_deref().unwrap_or("unnamed");
        prompt.push_str(&format!("<tool name=\"{}\">\n", xml_escape(name)));

        if let Some(desc) = &tool.description {
            prompt.push_str(&format!("Description: {}\n", desc));
        }

        if let Some(schema) = &tool.input_schema {
            let params = format_schema_params(schema);
            if !params.is_empty() {
                prompt.push_str("Parameters:\n");
                prompt.push_str(&params);
            }
        }

        prompt.push_str("</tool>\n");
    }

    prompt.push_str("</tools>\n\n");

    prompt.push_str(
        "TOOL CALL FORMAT — FOLLOW EXACTLY:\n\
         When you need to call tools, output ONLY the following XML format:\n\
         <tool_calls>\n\
         <tool_call>\n\
         <tool_name>TOOL_NAME</tool_name>\n\
         <parameters>{\"key\": \"value\"}</parameters>\n\
         </tool_call>\n\
         </tool_calls>\n\n\
         RULES:\n\
         1. Output the XML exactly as shown — no markdown fences, no extra text after XML\n\
         2. <parameters> must contain valid JSON\n\
         3. Multiple tool calls go inside one <tool_calls> block\n\
         4. If you don't need to call a tool, respond normally with text\n\
         5. Do NOT mix tool calls with regular text in the same response\n\
         6. Do NOT use native tool markers like <\u{ff5c}Tool\u{ff5c}> or role markers",
    );

    if let Some(example) = build_tool_call_example(tools, tool_choice) {
        prompt.push_str("\n\nEXAMPLE OUTPUT:\n");
        prompt.push_str(&example);
        prompt.push_str(
            "\nDo NOT output placeholder words like `TOOL_NAME`, `parameters`, `key`, or `value` by themselves.",
        );
    }

    if let Some(guidance) = build_tool_choice_guidance(tool_choice) {
        prompt.push_str("\n\nTOOL CHOICE REQUIREMENT:\n");
        prompt.push_str(&guidance);
    }

    prompt
}

fn build_tool_choice_guidance(tool_choice: Option<&Value>) -> Option<String> {
    match tool_choice::parse_tool_choice(tool_choice) {
        Some(CanonicalToolChoice::Auto) | None => None,
        Some(CanonicalToolChoice::None) => Some(
            "Do not call any tool. Respond with plain text unless later instructions explicitly re-enable tool use."
                .to_string(),
        ),
        Some(CanonicalToolChoice::Required) => Some(
            "You MUST call at least one tool before giving any final answer. Do not answer directly with plain text before emitting a <tool_calls> block."
                .to_string(),
        ),
        Some(CanonicalToolChoice::Specific(name)) => Some(format!(
            "You MUST call only the tool `{name}` before giving any final answer. Do not call any other tool. Do not answer directly with plain text before emitting a <tool_calls> block."
        )),
        Some(CanonicalToolChoice::PromptOnly) => None,
    }
}

fn build_tool_call_example(tools: &[CanonicalTool], tool_choice: Option<&Value>) -> Option<String> {
    let preferred_name = match tool_choice::parse_tool_choice(tool_choice) {
        Some(CanonicalToolChoice::Specific(name)) => Some(name),
        _ => None,
    };

    let tool = preferred_name
        .as_deref()
        .and_then(|name| tools.iter().find(|tool| tool.name.as_deref() == Some(name)))
        .or_else(|| tools.first())?;

    let tool_name = tool.name.as_deref().unwrap_or("tool");
    let example_args = build_example_arguments(tool.input_schema.as_ref());
    Some(format!(
        "<tool_calls>\n<tool_call>\n<tool_name>{}</tool_name>\n<parameters>{}</parameters>\n</tool_call>\n</tool_calls>",
        xml_escape(tool_name),
        xml_escape(&example_args),
    ))
}

fn build_example_arguments(schema: Option<&Value>) -> String {
    let mut object = serde_json::Map::new();
    let Some(schema) = schema else {
        object.insert("value".to_string(), Value::String("example".to_string()));
        return Value::Object(object).to_string();
    };

    let required: Vec<&str> = schema
        .get("required")
        .and_then(|value| value.as_array())
        .map(|value| value.iter().filter_map(|entry| entry.as_str()).collect())
        .unwrap_or_default();

    let properties = schema
        .get("properties")
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default();

    let candidate_keys: Vec<String> = if required.is_empty() {
        properties.keys().cloned().take(2).collect()
    } else {
        required.iter().map(|key| (*key).to_string()).collect()
    };

    for key in candidate_keys {
        let value = properties
            .get(&key)
            .and_then(|property| property.get("type"))
            .and_then(|value| value.as_str())
            .map(example_value_for_type)
            .unwrap_or_else(|| Value::String("example".to_string()));
        object.insert(key, value);
    }

    if object.is_empty() {
        object.insert("value".to_string(), Value::String("example".to_string()));
    }

    Value::Object(object).to_string()
}

fn example_value_for_type(value_type: &str) -> Value {
    match value_type {
        "integer" | "number" => json!(1),
        "boolean" => Value::Bool(true),
        "array" => json!(["example"]),
        "object" => json!({"value": "example"}),
        _ => Value::String("example".to_string()),
    }
}

/// Format JSON Schema properties into a human-readable parameter list.
fn format_schema_params(schema: &Value) -> String {
    let mut out = String::new();

    let properties = match schema.get("properties").and_then(|p| p.as_object()) {
        Some(p) => p,
        None => return out,
    };

    let required: Vec<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    for (name, prop) in properties {
        let ty = prop.get("type").and_then(|t| t.as_str()).unwrap_or("any");
        let req_str = if required.contains(&name.as_str()) {
            "required"
        } else {
            "optional"
        };
        let desc = prop
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("");

        if desc.is_empty() {
            out.push_str(&format!("- {} ({}, {})\n", name, ty, req_str));
        } else {
            out.push_str(&format!("- {} ({}, {}): {}\n", name, ty, req_str, desc));
        }
    }

    out
}
