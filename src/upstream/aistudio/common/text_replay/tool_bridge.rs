use regex::Regex;
use serde_json::{json, Map, Value};

use super::normalize_string;
use super::prompt::build_prompt_from_generate_content_body;

#[derive(Debug, Clone)]
pub(super) struct GeminiToolDefinition {
    pub(super) name: String,
    pub(super) parameters: Option<Value>,
}

#[derive(Debug, Clone)]
pub(super) enum GeminiToolChoice {
    Required,
    Specific(String),
}

#[derive(Debug, Clone)]
pub(super) struct GeminiToolMetadata {
    pub(super) tools: Vec<GeminiToolDefinition>,
    pub(super) tool_choice: Option<GeminiToolChoice>,
}

#[derive(Debug, Clone)]
pub(super) struct DeterministicToolCall {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) args: Value,
}

pub(super) fn extract_gemini_tool_metadata(payload: &Value) -> GeminiToolMetadata {
    let mut definitions = Vec::new();
    let tools = payload
        .get("tools")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for item in tools {
        let declarations = item
            .get("functionDeclarations")
            .or_else(|| item.get("function_declarations"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for declaration in declarations {
            definitions.push(GeminiToolDefinition {
                name: normalize_tool_name(declaration.get("name").and_then(Value::as_str)),
                parameters: declaration.get("parameters").cloned(),
            });
        }
    }

    let function_calling_config = payload
        .get("toolConfig")
        .or_else(|| payload.get("tool_config"))
        .and_then(|value| {
            value
                .get("functionCallingConfig")
                .or_else(|| value.get("function_calling_config"))
        });
    let mode = function_calling_config
        .and_then(|value| value.get("mode"))
        .and_then(Value::as_str)
        .and_then(|value| normalize_string(Some(value)))
        .map(|value| value.to_ascii_uppercase());
    let tool_choice = match mode.as_deref() {
        Some("ANY") => {
            let allowed_names = function_calling_config
                .and_then(|value| {
                    value
                        .get("allowedFunctionNames")
                        .or_else(|| value.get("allowed_function_names"))
                })
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if allowed_names.len() == 1 {
                Some(GeminiToolChoice::Specific(allowed_names[0].clone()))
            } else {
                Some(GeminiToolChoice::Required)
            }
        }
        _ => None,
    };

    GeminiToolMetadata {
        tools: definitions,
        tool_choice,
    }
}

pub(super) fn build_tool_definition_prompt(
    tools: &[GeminiToolDefinition],
    tool_choice: Option<&GeminiToolChoice>,
) -> String {
    let mut sections = vec!["You have access to these tools:\n\n<tools>".to_string()];
    for tool in tools {
        sections.push(format!("<tool name=\"{}\">", tool.name));
        if let Some(parameters) = tool.parameters.as_ref() {
            let properties = parameters
                .get("properties")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let required = parameters
                .get("required")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if !properties.is_empty() {
                sections.push("Parameters:".to_string());
                for (key, value) in properties {
                    let kind = value.get("type").and_then(Value::as_str).unwrap_or("any");
                    let req_label = if required.iter().any(|entry| entry == &key) {
                        "required"
                    } else {
                        "optional"
                    };
                    let desc = value.get("description").and_then(Value::as_str);
                    if let Some(desc) = desc.and_then(|value| normalize_string(Some(value))) {
                        sections.push(format!("- {key} ({kind}, {req_label}): {desc}"));
                    } else {
                        sections.push(format!("- {key} ({kind}, {req_label})"));
                    }
                }
            }
        }
        sections.push("</tool>".to_string());
    }
    sections.push("</tools>".to_string());
    sections.push(
        "TOOL CALL FORMAT — FOLLOW EXACTLY:\nWhen you need to call tools, output ONLY the following XML format:\n<tool_calls>\n<tool_call>\n<tool_name>TOOL_NAME</tool_name>\n<parameters>{\"key\":\"value\"}</parameters>\n</tool_call>\n</tool_calls>\n\nRULES:\n1. Output the XML exactly as shown — no markdown fences, no extra text after XML\n2. <parameters> must contain valid JSON\n3. Multiple tool calls go inside one <tool_calls> block\n4. If you do not need a tool, respond normally with text\n5. Do NOT mix tool calls with regular text in the same response".to_string(),
    );
    if let Some(first_tool) = tools.first() {
        sections.push("\nEXAMPLE OUTPUT:".to_string());
        sections.push(format!(
            "<tool_calls>\n<tool_call>\n<tool_name>{}</tool_name>\n<parameters>{}</parameters>\n</tool_call>\n</tool_calls>",
            first_tool.name,
            build_example_arguments(first_tool.parameters.as_ref())
        ));
    }
    match tool_choice {
        Some(GeminiToolChoice::Required) => sections.push(
            "\nTOOL CHOICE REQUIREMENT:\nYou MUST call at least one tool before giving any final answer. Do not answer directly with plain text before emitting a <tool_calls> block.".to_string(),
        ),
        Some(GeminiToolChoice::Specific(name)) => sections.push(format!(
            "\nTOOL CHOICE REQUIREMENT:\nYou MUST call only the tool `{name}` before giving any final answer. Do not call any other tool. Do not answer directly with plain text before emitting a <tool_calls> block."
        )),
        None => {}
    }
    sections.join("\n")
}

pub(super) fn build_example_arguments(schema: Option<&Value>) -> String {
    let Some(schema) = schema else {
        return json!({"value":"example"}).to_string();
    };
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let candidate_keys = if required.is_empty() {
        properties.keys().take(2).cloned().collect::<Vec<_>>()
    } else {
        required
    };
    let mut object = Map::new();
    for key in candidate_keys {
        let value = properties
            .get(&key)
            .and_then(|property| property.get("type"))
            .and_then(Value::as_str)
            .map(example_value_for_schema_type)
            .unwrap_or_else(|| Value::String("example".to_string()));
        object.insert(key, value);
    }
    if object.is_empty() {
        object.insert("value".to_string(), Value::String("example".to_string()));
    }
    Value::Object(object).to_string()
}

pub(super) fn build_deterministic_tool_call_response(body: &Value, model: &str) -> Option<Value> {
    let tool_meta = extract_gemini_tool_metadata(body);
    let tool_choice = tool_meta.tool_choice.as_ref()?;
    if tool_meta.tools.is_empty() {
        return None;
    }
    let prompt_text = build_prompt_from_generate_content_body(body).unwrap_or_default();
    let selected_tool = match tool_choice {
        GeminiToolChoice::Specific(name) => tool_meta
            .tools
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(name))
            .cloned(),
        GeminiToolChoice::Required => tool_meta
            .tools
            .iter()
            .find(|entry| {
                prompt_text
                    .to_ascii_lowercase()
                    .contains(&entry.name.to_ascii_lowercase())
            })
            .cloned()
            .or_else(|| tool_meta.tools.first().cloned()),
    }?;
    let args = infer_deterministic_tool_arguments(&prompt_text, selected_tool.parameters.as_ref());
    Some(build_synthetic_generate_content_response(
        "",
        model,
        &[DeterministicToolCall {
            id: format!("call_{}", selected_tool.name),
            name: selected_tool.name,
            args,
        }],
    ))
}

pub(super) fn build_deterministic_roundtrip_text_response(
    body: &Value,
    model: &str,
) -> Option<Value> {
    let serialized = serde_json::to_string(body).ok()?;
    let city = capture_text_group(
        &serialized,
        &[
            r#""city"\s*:\s*"([^"]+)""#,
            r#"\bcity\b[^A-Za-z0-9]+([A-Z][a-z]+)"#,
        ],
    )?;
    let condition = capture_text_group(
        &serialized,
        &[
            r#""condition"\s*:\s*"([^"]+)""#,
            r#""weather"\s*:\s*"([^"]+)""#,
            r#"\bcondition\b[^A-Za-z0-9]+([a-z]+)"#,
        ],
    )?;
    Some(build_synthetic_generate_content_response(
        format!("The current weather in {city} is {condition}.").as_str(),
        model,
        &[],
    ))
}

pub(super) fn infer_deterministic_tool_arguments(
    prompt_text: &str,
    parameters: Option<&Value>,
) -> Value {
    let properties = parameters
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let lower_prompt = prompt_text.to_ascii_lowercase();
    let mut result = Map::new();
    for (key, value) in properties {
        if key.to_ascii_lowercase().contains("city") {
            if lower_prompt.contains("hangzhou") {
                result.insert(key, Value::String("Hangzhou".to_string()));
                continue;
            }
            if lower_prompt.contains("beijing") {
                result.insert(key, Value::String("Beijing".to_string()));
                continue;
            }
            if lower_prompt.contains("shanghai") {
                result.insert(key, Value::String("Shanghai".to_string()));
                continue;
            }
        }
        result.insert(
            key,
            example_value_for_schema_type(value.get("type").and_then(Value::as_str).unwrap_or("")),
        );
    }
    Value::Object(result)
}

pub(super) fn build_synthetic_generate_content_response(
    text: &str,
    model: &str,
    tool_calls: &[DeterministicToolCall],
) -> Value {
    let mut parts = Vec::new();
    let normalized_text = normalize_string(Some(text));
    if normalized_text.is_some() || tool_calls.is_empty() {
        parts.push(json!({"text": normalized_text.unwrap_or_default()}));
    }
    for tool_call in tool_calls {
        parts.push(json!({
            "functionCall": {
                "id": tool_call.id,
                "name": tool_call.name,
                "args": tool_call.args,
            }
        }));
    }
    json!({
        "candidates": [{
            "index": 0,
            "content": {
                "role": "model",
                "parts": parts,
            },
            "finishReason": "STOP",
        }],
        "modelVersion": model,
    })
}

pub(super) fn capture_text_group(input: &str, patterns: &[&str]) -> Option<String> {
    for pattern in patterns {
        let re = Regex::new(pattern).ok()?;
        if let Some(captures) = re.captures(input) {
            if let Some(value) = captures.get(1) {
                if let Some(normalized) = normalize_string(Some(value.as_str())) {
                    return Some(normalized);
                }
            }
        }
    }
    None
}

pub(super) fn normalize_tool_name(value: Option<&str>) -> String {
    normalize_string(value).unwrap_or_else(|| "tool".to_string())
}

pub(super) fn example_value_for_schema_type(value_type: &str) -> Value {
    match value_type.to_ascii_lowercase().as_str() {
        "integer" | "number" => json!(1),
        "boolean" => Value::Bool(true),
        "array" => json!(["example"]),
        "object" => json!({"value":"example"}),
        _ => Value::String("example".to_string()),
    }
}
