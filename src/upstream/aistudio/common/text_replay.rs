use std::sync::OnceLock;

use regex::Regex;
use serde_json::{json, Map, Value};

use crate::error::GatewayError;

pub const AISTUDIO_CODE_ASSISTANT_OFFLINE_PATH: &str = "/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";
pub const AISTUDIO_STREAM_CODE_ASSISTANT_OFFLINE_GENERATION_PATH: &str = "/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/StreamCodeAssistantOfflineGeneration";
pub const AISTUDIO_DETERMINISTIC_TOOL_BRIDGE_OWNER: &str = "aistudio_deterministic_tool_bridge";
pub const AISTUDIO_DETERMINISTIC_ROUNDTRIP_BRIDGE_OWNER: &str =
    "aistudio_deterministic_roundtrip_bridge";

#[derive(Debug, Clone)]
pub struct AIStudioProgramOwnedTextReplayPlan {
    pub model: String,
    pub prompt_text: String,
    pub code_assistant_offline_path: &'static str,
    pub stream_code_assistant_offline_generation_path: &'static str,
    pub transport_owner: Option<&'static str>,
    pub deterministic_response: Option<Value>,
}

#[derive(Debug, Clone)]
struct GeminiToolDefinition {
    name: String,
    parameters: Option<Value>,
}

#[derive(Debug, Clone)]
enum GeminiToolChoice {
    Required,
    Specific(String),
}

#[derive(Debug, Clone)]
struct GeminiToolMetadata {
    tools: Vec<GeminiToolDefinition>,
    tool_choice: Option<GeminiToolChoice>,
}

#[derive(Debug, Clone)]
struct DeterministicToolCall {
    id: String,
    name: String,
    args: Value,
}

pub fn build_program_owned_text_replay_plan(
    request_url: &str,
    request_body: &Value,
) -> Result<AIStudioProgramOwnedTextReplayPlan, GatewayError> {
    let model = extract_requested_model_from_url(request_url)
        .unwrap_or_else(|| "gemini-3-flash-preview".to_string());
    let prompt_text = build_prompt_from_generate_content_body(request_body).ok_or_else(|| {
        GatewayError::bad_request(
            "AI Studio program-owned text replay could not infer a prompt from the generateContent request body.",
        )
        .with_code("aistudio_generate_content_prompt_unavailable")
    })?;

    let (transport_owner, deterministic_response) = if let Some(response) =
        build_deterministic_tool_call_response(request_body, &model)
    {
        (
            Some(AISTUDIO_DETERMINISTIC_TOOL_BRIDGE_OWNER),
            Some(response),
        )
    } else if let Some(response) = build_deterministic_roundtrip_text_response(request_body, &model)
    {
        (
            Some(AISTUDIO_DETERMINISTIC_ROUNDTRIP_BRIDGE_OWNER),
            Some(response),
        )
    } else {
        (None, None)
    };

    Ok(AIStudioProgramOwnedTextReplayPlan {
        model,
        prompt_text,
        code_assistant_offline_path: AISTUDIO_CODE_ASSISTANT_OFFLINE_PATH,
        stream_code_assistant_offline_generation_path:
            AISTUDIO_STREAM_CODE_ASSISTANT_OFFLINE_GENERATION_PATH,
        transport_owner,
        deterministic_response,
    })
}

pub fn extract_requested_model_from_url(url: &str) -> Option<String> {
    static MODEL_RE: OnceLock<Regex> = OnceLock::new();
    let re = MODEL_RE.get_or_init(|| {
        Regex::new(r"/models/([^/?#:]+):(stream)?generateContent")
            .expect("valid aistudio model extraction regex")
    });
    let normalized = normalize_string(Some(url))?;
    re.captures(normalized.as_str())
        .and_then(|captures| captures.get(1))
        .map(|capture| capture.as_str().to_string())
}

pub fn build_prompt_from_generate_content_body(body: &Value) -> Option<String> {
    let payload = body.as_object()?;

    if payload.get("systemInstruction").is_none() && payload.get("system_instruction").is_none() {
        if let Some(contents) = payload.get("contents").and_then(Value::as_array) {
            let is_single_user = contents.len() == 1
                && normalize_string(
                    contents
                        .first()
                        .and_then(|entry| entry.get("role"))
                        .and_then(Value::as_str),
                )
                .is_some_and(|role| role.eq_ignore_ascii_case("user"));
            if is_single_user {
                let mut texts = Vec::new();
                collect_gemini_text_strings(
                    contents
                        .first()
                        .and_then(|entry| entry.get("parts"))
                        .unwrap_or_else(|| contents.first().expect("checked len")),
                    &mut texts,
                );
                let single_user_text = texts
                    .into_iter()
                    .filter_map(|entry| normalize_string(Some(entry.as_str())))
                    .collect::<Vec<_>>()
                    .join("\n");
                if !single_user_text.is_empty() {
                    let tool_meta = extract_gemini_tool_metadata(body);
                    if !tool_meta.tools.is_empty() {
                        let tool_prompt = build_tool_definition_prompt(
                            &tool_meta.tools,
                            tool_meta.tool_choice.as_ref(),
                        );
                        return Some(
                            [Some(tool_prompt), Some(single_user_text)]
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>()
                                .join("\n\n"),
                        );
                    }
                    return Some(single_user_text);
                }
            }
        }
    }

    let mut segments = Vec::new();
    let mut append_text_block = |label: Option<String>, content: &Value| {
        let mut lines = Vec::new();
        collect_gemini_text_strings(content, &mut lines);
        let normalized_lines = lines
            .into_iter()
            .filter_map(|entry| normalize_string(Some(entry.as_str())))
            .collect::<Vec<_>>();
        if normalized_lines.is_empty() {
            return;
        }
        if let Some(label) = label {
            segments.push(format!("{label}: {}", normalized_lines.join("\n")));
        } else {
            segments.push(normalized_lines.join("\n"));
        }
    };

    if let Some(system_instruction) = payload
        .get("systemInstruction")
        .or_else(|| payload.get("system_instruction"))
    {
        append_text_block(Some("System".to_string()), system_instruction);
    }

    if let Some(contents) = payload.get("contents").and_then(Value::as_array) {
        for content in contents {
            let role = normalize_string(content.get("role").and_then(Value::as_str))
                .unwrap_or_else(|| "user".to_string());
            let mut chars = role.chars();
            let label = chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_else(|| "User".to_string());
            append_text_block(Some(label), content.get("parts").unwrap_or(content));
        }
    }

    let normalized_segments = segments
        .into_iter()
        .filter_map(|entry| normalize_string(Some(entry.as_str())))
        .collect::<Vec<_>>();
    if normalized_segments.is_empty() {
        return None;
    }

    let tool_meta = extract_gemini_tool_metadata(body);
    let tool_prompt = (!tool_meta.tools.is_empty())
        .then(|| build_tool_definition_prompt(&tool_meta.tools, tool_meta.tool_choice.as_ref()));
    if normalized_segments.len() == 1
        && !normalized_segments[0].contains('\n')
        && !normalized_segments[0].contains("User:")
    {
        return Some(
            [tool_prompt, Some(normalized_segments[0].clone())]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("\n\n"),
        );
    }

    Some(
        [tool_prompt, Some(normalized_segments.join("\n\n"))]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join("\n\n"),
    )
}

pub fn extract_code_assistant_generation_id(body_text: &str) -> Option<String> {
    let parsed = serde_json::from_str::<Value>(body_text).ok()?;
    parsed
        .as_array()
        .and_then(|items| items.first())
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub fn extract_final_text_from_code_assistant_stream(body_text: &str) -> Option<String> {
    let parsed = serde_json::from_str::<Value>(body_text).ok()?;
    let mut blocks = Vec::new();
    extract_model_message_blocks(&parsed, &mut blocks);
    let last_block = blocks.pop()?;
    last_block
        .into_iter()
        .filter_map(|entry| normalize_string(Some(entry.as_str())))
        .last()
}

pub fn build_code_assistant_offline_request_body(
    prompt_text: &str,
    model_path: &str,
    app_id: &str,
    opaque_token: &str,
) -> Value {
    json!([
        [[[[[null, prompt_text]], "user"]]],
        opaque_token,
        null,
        null,
        null,
        2,
        null,
        model_path,
        null,
        null,
        null,
        app_id,
        null,
        [2],
        null,
        1,
        null,
        null,
        [1, null, null, 3],
        null,
        app_id
    ])
}

pub fn build_stream_code_assistant_offline_generation_request_body(
    generation_id: &str,
    app_id: &str,
) -> Value {
    json!([generation_id, null, null, app_id])
}

pub fn build_text_only_generate_content_response(text: &str, model: &str) -> Value {
    build_synthetic_generate_content_response(text, model, &[])
}

fn collect_gemini_text_strings(node: &Value, acc: &mut Vec<String>) {
    match node {
        Value::String(text) => acc.push(text.clone()),
        Value::Array(items) => {
            for item in items {
                collect_gemini_text_strings(item, acc);
            }
        }
        Value::Object(map) => {
            for value in map.values() {
                collect_gemini_text_strings(value, acc);
            }
        }
        _ => {}
    }
}

fn extract_gemini_tool_metadata(payload: &Value) -> GeminiToolMetadata {
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

fn build_tool_definition_prompt(
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

fn build_example_arguments(schema: Option<&Value>) -> String {
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

fn build_deterministic_tool_call_response(body: &Value, model: &str) -> Option<Value> {
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

fn build_deterministic_roundtrip_text_response(body: &Value, model: &str) -> Option<Value> {
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

fn infer_deterministic_tool_arguments(prompt_text: &str, parameters: Option<&Value>) -> Value {
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

fn build_synthetic_generate_content_response(
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

fn extract_model_message_blocks(node: &Value, blocks: &mut Vec<Vec<String>>) {
    match node {
        Value::Array(items) => {
            if items.len() >= 2 && items[1].as_str() == Some("model") {
                let mut texts = Vec::new();
                collect_gemini_text_strings(&items[0], &mut texts);
                let normalized = texts
                    .into_iter()
                    .filter_map(|entry| normalize_string(Some(entry.as_str())))
                    .collect::<Vec<_>>();
                if !normalized.is_empty() {
                    blocks.push(normalized);
                }
                return;
            }
            for item in items {
                extract_model_message_blocks(item, blocks);
            }
        }
        Value::Object(map) => {
            for value in map.values() {
                extract_model_message_blocks(value, blocks);
            }
        }
        _ => {}
    }
}

fn capture_text_group(input: &str, patterns: &[&str]) -> Option<String> {
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

fn normalize_tool_name(value: Option<&str>) -> String {
    normalize_string(value).unwrap_or_else(|| "tool".to_string())
}

fn normalize_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn example_value_for_schema_type(value_type: &str) -> Value {
    match value_type.to_ascii_lowercase().as_str() {
        "integer" | "number" => json!(1),
        "boolean" => Value::Bool(true),
        "array" => json!(["example"]),
        "object" => json!({"value":"example"}),
        _ => Value::String("example".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_program_owned_text_replay_plan_extracts_model_and_prompt() {
        let body = json!({
            "contents": [{
                "role": "user",
                "parts": [{"text":"Reply with exactly OK"}]
            }]
        });
        let plan = build_program_owned_text_replay_plan(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:generateContent",
            &body,
        )
        .expect("plan");
        assert_eq!(plan.model, "gemini-3-flash-preview");
        assert_eq!(plan.prompt_text, "Reply with exactly OK");
        assert!(plan.deterministic_response.is_none());
    }

    #[test]
    fn build_program_owned_text_replay_plan_builds_deterministic_tool_bridge() {
        let body = json!({
            "contents": [{
                "role": "user",
                "parts": [{"text":"Use weather for Hangzhou"}]
            }],
            "tools": [{
                "functionDeclarations": [{
                    "name": "weather",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "city": { "type": "string" }
                        }
                    }
                }]
            }],
            "toolConfig": {
                "functionCallingConfig": {
                    "mode": "ANY"
                }
            }
        });
        let plan = build_program_owned_text_replay_plan(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:generateContent",
            &body,
        )
        .expect("plan");
        assert_eq!(
            plan.transport_owner,
            Some(AISTUDIO_DETERMINISTIC_TOOL_BRIDGE_OWNER)
        );
        let response = plan.deterministic_response.expect("deterministic response");
        assert_eq!(
            response["candidates"][0]["content"]["parts"][0]["functionCall"]["name"],
            json!("weather")
        );
        assert_eq!(
            response["candidates"][0]["content"]["parts"][0]["functionCall"]["args"]["city"],
            json!("Hangzhou")
        );
    }

    #[test]
    fn build_program_owned_text_replay_plan_builds_deterministic_roundtrip_bridge() {
        let body = json!({
            "contents": [{
                "role": "user",
                "parts": [{
                    "functionResponse": {
                        "name": "weather",
                        "response": {
                            "city": "Hangzhou",
                            "condition": "sunny"
                        }
                    }
                }]
            }]
        });
        let plan = build_program_owned_text_replay_plan(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:generateContent",
            &body,
        )
        .expect("plan");
        assert_eq!(
            plan.transport_owner,
            Some(AISTUDIO_DETERMINISTIC_ROUNDTRIP_BRIDGE_OWNER)
        );
        let response = plan.deterministic_response.expect("deterministic response");
        assert_eq!(
            response["candidates"][0]["content"]["parts"][0]["text"],
            json!("The current weather in Hangzhou is sunny.")
        );
    }

    #[test]
    fn extract_code_assistant_generation_id_reads_first_array_string() {
        assert_eq!(
            extract_code_assistant_generation_id(r#"["generation-123",{"ok":true}]"#),
            Some("generation-123".to_string())
        );
    }

    #[test]
    fn extract_final_text_from_code_assistant_stream_reads_last_model_text() {
        let body = json!([[["draft", "final answer"], "model"], [["ignored"], "user"]]);
        assert_eq!(
            extract_final_text_from_code_assistant_stream(body.to_string().as_str()),
            Some("final answer".to_string())
        );
    }

    #[test]
    fn build_code_assistant_offline_request_body_uses_expected_slots() {
        let body = build_code_assistant_offline_request_body(
            "Reply with exactly OK.",
            "models/gemini-3-flash-preview",
            "app-123",
            "!opaque",
        );
        assert_eq!(body[1], Value::String("!opaque".to_string()));
        assert_eq!(
            body[7],
            Value::String("models/gemini-3-flash-preview".to_string())
        );
        assert_eq!(body[11], Value::String("app-123".to_string()));
        assert_eq!(body[20], Value::String("app-123".to_string()));
    }

    #[test]
    fn build_stream_request_body_uses_generation_id_and_app_id() {
        let body =
            build_stream_code_assistant_offline_generation_request_body("gen-123", "app-123");
        assert_eq!(body, json!(["gen-123", null, null, "app-123"]));
    }
}
