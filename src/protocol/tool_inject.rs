// ---------------------------------------------------------------------------
// protocol/tool_inject.rs — XML tool call injection and extraction
//
// Enables models without native tool-calling support (DeepSeek, small models)
// to work with tool-calling clients by:
//   1. Injecting XML tool definitions into the system prompt
//   2. Stripping the `tools` field from the request
//   3. Parsing XML tool calls from the model's text response
//   4. Converting them back to standard OpenAI `tool_calls` format
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::OnceLock;

use bytes::Bytes;
use futures::Stream;
use regex::Regex;
use serde_json::{json, Value};

use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalTool, CanonicalToolCall, ContentPart,
    MessageRole,
};
use crate::protocol::tool_choice::{self, CanonicalToolChoice};

// ---------------------------------------------------------------------------
// Result type for parsed tool calls
// ---------------------------------------------------------------------------

/// Result of parsing XML tool calls from model text output.
#[derive(Debug, Clone)]
pub struct ToolCallParseResult {
    /// Extracted tool calls in canonical format.
    pub tool_calls: Vec<CanonicalToolCall>,
    /// Text with tool call XML removed.
    pub clean_text: String,
    /// Whether any tool calls were found.
    pub had_tool_calls: bool,
}

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

// ---------------------------------------------------------------------------
// Part 2: Inject tools into request
// ---------------------------------------------------------------------------

/// Inject tool definitions into a [`CanonicalRelayRequest`].
///
/// - Prepends the XML tool prompt to the system message (or creates one)
/// - Converts previous `tool_call` messages in conversation history to XML
///   format so the model understands context
/// - Converts `tool` (result) messages to text format
/// - Clears the `tools` field (model does not understand native tool format)
pub fn inject_tools(req: &mut CanonicalRelayRequest) {
    if req.tools.is_empty() {
        serialize_tool_history(req);
        return;
    }

    let tool_prompt = build_tool_injection_prompt(&req.tools, req.tool_choice.as_ref());

    // Prepend to existing system message or create new one.
    let has_system = req.messages.iter().any(|m| m.role == MessageRole::System);

    if has_system {
        for msg in &mut req.messages {
            if msg.role == MessageRole::System {
                let existing_text = msg.text_content();
                msg.content = vec![ContentPart::Text {
                    text: format!("{}\n\n{}", tool_prompt, existing_text),
                }];
                break;
            }
        }
    } else {
        req.messages.insert(
            0,
            CanonicalMessage {
                role: MessageRole::System,
                content: vec![ContentPart::Text { text: tool_prompt }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
        );
    }

    serialize_tool_history(req);

    // Clear native tools (model doesn't understand them).
    req.tools.clear();
    req.tool_choice = None;
}

/// Convert tool history into a text-only transcript for upstreams that do not
/// understand native tool wire, even when the current turn does not carry a
/// fresh `tools` array.
pub fn serialize_tool_history(req: &mut CanonicalRelayRequest) {
    let mut i = 0;
    while i < req.messages.len() {
        let role = req.messages[i].role;

        if role == MessageRole::Assistant && !req.messages[i].tool_calls.is_empty() {
            let xml = format_tool_call_as_xml(&req.messages[i].tool_calls);
            let existing = req.messages[i].text_content();
            let new_text = if existing.is_empty() {
                xml
            } else {
                format!("{}\n{}", existing, xml)
            };
            req.messages[i].content = vec![ContentPart::Text { text: new_text }];
            req.messages[i].tool_calls.clear();
        } else if role == MessageRole::Tool {
            let tool_call_id = req.messages[i].tool_call_id.clone().unwrap_or_default();
            let content_text = render_tool_result_content(&req.messages[i]);
            let formatted = format_tool_result_as_text(&tool_call_id, &content_text);

            req.messages[i].role = MessageRole::User;
            req.messages[i].content = vec![ContentPart::Text { text: formatted }];
            req.messages[i].tool_call_id = None;
        }

        i += 1;
    }
}

// ---------------------------------------------------------------------------
// Part 3: Parse XML tool calls from response text
// ---------------------------------------------------------------------------

/// Parse XML tool calls from model text output.
///
/// Supports multiple XML formats for maximum compatibility:
/// 1. `<tool_calls><tool_call><tool_name>...<parameters>...</tool_call></tool_calls>`
/// 2. `<function_calls><function_call><tool>...<args_json>...</function_call></function_calls>`
/// 3. `<invoke name="..."><parameter name="k">v</parameter></invoke>`
///
/// Returns the extracted tool calls and the remaining text (with XML stripped).
pub fn parse_tool_calls_from_text(text: &str) -> ToolCallParseResult {
    parse_tool_calls_from_text_with_context(text, &[], None, None)
}

pub fn parse_tool_calls_from_text_with_context(
    text: &str,
    tools: &[CanonicalTool],
    tool_choice: Option<&Value>,
    conversation_hint: Option<&str>,
) -> ToolCallParseResult {
    // 1. Strip <think>...</think> blocks.
    let cleaned = strip_think_blocks(text);

    // 2. Try each XML pattern in order.

    // Pattern 1: <tool_calls>...<tool_call>...<tool_name>...<parameters>...</tool_call>...</tool_calls>
    if let Some(result) =
        try_parse_tool_calls_pattern(&cleaned, tools, tool_choice, conversation_hint)
    {
        return result;
    }

    // Pattern 2: <function_calls>...<function_call>...<tool>...<args_json>...</function_call>...</function_calls>
    if let Some(result) =
        try_parse_function_calls_pattern(&cleaned, tools, tool_choice, conversation_hint)
    {
        return result;
    }

    // Pattern 3: <invoke name="...">...<parameter name="k">v</parameter>...</invoke>
    if let Some(result) = try_parse_invoke_pattern(&cleaned) {
        return result;
    }

    // No tool calls found.
    ToolCallParseResult {
        tool_calls: vec![],
        clean_text: cleaned,
        had_tool_calls: false,
    }
}

// ---------------------------------------------------------------------------
// Pattern 1: <tool_calls> / <tool_call>
// ---------------------------------------------------------------------------

fn re_tool_calls_outer() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?is)<tool_calls>\s*(.*?)\s*</tool_calls>").unwrap())
}

fn re_tool_call_inner() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?is)<tool_call>\s*(.*?)\s*</tool_call>").unwrap())
}

fn re_tool_name() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?is)<tool_name>\s*(.*?)\s*</tool_name>").unwrap())
}

fn re_parameters() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?is)<parameters>\s*(.*?)\s*</parameters>").unwrap())
}

fn re_generic_xml_tag() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?is)<([a-zA-Z_][a-zA-Z0-9_\-]*)>\s*(.*?)\s*</([a-zA-Z_][a-zA-Z0-9_\-]*)>")
            .unwrap()
    })
}

fn try_parse_tool_calls_pattern(
    text: &str,
    tools: &[CanonicalTool],
    tool_choice: Option<&Value>,
    conversation_hint: Option<&str>,
) -> Option<ToolCallParseResult> {
    let outer_re = re_tool_calls_outer();
    let (outer_start, inner_text) = if let Some(outer_match) = outer_re.find(text) {
        let captures = outer_re.captures(text)?;
        let inner_text = captures.get(1)?.as_str();
        (outer_match.start(), inner_text)
    } else {
        extract_incomplete_outer_block(text, "tool_calls")?
    };

    let mut tool_calls = Vec::new();

    for cap in re_tool_call_inner().captures_iter(inner_text) {
        let block = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let name = re_tool_name()
            .captures(block)
            .and_then(|c| c.get(1))
            .or_else(|| re_func_tool().captures(block).and_then(|c| c.get(1)))
            .map(|m| xml_unescape(m.as_str().trim()));
        let args = re_parameters()
            .captures(block)
            .and_then(|c| c.get(1))
            .map(|m| xml_unescape(m.as_str().trim()))
            .or_else(|| extract_inline_json_object(block));

        let resolved_name = resolve_tool_name(
            name,
            block,
            args.as_deref(),
            tools,
            tool_choice,
            conversation_hint,
        );
        if resolved_name.is_some() {
            tool_calls.push(make_tool_call(resolved_name, args));
        }
    }

    if tool_calls.is_empty() {
        if let Some(fallback_tool_call) = parse_malformed_tool_call_block(
            inner_text,
            tools,
            tool_choice,
            conversation_hint,
            re_tool_name(),
            re_parameters(),
        ) {
            tool_calls.push(fallback_tool_call);
        }
    }

    if tool_calls.is_empty() {
        return None;
    }

    let clean_text = text[..outer_start].trim().to_string();

    Some(ToolCallParseResult {
        tool_calls,
        clean_text,
        had_tool_calls: true,
    })
}

// ---------------------------------------------------------------------------
// Pattern 2: <function_calls> / <function_call>
// ---------------------------------------------------------------------------

fn re_func_calls_outer() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?is)<function_calls>\s*(.*?)\s*</function_calls>").unwrap())
}

fn re_func_call_inner() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?is)<function_call>\s*(.*?)\s*</function_call>").unwrap())
}

fn re_func_tool() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?is)<tool>\s*(.*?)\s*</tool>").unwrap())
}

fn re_args_json() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?is)<args_json>\s*(.*?)\s*</args_json>").unwrap())
}

fn try_parse_function_calls_pattern(
    text: &str,
    tools: &[CanonicalTool],
    tool_choice: Option<&Value>,
    conversation_hint: Option<&str>,
) -> Option<ToolCallParseResult> {
    let outer_re = re_func_calls_outer();
    let (outer_start, inner_text) = if let Some(outer_match) = outer_re.find(text) {
        let captures = outer_re.captures(text)?;
        let inner_text = captures.get(1)?.as_str();
        (outer_match.start(), inner_text)
    } else {
        extract_incomplete_outer_block(text, "function_calls")?
    };

    let mut tool_calls = Vec::new();

    for cap in re_func_call_inner().captures_iter(inner_text) {
        let block = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let name = re_func_tool()
            .captures(block)
            .and_then(|c| c.get(1))
            .map(|m| xml_unescape(m.as_str().trim()));
        let args = re_args_json()
            .captures(block)
            .and_then(|c| c.get(1))
            .map(|m| xml_unescape(m.as_str().trim()));

        let resolved_name = resolve_tool_name(
            name,
            block,
            args.as_deref(),
            tools,
            tool_choice,
            conversation_hint,
        );
        if resolved_name.is_some() {
            tool_calls.push(make_tool_call(resolved_name, args));
        }
    }

    if tool_calls.is_empty() {
        if let Some(fallback_tool_call) = parse_malformed_tool_call_block(
            inner_text,
            tools,
            tool_choice,
            conversation_hint,
            re_func_tool(),
            re_args_json(),
        ) {
            tool_calls.push(fallback_tool_call);
        }
    }

    if tool_calls.is_empty() {
        return None;
    }

    let clean_text = text[..outer_start].trim().to_string();

    Some(ToolCallParseResult {
        tool_calls,
        clean_text,
        had_tool_calls: true,
    })
}

// ---------------------------------------------------------------------------
// Pattern 3: <invoke name="...">
// ---------------------------------------------------------------------------

fn re_invoke() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?is)<invoke\s+name="([^"]+)"\s*>(.*?)</invoke>"#).unwrap())
}

fn re_param() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?is)<parameter\s+name="([^"]+)"\s*>(.*?)</parameter>"#).unwrap()
    })
}

fn try_parse_invoke_pattern(text: &str) -> Option<ToolCallParseResult> {
    let invoke_re = re_invoke();
    let all_matches: Vec<_> = invoke_re.find_iter(text).collect();

    if all_matches.is_empty() {
        return None;
    }

    let mut tool_calls = Vec::new();

    for cap in invoke_re.captures_iter(text) {
        let name = cap.get(1).map(|m| m.as_str().trim().to_string());
        let body = cap.get(2).map(|m| m.as_str()).unwrap_or("");

        // Build a JSON object from <parameter> tags.
        let mut params = serde_json::Map::new();
        for param_cap in re_param().captures_iter(body) {
            let key = param_cap
                .get(1)
                .map(|m| xml_unescape(m.as_str().trim()))
                .unwrap_or_default();
            let val = param_cap
                .get(2)
                .map(|m| xml_unescape(m.as_str().trim()))
                .unwrap_or_default();
            params.insert(key, Value::String(val));
        }

        let args_json = serde_json::to_string(&Value::Object(params)).ok();
        tool_calls.push(make_tool_call(name, args_json));
    }

    if tool_calls.is_empty() {
        return None;
    }

    // Remove all invoke blocks from the text.
    let clean_text = invoke_re.replace_all(text, "").trim().to_string();

    Some(ToolCallParseResult {
        tool_calls,
        clean_text,
        had_tool_calls: true,
    })
}

// ---------------------------------------------------------------------------
// Part 4: Model detection
// ---------------------------------------------------------------------------

/// Check if a model needs XML tool injection (doesn't support native tool calling).
///
/// Models that DO support native tools (no injection needed):
/// - `gpt-*` (OpenAI)
/// - `claude-*` (Anthropic)
/// - `o1-*`, `o3-*`, `o4-*` (OpenAI reasoning)
/// - Any model accessed via adapters with native tool support:
///   - `anthropic_compatible`
///   - `kiro_compatible`
///   - `gemini_api_compatible`
///   - `aistudio_web_reverse_compatible`
///   - `gemini_canvas_compatible`
///   - `bedrock_converse_compatible`
///   - `cohere_compatible`
///
/// Models that NEED injection:
/// - `deepseek-*`
/// - Any model not in the native-tools list
pub fn needs_tool_injection(model: &str, adapter: &str) -> bool {
    if adapter == "chatgpt_web_reverse_compatible" {
        return true;
    }

    // Native-tool adapters should never downgrade to XML tool injection.
    if matches!(
        adapter,
        "anthropic_compatible"
            | "kiro_compatible"
            | "gemini_api_compatible"
            | "gemini_api_modular_compatible"
            | "aistudio_web_reverse_compatible"
            | "gemini_canvas_compatible"
            | "gemini_canvas_web_reverse_compatible"
            | "bedrock_converse_compatible"
            | "cohere_compatible"
    ) {
        return false;
    }

    let model_lower = model.to_lowercase();

    // Models with native tool support — no injection needed.
    let native_prefixes = ["gpt-", "o1-", "o3-", "o4-", "claude-"];

    // Exact matches for short model names.
    let native_exact = ["o1", "o3", "o4"];

    for exact in &native_exact {
        if model_lower == *exact {
            return false;
        }
    }

    for prefix in &native_prefixes {
        if model_lower.starts_with(prefix) {
            return false;
        }
    }

    // Everything else needs injection.
    true
}

// ---------------------------------------------------------------------------
// Part 5: Tool history formatting helpers
// ---------------------------------------------------------------------------

/// Convert tool calls from an assistant message into XML format
/// so the model understands what tools were previously called.
fn format_tool_call_as_xml(tool_calls: &[CanonicalToolCall]) -> String {
    let mut xml = String::from("<tool_calls>\n");
    for tc in tool_calls {
        xml.push_str("<tool_call>\n");
        xml.push_str(&format!(
            "<tool_name>{}</tool_name>\n",
            xml_escape(tc.name.as_deref().unwrap_or(""))
        ));
        xml.push_str(&format!(
            "<parameters>{}</parameters>\n",
            xml_escape(tc.arguments.as_deref().unwrap_or("{}"))
        ));
        xml.push_str("</tool_call>\n");
    }
    xml.push_str("</tool_calls>");
    xml
}

/// Convert a tool result message to text format for the model.
fn format_tool_result_as_text(tool_call_id: &str, content: &str) -> String {
    format!(
        "Tool execution result (call_id: {}):\n<tool_result>\n{}\n</tool_result>",
        tool_call_id, content
    )
}

fn resolve_tool_name(
    explicit_name: Option<String>,
    block: &str,
    args: Option<&str>,
    tools: &[CanonicalTool],
    tool_choice: Option<&Value>,
    conversation_hint: Option<&str>,
) -> Option<String> {
    let explicit_name = explicit_name
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .or_else(|| extract_implicit_tool_name(block));

    match explicit_name {
        Some(name) if !is_placeholder_tool_name(&name) => Some(name),
        _ => infer_tool_name_from_context(args, tools, tool_choice, conversation_hint),
    }
}

fn extract_implicit_tool_name(block: &str) -> Option<String> {
    for cap in re_generic_xml_tag().captures_iter(block) {
        let open_tag = cap.get(1).map(|m| m.as_str())?;
        let close_tag = cap.get(3).map(|m| m.as_str())?;
        if !open_tag.eq_ignore_ascii_case(close_tag) || matches_ignored_tool_tag(open_tag) {
            continue;
        }

        return Some(open_tag.trim().to_string());
    }

    None
}

fn matches_ignored_tool_tag(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "tool_name" | "tool" | "parameters" | "args_json" | "arguments" | "function" | "name"
    )
}

fn is_placeholder_tool_name(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "" | "tool_name"
            | "tool"
            | "function"
            | "function_name"
            | "name"
            | "unnamed"
            | "unknown"
            | "parameters"
            | "args"
            | "args_json"
            | "arguments"
            | "parameter"
            | "placeholder"
    )
}

fn infer_tool_name_from_context(
    args: Option<&str>,
    tools: &[CanonicalTool],
    tool_choice: Option<&Value>,
    conversation_hint: Option<&str>,
) -> Option<String> {
    if let Some(CanonicalToolChoice::Specific(name)) = tool_choice::parse_tool_choice(tool_choice) {
        return Some(name);
    }

    let named_tools: Vec<&CanonicalTool> = tools
        .iter()
        .filter(|tool| {
            tool.name
                .as_deref()
                .is_some_and(|name| !name.trim().is_empty())
        })
        .collect();

    if named_tools.len() == 1 {
        return named_tools[0].name.clone();
    }

    let Some(args_object) = args
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .and_then(|value| value.as_object().cloned())
    else {
        return infer_tool_name_from_hint_text(tools, conversation_hint);
    };

    let arg_keys: Vec<&str> = args_object.keys().map(String::as_str).collect();
    let mut best_name: Option<String> = None;
    let mut best_score = 0usize;
    let mut tied = false;

    for tool in named_tools {
        let schema = match tool
            .input_schema
            .as_ref()
            .and_then(|schema| schema.as_object())
        {
            Some(schema) => schema,
            None => continue,
        };
        let properties = schema
            .get("properties")
            .and_then(|value| value.as_object())
            .cloned()
            .unwrap_or_default();
        if properties.is_empty() {
            continue;
        }

        let required: Vec<&str> = schema
            .get("required")
            .and_then(|value| value.as_array())
            .map(|value| value.iter().filter_map(|entry| entry.as_str()).collect())
            .unwrap_or_default();

        let mut score = 0usize;
        for key in &arg_keys {
            if properties.contains_key(*key) {
                score += 1;
                if required.contains(key) {
                    score += 2;
                }
            }
        }

        if score == 0 {
            continue;
        }

        if score > best_score {
            best_score = score;
            best_name = tool.name.clone();
            tied = false;
        } else if score == best_score {
            tied = true;
        }
    }

    if tied {
        infer_tool_name_from_hint_text(tools, conversation_hint)
    } else {
        best_name.or_else(|| infer_tool_name_from_hint_text(tools, conversation_hint))
    }
}

fn infer_tool_name_from_hint_text(
    tools: &[CanonicalTool],
    conversation_hint: Option<&str>,
) -> Option<String> {
    let hint = conversation_hint?.to_ascii_lowercase();
    let mut matches = tools
        .iter()
        .filter_map(|tool| tool.name.as_deref())
        .filter(|name| !name.trim().is_empty())
        .filter(|name| hint.contains(&name.to_ascii_lowercase()))
        .map(str::to_string)
        .collect::<Vec<_>>();
    matches.sort();
    matches.dedup();
    if matches.len() == 1 {
        matches.into_iter().next()
    } else {
        None
    }
}

fn parse_malformed_tool_call_block(
    inner_text: &str,
    tools: &[CanonicalTool],
    tool_choice: Option<&Value>,
    conversation_hint: Option<&str>,
    name_re: &Regex,
    args_re: &Regex,
) -> Option<CanonicalToolCall> {
    let explicit_name = name_re
        .captures(inner_text)
        .and_then(|captures| captures.get(1))
        .map(|value| xml_unescape(value.as_str().trim()));
    let arguments = args_re
        .captures(inner_text)
        .and_then(|captures| captures.get(1))
        .map(|value| xml_unescape(value.as_str().trim()))
        .or_else(|| extract_inline_json_object(inner_text));
    let resolved_name = resolve_tool_name(
        explicit_name,
        inner_text,
        arguments.as_deref(),
        tools,
        tool_choice,
        conversation_hint,
    );

    if resolved_name.is_none() && arguments.is_none() {
        return None;
    }

    Some(make_tool_call(
        resolved_name,
        Some(arguments.unwrap_or_else(|| "{}".to_string())),
    ))
}

fn extract_inline_json_object(text: &str) -> Option<String> {
    let mut start_index = None;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (index, ch) in text.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start_index = Some(index);
                }
                depth += 1;
            }
            '}' => {
                if depth == 0 {
                    continue;
                }
                depth -= 1;
                if depth == 0 {
                    let start = start_index?;
                    return Some(text[start..=index].trim().to_string());
                }
            }
            _ => {}
        }
    }

    None
}

fn extract_incomplete_outer_block<'a>(text: &'a str, tag_name: &str) -> Option<(usize, &'a str)> {
    let needle = format!("<{tag_name}");
    let start = text.find(&needle)?;
    let after_start = &text[start..];
    let close = after_start.find('>')?;
    let inner_start = start + close + 1;
    Some((start, &text[inner_start..]))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Escape XML special characters in text.
fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn xml_unescape(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn render_tool_result_content(message: &CanonicalMessage) -> String {
    let mut fragments = Vec::new();
    for part in &message.content {
        match part {
            ContentPart::Text { text } => {
                if !text.trim().is_empty() {
                    fragments.push(text.clone());
                }
            }
            ContentPart::Json { value } | ContentPart::Raw { value } => {
                let rendered = value
                    .get("text")
                    .and_then(|entry| entry.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| value.to_string());
                if !rendered.trim().is_empty() {
                    fragments.push(rendered);
                }
            }
            ContentPart::ImageUrl { .. } => {}
        }
    }
    fragments.join("\n")
}

/// Strip `<think>...</think>` blocks from text, handling nested tags.
fn strip_think_blocks(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut depth = 0u32;
    let mut i = 0;
    let len = text.len();

    while i < len {
        let remaining = &text[i..];
        // Check for opening <think> tag (case-insensitive).
        if remaining.starts_with('<') {
            if remaining
                .get(..7)
                .is_some_and(|tag| tag.eq_ignore_ascii_case("<think>"))
            {
                depth += 1;
                i += 7;
                continue;
            }
            // Check for closing </think> tag.
            if remaining
                .get(..8)
                .is_some_and(|tag| tag.eq_ignore_ascii_case("</think>"))
            {
                if depth > 0 {
                    depth -= 1;
                }
                i += 8;
                continue;
            }
        }

        let ch = remaining
            .chars()
            .next()
            .expect("non-empty UTF-8 remainder should contain a character");
        if depth == 0 {
            result.push(ch);
        }
        i += ch.len_utf8();
    }

    result
}

/// Create a [`CanonicalToolCall`] from parsed name and arguments.
fn make_tool_call(name: Option<String>, arguments: Option<String>) -> CanonicalToolCall {
    CanonicalToolCall {
        id: Some(format!("call_{}", uuid::Uuid::new_v4().simple())),
        call_type: "function".to_string(),
        name,
        arguments,
        raw: HashMap::new(),
    }
}

// ---------------------------------------------------------------------------
// Part 6: Streaming XML tool call detection
// ---------------------------------------------------------------------------

// XML opening tags that trigger capture mode in the streaming detector.
const TOOL_CALL_OPEN_TAGS: &[&str] = &[
    "<tool_calls>",
    "<tool_calls ",
    "<function_calls>",
    "<function_calls ",
    "<tool_call>",
    "<tool_call ",
    "<invoke ",
    "<invoke>",
];

/// Check whether `text` contains an XML opening tag that indicates the start
/// of a tool call block.
fn has_tool_call_opening(text: &str) -> bool {
    TOOL_CALL_OPEN_TAGS.iter().any(|tag| text.contains(tag))
}

/// Extract the `content` string from an OpenAI SSE `data:` line.
///
/// Expected format: `data: {"choices":[{"delta":{"content":"..."}}]}`
/// Returns `None` for `[DONE]`, empty lines, or missing content field.
fn extract_sse_content(line: &str) -> Option<String> {
    let data = line
        .strip_prefix("data:")
        .or_else(|| line.strip_prefix("data: "))?;
    let data = data.trim();
    if data == "[DONE]" || data.is_empty() {
        return None;
    }
    let parsed: Value = serde_json::from_str(data).ok()?;
    parsed
        .get("choices")?
        .get(0)?
        .get("delta")?
        .get("content")?
        .as_str()
        .map(|s| s.to_string())
}

/// Build an SSE chunk that carries OpenAI-format `tool_calls` deltas.
///
/// This produces a single SSE frame containing ALL tool calls at once
/// (matching the OpenAI convention for the first chunk that announces
/// tool calls).
fn build_tool_calls_sse_chunk(
    tool_calls: &[CanonicalToolCall],
    model: &str,
    response_id: &str,
) -> Vec<u8> {
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let tc_json: Vec<Value> = tool_calls
        .iter()
        .enumerate()
        .map(|(i, tc)| {
            json!({
                "index": i,
                "id": tc.id,
                "type": tc.call_type,
                "function": {
                    "name": tc.name,
                    "arguments": tc.arguments.as_deref().unwrap_or("{}")
                }
            })
        })
        .collect();

    let chunk = json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {
                "role": "assistant",
                "content": null,
                "tool_calls": tc_json,
            },
            "finish_reason": "tool_calls",
        }]
    });

    format!("data: {}\n\n", chunk).into_bytes()
}

/// Wrap a byte stream to detect and translate XML tool calls.
///
/// When `tools_were_injected` is true, the model is expected to output
/// tool calls as its entire response (not mixed with regular text).
/// This wrapper accumulates ALL text content from the SSE stream, then
/// on stream end checks for XML tool calls.  If found, it emits
/// OpenAI-format `tool_calls` chunks + `[DONE]` instead of the raw text.
/// If no tool calls are found, it replays the original chunks unmodified.
///
/// This "accumulate then decide" strategy is correct because tool-injected
/// models output tool calls as their ENTIRE response.
pub fn wrap_streaming_tool_detection(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
    response_id: String,
    tools: Vec<CanonicalTool>,
    tool_choice: Option<Value>,
    conversation_hint: Option<String>,
) -> std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static>> {
    let state = ToolDetectorState {
        model,
        response_id,
        tools,
        tool_choice,
        conversation_hint,
        accumulated_text: String::new(),
        original_chunks: Vec::new(),
        done: false,
        replay_index: 0,
        emit_queue: Vec::new(),
        emit_index: 0,
    };

    Box::pin(futures::stream::unfold(
        (
            Box::pin(inner)
                as std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
            state,
        ),
        |(mut stream, mut st)| async move {
            use futures::StreamExt;

            // Phase 3: emitting tool_calls chunks (after detection succeeded).
            if !st.emit_queue.is_empty() {
                if st.emit_index < st.emit_queue.len() {
                    let chunk = st.emit_queue[st.emit_index].clone();
                    st.emit_index += 1;
                    return Some((Ok(Bytes::from(chunk)), (stream, st)));
                }
                return None; // All emitted.
            }

            // Phase 2: replaying original chunks (after detection found no tool calls).
            if st.done && st.replay_index > 0 {
                if st.replay_index <= st.original_chunks.len() {
                    let idx = st.replay_index - 1;
                    st.replay_index += 1;
                    if idx < st.original_chunks.len() {
                        let chunk = st.original_chunks[idx].clone();
                        return Some((Ok(chunk), (stream, st)));
                    }
                }
                return None;
            }

            // Phase 1: accumulate all chunks from the inner stream.
            if !st.done {
                loop {
                    match stream.next().await {
                        Some(Ok(chunk)) => {
                            // Extract text content from SSE lines in this chunk.
                            if let Ok(text) = std::str::from_utf8(&chunk) {
                                for line in text.lines() {
                                    let line = line.trim();
                                    if line.is_empty() {
                                        continue;
                                    }
                                    if let Some(content) = extract_sse_content(line) {
                                        st.accumulated_text.push_str(&content);
                                    }
                                }
                            }
                            st.original_chunks.push(chunk);
                        }
                        Some(Err(e)) => {
                            // Error — forward immediately.
                            st.done = true;
                            return Some((Err(e), (stream, st)));
                        }
                        None => {
                            // Inner stream ended — decide what to do.
                            st.done = true;
                            break;
                        }
                    }
                }

                // Stream finished. Check if accumulated text has tool calls.
                if has_tool_call_opening(&st.accumulated_text) {
                    let parse_result = parse_tool_calls_from_text_with_context(
                        &st.accumulated_text,
                        &st.tools,
                        st.tool_choice.as_ref(),
                        st.conversation_hint.as_deref(),
                    );
                    if parse_result.had_tool_calls {
                        // Build tool_calls SSE chunks.
                        let tc_chunk = build_tool_calls_sse_chunk(
                            &parse_result.tool_calls,
                            &st.model,
                            &st.response_id,
                        );
                        st.emit_queue.push(tc_chunk);
                        st.emit_queue.push(b"data: [DONE]\n\n".to_vec());
                        st.emit_index = 0;

                        // Emit the first chunk.
                        let first = st.emit_queue[st.emit_index].clone();
                        st.emit_index += 1;
                        return Some((Ok(Bytes::from(first)), (stream, st)));
                    }
                }

                // No tool calls — replay original chunks.
                if st.original_chunks.is_empty() {
                    return None;
                }
                let first = st.original_chunks[0].clone();
                st.replay_index = 2; // Next call will read index 1.
                return Some((Ok(first), (stream, st)));
            }

            None
        },
    ))
}

/// Internal state for the streaming tool call detector.
struct ToolDetectorState {
    model: String,
    response_id: String,
    tools: Vec<CanonicalTool>,
    tool_choice: Option<Value>,
    conversation_hint: Option<String>,
    /// All text content extracted from SSE `content` fields.
    accumulated_text: String,
    /// Original byte chunks saved for replay if no tool calls are detected.
    original_chunks: Vec<Bytes>,
    /// Whether the inner stream has ended.
    done: bool,
    /// Index for replaying original_chunks (1-based, 0 means not replaying).
    replay_index: usize,
    /// Pre-built tool_calls SSE chunks to emit (when tool calls detected).
    emit_queue: Vec<Vec<u8>>,
    /// Index into emit_queue for emission.
    emit_index: usize,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily};
    use serde_json::json;

    // ── Helpers ──────────────────────────────────────────────────────────

    fn make_tool(name: &str, desc: &str, schema: Value) -> CanonicalTool {
        CanonicalTool {
            tool_type: "function".to_string(),
            name: Some(name.to_string()),
            description: Some(desc.to_string()),
            input_schema: Some(schema),
            raw: HashMap::new(),
        }
    }

    fn make_request_with_tools(tools: Vec<CanonicalTool>) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("deepseek-chat".to_string()),
            stream: false,
            messages: vec![
                CanonicalMessage {
                    role: MessageRole::System,
                    content: vec![ContentPart::Text {
                        text: "You are a helpful assistant.".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
                CanonicalMessage {
                    role: MessageRole::User,
                    content: vec![ContentPart::Text {
                        text: "Read the file /tmp/test.txt".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
            ],
            tools,
            tool_choice: Some(json!("auto")),
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    // ── build_tool_injection_prompt ──────────────────────────────────────

    #[test]
    fn build_prompt_generates_correct_xml() {
        let tools = vec![make_tool(
            "read_file",
            "Read the contents of a file",
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file path to read"
                    },
                    "encoding": {
                        "type": "string",
                        "description": "File encoding, default utf-8"
                    }
                },
                "required": ["path"]
            }),
        )];

        let prompt = build_tool_injection_prompt(&tools, None);

        assert!(prompt.contains("<tools>"));
        assert!(prompt.contains("</tools>"));
        assert!(prompt.contains("<tool name=\"read_file\">"));
        assert!(prompt.contains("Read the contents of a file"));
        assert!(prompt.contains("- path (string, required): The file path to read"));
        assert!(prompt.contains("- encoding (string, optional): File encoding, default utf-8"));
        assert!(prompt.contains("TOOL CALL FORMAT"));
        assert!(prompt.contains("<tool_calls>"));
        assert!(prompt.contains("<tool_name>"));
        assert!(prompt.contains("<parameters>"));
    }

    #[test]
    fn build_prompt_handles_no_schema() {
        let tools = vec![CanonicalTool {
            tool_type: "function".to_string(),
            name: Some("simple_tool".to_string()),
            description: Some("A simple tool".to_string()),
            input_schema: None,
            raw: HashMap::new(),
        }];

        let prompt = build_tool_injection_prompt(&tools, None);
        assert!(prompt.contains("<tool name=\"simple_tool\">"));
        assert!(prompt.contains("A simple tool"));
        // Should not contain "Parameters:" when no schema
        assert!(!prompt.contains("Parameters:"));
    }

    #[test]
    fn build_prompt_includes_required_tool_choice_guidance() {
        let tools = vec![make_tool(
            "weather",
            "Read weather",
            json!({
                "type": "object",
                "properties": {
                    "city": {"type": "string"}
                },
                "required": ["city"]
            }),
        )];

        let prompt = build_tool_injection_prompt(&tools, Some(&json!("required")));
        assert!(prompt.contains("TOOL CHOICE REQUIREMENT"));
        assert!(prompt.contains("MUST call at least one tool"));
    }

    #[test]
    fn build_prompt_includes_specific_tool_choice_guidance() {
        let tools = vec![make_tool(
            "weather",
            "Read weather",
            json!({
                "type": "object",
                "properties": {
                    "city": {"type": "string"}
                },
                "required": ["city"]
            }),
        )];

        let prompt = build_tool_injection_prompt(
            &tools,
            Some(&json!({"type": "function", "function": {"name": "weather"}})),
        );
        assert!(prompt.contains("TOOL CHOICE REQUIREMENT"));
        assert!(prompt.contains("MUST call only the tool `weather`"));
    }

    // ── parse_tool_calls_from_text — <tool_calls> format ────────────────

    #[test]
    fn parse_tool_calls_format() {
        let text = r#"<tool_calls>
<tool_call>
<tool_name>read_file</tool_name>
<parameters>{"path": "/tmp/test.txt"}</parameters>
</tool_call>
</tool_calls>"#;

        let result = parse_tool_calls_from_text(text);
        assert!(result.had_tool_calls);
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name.as_deref(), Some("read_file"));
        assert_eq!(
            result.tool_calls[0].arguments.as_deref(),
            Some("{\"path\": \"/tmp/test.txt\"}")
        );
        assert!(result.clean_text.is_empty());
    }

    #[test]
    fn parse_tool_calls_format_accepts_tool_tag_alias() {
        let text = r#"<tool_calls>
<tool_call>
<tool>weather</tool>
<parameters>{"city": "Hangzhou"}</parameters>
</tool_call>
</tool_calls>"#;

        let result = parse_tool_calls_from_text(text);
        assert!(result.had_tool_calls);
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name.as_deref(), Some("weather"));
        assert_eq!(
            result.tool_calls[0].arguments.as_deref(),
            Some("{\"city\": \"Hangzhou\"}")
        );
    }

    #[test]
    fn parse_tool_calls_recovers_name_from_specific_tool_choice() {
        let tools = vec![
            make_tool(
                "weather",
                "Read weather",
                json!({
                    "type": "object",
                    "properties": {
                        "location": {"type": "string"}
                    },
                    "required": ["location"]
                }),
            ),
            make_tool(
                "calendar",
                "Read calendar",
                json!({
                    "type": "object",
                    "properties": {
                        "date": {"type": "string"}
                    },
                    "required": ["date"]
                }),
            ),
        ];
        let text = r#"<tool_calls>
<tool_call>
<unnamed></unnamed>
<parameters>{"location": "Hangzhou"}</parameters>
</tool_call>
</tool_calls>"#;

        let result = parse_tool_calls_from_text_with_context(
            text,
            &tools,
            Some(&json!({"type": "function", "function": {"name": "weather"}})),
            None,
        );
        assert!(result.had_tool_calls);
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name.as_deref(), Some("weather"));
    }

    #[test]
    fn parse_tool_calls_recovers_name_from_single_tool_context() {
        let tools = vec![make_tool(
            "weather",
            "Read weather",
            json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                },
                "required": ["location"]
            }),
        )];
        let text = r#"<tool_calls>
<tool_call>
<unnamed></unnamed>
<parameters>{"location": "Hangzhou"}</parameters>
</tool_call>
</tool_calls>"#;

        let result = parse_tool_calls_from_text_with_context(text, &tools, None, None);
        assert!(result.had_tool_calls);
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name.as_deref(), Some("weather"));
    }

    #[test]
    fn parse_tool_calls_recovers_name_from_conversation_hint() {
        let tools = vec![
            make_tool(
                "weather",
                "Read weather",
                json!({
                    "type": "object",
                    "properties": {
                        "city": {"type": "string"}
                    },
                    "required": ["city"]
                }),
            ),
            make_tool(
                "calendar",
                "Read calendar",
                json!({
                    "type": "object",
                    "properties": {
                        "city": {"type": "string"}
                    },
                    "required": ["city"]
                }),
            ),
        ];
        let text = r#"<tool_calls>
<tool_call>
<tool_name>unnamed</tool_name>
<parameters>{"city": "Hangzhou"}</parameters>
</tool_call>
</tool_calls>"#;

        let result = parse_tool_calls_from_text_with_context(
            text,
            &tools,
            Some(&json!("required")),
            Some("Use only the weather tool for Hangzhou."),
        );
        assert!(result.had_tool_calls);
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name.as_deref(), Some("weather"));
    }

    #[test]
    fn parse_tool_calls_with_preceding_text() {
        let text = r#"I'll read that file for you.

<tool_calls>
<tool_call>
<tool_name>read_file</tool_name>
<parameters>{"path": "/tmp/test.txt"}</parameters>
</tool_call>
</tool_calls>"#;

        let result = parse_tool_calls_from_text(text);
        assert!(result.had_tool_calls);
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.clean_text, "I'll read that file for you.");
    }

    #[test]
    fn parse_tool_calls_recovers_from_malformed_closing_tag() {
        let text = r#"<tool_calls>
<tool_call>
<tool_name>weather</tool_name>
<parameters>{"city":"Hangzhou"}</parameters>
</arg_value>
</tool_calls>"#;

        let result = parse_tool_calls_from_text(text);
        assert!(result.had_tool_calls);
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name.as_deref(), Some("weather"));
        assert_eq!(
            result.tool_calls[0].arguments.as_deref(),
            Some("{\"city\":\"Hangzhou\"}")
        );
    }

    #[test]
    fn parse_tool_calls_recovers_when_outer_block_is_not_closed() {
        let text = r#"<tool_calls>
<tool_call>
<tool_name>weather</tool_name>
<parameters>{"city":"Hangzhou"}</parameters>
</arg_value>"#;

        let result = parse_tool_calls_from_text(text);
        assert!(result.had_tool_calls);
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name.as_deref(), Some("weather"));
        assert_eq!(
            result.tool_calls[0].arguments.as_deref(),
            Some("{\"city\":\"Hangzhou\"}")
        );
        assert_eq!(result.clean_text, "");
    }

    #[test]
    fn parse_empty_tool_block_infers_specific_tool_name_from_context() {
        let tools = vec![
            make_tool(
                "weather",
                "Read weather",
                json!({
                    "type": "object",
                    "properties": {
                        "city": {"type": "string"}
                    },
                    "required": ["city"]
                }),
            ),
            make_tool(
                "calendar",
                "Read calendar",
                json!({
                    "type": "object",
                    "properties": {
                        "city": {"type": "string"}
                    },
                    "required": ["city"]
                }),
            ),
        ];
        let text = "<tool_calls>\n\n</tool_calls>";

        let result = parse_tool_calls_from_text_with_context(
            text,
            &tools,
            Some(&json!("required")),
            Some("Use only the weather tool for Hangzhou."),
        );

        assert!(result.had_tool_calls);
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name.as_deref(), Some("weather"));
        assert_eq!(result.tool_calls[0].arguments.as_deref(), Some("{}"));
    }

    #[test]
    fn parse_tool_calls_recovers_inline_json_without_parameters_tag() {
        let text = r#"<tool_calls>
<tool_call>
<tool_name>weather</tool_name>{"city":"San Francisco"}
</tool_call>
</tool_calls>"#;

        let result = parse_tool_calls_from_text(text);
        assert!(result.had_tool_calls);
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name.as_deref(), Some("weather"));
        assert_eq!(
            result.tool_calls[0].arguments.as_deref(),
            Some("{\"city\":\"San Francisco\"}")
        );
    }

    #[test]
    fn parse_opening_only_tool_block_infers_name_from_context() {
        let tools = vec![make_tool(
            "weather",
            "Read weather",
            json!({
                "type": "object",
                "properties": {
                    "city": {"type": "string"}
                },
                "required": ["city"]
            }),
        )];
        let result = parse_tool_calls_from_text_with_context(
            "<tool_calls>\n",
            &tools,
            Some(&json!("required")),
            Some("Use only the weather tool for Hangzhou."),
        );

        assert!(result.had_tool_calls);
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name.as_deref(), Some("weather"));
        assert_eq!(result.tool_calls[0].arguments.as_deref(), Some("{}"));
    }

    #[test]
    fn parse_garbled_tool_block_closer_still_infers_name_from_context() {
        let tools = vec![make_tool(
            "weather",
            "Read weather",
            json!({
                "type": "object",
                "properties": {
                    "city": {"type": "string"}
                },
                "required": ["city"]
            }),
        )];
        let result = parse_tool_calls_from_text_with_context(
            "<tool_calls>\n</</tool_calls>",
            &tools,
            Some(&json!("required")),
            Some("Use only the weather tool for Hangzhou."),
        );

        assert!(result.had_tool_calls);
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name.as_deref(), Some("weather"));
        assert_eq!(result.tool_calls[0].arguments.as_deref(), Some("{}"));
    }

    // ── parse_tool_calls_from_text — <function_calls> format ────────────

    #[test]
    fn parse_function_calls_format() {
        let text = r#"<function_calls>
<function_call>
<tool>get_weather</tool>
<args_json>{"location": "NYC", "units": "metric"}</args_json>
</function_call>
</function_calls>"#;

        let result = parse_tool_calls_from_text(text);
        assert!(result.had_tool_calls);
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name.as_deref(), Some("get_weather"));
        assert_eq!(
            result.tool_calls[0].arguments.as_deref(),
            Some("{\"location\": \"NYC\", \"units\": \"metric\"}")
        );
    }

    #[test]
    fn parse_tool_calls_unescapes_xml_encoded_arguments() {
        let text = r#"<tool_calls>
<tool_call>
<tool_name>weather</tool_name>
<parameters>{&quot;city&quot;:&quot;Hangzhou&quot;}</parameters>
</tool_call>
</tool_calls>"#;

        let result = parse_tool_calls_from_text(text);
        assert!(result.had_tool_calls);
        assert_eq!(
            result.tool_calls[0].arguments.as_deref(),
            Some("{\"city\":\"Hangzhou\"}")
        );
    }

    // ── parse_tool_calls_from_text — <invoke> format ────────────────────

    #[test]
    fn parse_invoke_format() {
        let text = r#"<invoke name="search_web">
<parameter name="query">rust programming</parameter>
<parameter name="limit">10</parameter>
</invoke>"#;

        let result = parse_tool_calls_from_text(text);
        assert!(result.had_tool_calls);
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name.as_deref(), Some("search_web"));

        let args: Value =
            serde_json::from_str(result.tool_calls[0].arguments.as_deref().unwrap()).unwrap();
        assert_eq!(args["query"], "rust programming");
        assert_eq!(args["limit"], "10");
    }

    // ── parse_tool_calls_from_text — no tool calls ──────────────────────

    #[test]
    fn parse_no_tool_calls() {
        let text = "Just a regular response with no tool calls.";
        let result = parse_tool_calls_from_text(text);
        assert!(!result.had_tool_calls);
        assert!(result.tool_calls.is_empty());
        assert_eq!(result.clean_text, text);
    }

    // ── Multiple tool calls in one response ─────────────────────────────

    #[test]
    fn parse_multiple_tool_calls() {
        let text = r#"<tool_calls>
<tool_call>
<tool_name>read_file</tool_name>
<parameters>{"path": "/tmp/a.txt"}</parameters>
</tool_call>
<tool_call>
<tool_name>read_file</tool_name>
<parameters>{"path": "/tmp/b.txt"}</parameters>
</tool_call>
</tool_calls>"#;

        let result = parse_tool_calls_from_text(text);
        assert!(result.had_tool_calls);
        assert_eq!(result.tool_calls.len(), 2);
        assert_eq!(result.tool_calls[0].name.as_deref(), Some("read_file"));
        assert_eq!(result.tool_calls[1].name.as_deref(), Some("read_file"));

        let args0: Value =
            serde_json::from_str(result.tool_calls[0].arguments.as_deref().unwrap()).unwrap();
        let args1: Value =
            serde_json::from_str(result.tool_calls[1].arguments.as_deref().unwrap()).unwrap();
        assert_eq!(args0["path"], "/tmp/a.txt");
        assert_eq!(args1["path"], "/tmp/b.txt");
    }

    // ── Complex JSON parameters ─────────────────────────────────────────

    #[test]
    fn parse_complex_json_parameters() {
        let text = r#"<tool_calls>
<tool_call>
<tool_name>execute_query</tool_name>
<parameters>{"query": "SELECT * FROM users", "options": {"limit": 10, "offset": 0}, "tags": ["prod", "readonly"]}</parameters>
</tool_call>
</tool_calls>"#;

        let result = parse_tool_calls_from_text(text);
        assert!(result.had_tool_calls);
        assert_eq!(result.tool_calls.len(), 1);

        let args: Value =
            serde_json::from_str(result.tool_calls[0].arguments.as_deref().unwrap()).unwrap();
        assert_eq!(args["query"], "SELECT * FROM users");
        assert_eq!(args["options"]["limit"], 10);
        assert_eq!(args["tags"][0], "prod");
    }

    // ── strip_think_blocks ──────────────────────────────────────────────

    #[test]
    fn strip_think_simple() {
        let text = "<think>internal reasoning</think>The answer is 42.";
        let result = strip_think_blocks(text);
        assert_eq!(result, "The answer is 42.");
    }

    #[test]
    fn strip_think_nested() {
        let text = "<think>outer <think>inner</think> still thinking</think>Done.";
        let result = strip_think_blocks(text);
        assert_eq!(result, "Done.");
    }

    #[test]
    fn strip_think_no_think_blocks() {
        let text = "No thinking here.";
        let result = strip_think_blocks(text);
        assert_eq!(result, "No thinking here.");
    }

    #[test]
    fn strip_think_preserves_utf8_without_think_blocks() {
        let text = "法国的首都是巴黎（Paris）。";
        let result = strip_think_blocks(text);
        assert_eq!(result, text);
    }

    #[test]
    fn strip_think_preserves_utf8_around_removed_block() {
        let text = "法国的首都<think>先分析一下</think>是巴黎。";
        let result = strip_think_blocks(text);
        assert_eq!(result, "法国的首都是巴黎。");
    }

    #[test]
    fn strip_think_with_tool_calls() {
        let text = r#"<think>Let me think about which tool to call...</think>
<tool_calls>
<tool_call>
<tool_name>read_file</tool_name>
<parameters>{"path": "/tmp/test.txt"}</parameters>
</tool_call>
</tool_calls>"#;

        let cleaned = strip_think_blocks(text);
        assert!(!cleaned.contains("<think>"));
        assert!(cleaned.contains("<tool_calls>"));

        let result = parse_tool_calls_from_text(text);
        assert!(result.had_tool_calls);
        assert_eq!(result.tool_calls.len(), 1);
    }

    // ── needs_tool_injection ────────────────────────────────────────────

    #[test]
    fn native_openai_models_no_injection() {
        assert!(!needs_tool_injection("gpt-4o", "openai_compatible"));
        assert!(!needs_tool_injection("gpt-3.5-turbo", "openai_compatible"));
        assert!(!needs_tool_injection("GPT-4o", "openai_compatible"));
    }

    #[test]
    fn native_reasoning_models_no_injection() {
        assert!(!needs_tool_injection("o1-preview", "openai_compatible"));
        assert!(!needs_tool_injection("o3-mini", "openai_compatible"));
        assert!(!needs_tool_injection("o4-mini", "openai_compatible"));
        assert!(!needs_tool_injection("o1", "openai_compatible"));
        assert!(!needs_tool_injection("o3", "openai_compatible"));
    }

    #[test]
    fn native_claude_models_no_injection() {
        assert!(!needs_tool_injection(
            "claude-3-5-sonnet",
            "openai_compatible"
        ));
        assert!(!needs_tool_injection(
            "claude-sonnet-4-6",
            "openai_compatible"
        ));
    }

    #[test]
    fn anthropic_adapter_no_injection() {
        assert!(!needs_tool_injection("any-model", "anthropic_compatible"));
        assert!(!needs_tool_injection(
            "deepseek-chat",
            "anthropic_compatible"
        ));
    }

    #[test]
    fn gemini_and_native_protocol_adapters_never_inject() {
        assert!(!needs_tool_injection(
            "google-gemini-api-fixture",
            "gemini_api_compatible"
        ));
        assert!(!needs_tool_injection(
            "google-gemini-api-fixture",
            "gemini_api_modular_compatible"
        ));
        assert!(!needs_tool_injection(
            "aistudio-web-reverse-fixture",
            "aistudio_web_reverse_compatible"
        ));
        assert!(!needs_tool_injection(
            "gemini-2.5-flash",
            "gemini_canvas_compatible"
        ));
        assert!(!needs_tool_injection(
            "gemini-2.5-flash",
            "gemini_canvas_web_reverse_compatible"
        ));
        assert!(!needs_tool_injection("command-r-plus", "cohere_compatible"));
        assert!(!needs_tool_injection(
            "claude-bedrock",
            "bedrock_converse_compatible"
        ));
    }

    #[test]
    fn deepseek_needs_injection() {
        assert!(needs_tool_injection("deepseek-chat", "openai_compatible"));
        assert!(needs_tool_injection("deepseek-coder", "openai_compatible"));
        assert!(needs_tool_injection("DeepSeek-V2", "openai_compatible"));
    }

    #[test]
    fn unknown_model_needs_injection() {
        assert!(needs_tool_injection("my-custom-model", "openai_compatible"));
        assert!(needs_tool_injection("llama-3-70b", "openai_compatible"));
        assert!(needs_tool_injection("qwen-72b", "openai_compatible"));
    }

    // ── inject_tools ────────────────────────────────────────────────────

    #[test]
    fn inject_tools_modifies_request() {
        let tools = vec![make_tool(
            "read_file",
            "Read a file",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path"}
                },
                "required": ["path"]
            }),
        )];

        let mut req = make_request_with_tools(tools);
        assert!(!req.tools.is_empty());
        assert!(req.tool_choice.is_some());

        inject_tools(&mut req);

        // Tools and tool_choice should be cleared.
        assert!(req.tools.is_empty());
        assert!(req.tool_choice.is_none());

        // System message should contain the tool prompt.
        let system_text = req
            .messages
            .iter()
            .find(|m| m.role == MessageRole::System)
            .unwrap()
            .text_content();
        assert!(system_text.contains("<tools>"));
        assert!(system_text.contains("read_file"));
        assert!(system_text.contains("TOOL CALL FORMAT"));
        // Original system message should still be present.
        assert!(system_text.contains("You are a helpful assistant."));
    }

    #[test]
    fn inject_tools_creates_system_when_absent() {
        let tools = vec![make_tool(
            "test_tool",
            "A test tool",
            json!({"type": "object", "properties": {}}),
        )];

        let mut req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("deepseek-chat".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Hello".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools,
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        inject_tools(&mut req);

        // A system message should be inserted at position 0.
        assert_eq!(req.messages[0].role, MessageRole::System);
        assert!(req.messages[0].text_content().contains("<tools>"));
        // User message should still be at position 1.
        assert_eq!(req.messages[1].role, MessageRole::User);
    }

    #[test]
    fn inject_tools_noop_when_no_tools() {
        let mut req = make_request_with_tools(vec![]);
        let original_msg_count = req.messages.len();
        inject_tools(&mut req);
        assert_eq!(req.messages.len(), original_msg_count);
    }

    #[test]
    fn inject_tools_converts_tool_history() {
        let tools = vec![make_tool(
            "read_file",
            "Read a file",
            json!({"type": "object", "properties": {}}),
        )];

        let mut req = make_request_with_tools(tools);

        // Add assistant message with tool_calls.
        req.messages.push(CanonicalMessage {
            role: MessageRole::Assistant,
            content: vec![],
            name: None,
            tool_call_id: None,
            tool_calls: vec![CanonicalToolCall {
                id: Some("call_123".to_string()),
                call_type: "function".to_string(),
                name: Some("read_file".to_string()),
                arguments: Some("{\"path\":\"/tmp/test.txt\"}".to_string()),
                raw: HashMap::new(),
            }],
        });

        // Add tool result message.
        req.messages.push(CanonicalMessage {
            role: MessageRole::Tool,
            content: vec![ContentPart::Text {
                text: "file contents here".to_string(),
            }],
            name: None,
            tool_call_id: Some("call_123".to_string()),
            tool_calls: vec![],
        });

        inject_tools(&mut req);

        // Assistant message should now have XML tool calls in text.
        let assistant_msg = &req.messages[2];
        assert_eq!(assistant_msg.role, MessageRole::Assistant);
        let assistant_text = assistant_msg.text_content();
        assert!(assistant_text.contains("<tool_calls>"));
        assert!(assistant_text.contains("read_file"));
        assert!(assistant_msg.tool_calls.is_empty());

        // Tool result should be converted to user role with XML formatting.
        let tool_msg = &req.messages[3];
        assert_eq!(tool_msg.role, MessageRole::User);
        let tool_text = tool_msg.text_content();
        assert!(tool_text.contains("<tool_result>"));
        assert!(tool_text.contains("file contents here"));
        assert!(tool_text.contains("call_123"));
        assert!(tool_msg.tool_call_id.is_none());
    }

    #[test]
    fn serialize_tool_history_preserves_json_tool_result_content_without_tools() {
        let mut req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("qwen".to_string()),
            stream: false,
            messages: vec![
                CanonicalMessage {
                    role: MessageRole::Assistant,
                    content: vec![],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![CanonicalToolCall {
                        id: Some("call_weather".to_string()),
                        call_type: "function".to_string(),
                        name: Some("weather".to_string()),
                        arguments: Some("{\"city\":\"Hangzhou\"}".to_string()),
                        raw: HashMap::new(),
                    }],
                },
                CanonicalMessage {
                    role: MessageRole::Tool,
                    content: vec![ContentPart::Json {
                        value: json!({"city":"Hangzhou","condition":"sunny"}),
                    }],
                    name: Some("weather".to_string()),
                    tool_call_id: Some("call_weather".to_string()),
                    tool_calls: vec![],
                },
            ],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        serialize_tool_history(&mut req);
        assert_eq!(req.messages[0].role, MessageRole::Assistant);
        assert!(req.messages[0].text_content().contains("<tool_calls>"));
        assert_eq!(req.messages[1].role, MessageRole::User);
        let rendered = req.messages[1].text_content();
        assert!(rendered.contains("<tool_result>"));
        assert!(rendered.contains("\"condition\":\"sunny\""));
    }

    // ── format_tool_call_as_xml ─────────────────────────────────────────

    #[test]
    fn format_tool_call_xml_valid() {
        let calls = vec![CanonicalToolCall {
            id: Some("call_1".to_string()),
            call_type: "function".to_string(),
            name: Some("test_tool".to_string()),
            arguments: Some("{\"key\":\"value\"}".to_string()),
            raw: HashMap::new(),
        }];

        let xml = format_tool_call_as_xml(&calls);
        assert!(xml.contains("<tool_calls>"));
        assert!(xml.contains("</tool_calls>"));
        assert!(xml.contains("<tool_name>test_tool</tool_name>"));
        assert!(xml.contains("<parameters>"));
    }

    // ── xml_escape ──────────────────────────────────────────────────────

    #[test]
    fn xml_escape_special_chars() {
        assert_eq!(xml_escape("a & b"), "a &amp; b");
        assert_eq!(xml_escape("<tag>"), "&lt;tag&gt;");
        assert_eq!(xml_escape("\"quoted\""), "&quot;quoted&quot;");
    }

    // ── Tool call IDs ───────────────────────────────────────────────────

    #[test]
    fn parsed_tool_calls_have_unique_ids() {
        let text = r#"<tool_calls>
<tool_call>
<tool_name>tool_a</tool_name>
<parameters>{}</parameters>
</tool_call>
<tool_call>
<tool_name>tool_b</tool_name>
<parameters>{}</parameters>
</tool_call>
</tool_calls>"#;

        let result = parse_tool_calls_from_text(text);
        assert_eq!(result.tool_calls.len(), 2);

        let id_a = result.tool_calls[0].id.as_ref().unwrap();
        let id_b = result.tool_calls[1].id.as_ref().unwrap();
        assert_ne!(id_a, id_b);
        assert!(id_a.starts_with("call_"));
        assert!(id_b.starts_with("call_"));
    }

    // ── call_type is always "function" ──────────────────────────────────

    #[test]
    fn parsed_tool_calls_have_function_type() {
        let text = r#"<tool_calls>
<tool_call>
<tool_name>my_tool</tool_name>
<parameters>{"x": 1}</parameters>
</tool_call>
</tool_calls>"#;

        let result = parse_tool_calls_from_text(text);
        assert_eq!(result.tool_calls[0].call_type, "function");
    }

    // ── Streaming tool call detection ──────────────────────────────────

    #[test]
    fn has_tool_call_opening_detects_tags() {
        assert!(has_tool_call_opening("text before <tool_calls> text after"));
        assert!(has_tool_call_opening("<function_calls>"));
        assert!(has_tool_call_opening("<invoke name=\"test\">"));
        assert!(!has_tool_call_opening("no tool calls here"));
        assert!(!has_tool_call_opening(""));
    }

    #[test]
    fn extract_sse_content_basic() {
        let line = r#"data: {"choices":[{"delta":{"content":"hello"}}]}"#;
        assert_eq!(extract_sse_content(line), Some("hello".to_string()));
    }

    #[test]
    fn extract_sse_content_done() {
        assert_eq!(extract_sse_content("data: [DONE]"), None);
    }

    #[test]
    fn extract_sse_content_no_content_field() {
        let line = r#"data: {"choices":[{"delta":{"role":"assistant"}}]}"#;
        assert_eq!(extract_sse_content(line), None);
    }

    #[test]
    fn extract_sse_content_non_data_line() {
        assert_eq!(extract_sse_content(": ping"), None);
        assert_eq!(extract_sse_content(""), None);
    }

    #[test]
    fn build_tool_calls_chunk_format() {
        let calls = vec![CanonicalToolCall {
            id: Some("call_abc".to_string()),
            call_type: "function".to_string(),
            name: Some("read_file".to_string()),
            arguments: Some("{\"path\":\"/tmp/x\"}".to_string()),
            raw: HashMap::new(),
        }];

        let chunk_bytes = build_tool_calls_sse_chunk(&calls, "test-model", "resp-1");
        let chunk_str = String::from_utf8(chunk_bytes).unwrap();
        assert!(chunk_str.starts_with("data: "));
        assert!(chunk_str.ends_with("\n\n"));

        let json_str = chunk_str.strip_prefix("data: ").unwrap().trim();
        let parsed: Value = serde_json::from_str(json_str).unwrap();
        assert_eq!(parsed["id"], "resp-1");
        assert_eq!(parsed["model"], "test-model");
        assert_eq!(parsed["choices"][0]["finish_reason"], "tool_calls");
        let tc = &parsed["choices"][0]["delta"]["tool_calls"][0];
        assert_eq!(tc["id"], "call_abc");
        assert_eq!(tc["function"]["name"], "read_file");
        assert_eq!(tc["function"]["arguments"], "{\"path\":\"/tmp/x\"}");
    }

    #[tokio::test]
    async fn streaming_detection_with_tool_calls() {
        use futures::StreamExt;

        // Simulate SSE chunks containing XML tool calls in content.
        let sse_events = vec![
            format!(
                "data: {}\n\n",
                json!({
                    "choices": [{"delta": {"content": "<tool_calls>\n<tool_call>\n"}}]
                })
            ),
            format!(
                "data: {}\n\n",
                json!({
                    "choices": [{"delta": {"content": "<tool_name>read_file</tool_name>\n"}}]
                })
            ),
            format!(
                "data: {}\n\n",
                json!({
                    "choices": [{"delta": {"content": "<parameters>{\"path\":\"/tmp/test.txt\"}</parameters>\n"}}]
                })
            ),
            format!(
                "data: {}\n\n",
                json!({
                    "choices": [{"delta": {"content": "</tool_call>\n</tool_calls>"}}]
                })
            ),
            "data: [DONE]\n\n".to_string(),
        ];

        let chunks: Vec<Result<Bytes, rquest::Error>> =
            sse_events.into_iter().map(|s| Ok(Bytes::from(s))).collect();

        let inner = futures::stream::iter(chunks);
        let mut wrapped = wrap_streaming_tool_detection(
            inner,
            "deepseek-chat".to_string(),
            "resp-test".to_string(),
            vec![],
            None,
            None,
        );

        let mut collected: Vec<String> = Vec::new();
        while let Some(Ok(bytes)) = wrapped.next().await {
            collected.push(String::from_utf8(bytes.to_vec()).unwrap());
        }

        // Should emit tool_calls chunk + [DONE], NOT the original text chunks.
        assert_eq!(
            collected.len(),
            2,
            "expected 2 chunks: tool_calls + DONE, got: {collected:?}"
        );

        // First chunk should be tool_calls.
        let json_str = collected[0].strip_prefix("data: ").unwrap().trim();
        let parsed: Value = serde_json::from_str(json_str).unwrap();
        assert_eq!(parsed["choices"][0]["finish_reason"], "tool_calls");
        let tc = &parsed["choices"][0]["delta"]["tool_calls"][0];
        assert_eq!(tc["function"]["name"], "read_file");

        // Second chunk should be [DONE].
        assert_eq!(collected[1], "data: [DONE]\n\n");
    }

    #[tokio::test]
    async fn streaming_detection_without_tool_calls() {
        use futures::StreamExt;

        // Regular text response — no tool calls.
        let sse_events = vec![
            format!(
                "data: {}\n\n",
                json!({"choices": [{"delta": {"content": "Hello, "}}]})
            ),
            format!(
                "data: {}\n\n",
                json!({"choices": [{"delta": {"content": "world!"}}]})
            ),
            "data: [DONE]\n\n".to_string(),
        ];

        let original_count = sse_events.len();
        let chunks: Vec<Result<Bytes, rquest::Error>> =
            sse_events.into_iter().map(|s| Ok(Bytes::from(s))).collect();

        let inner = futures::stream::iter(chunks);
        let mut wrapped = wrap_streaming_tool_detection(
            inner,
            "deepseek-chat".to_string(),
            "resp-test".to_string(),
            vec![],
            None,
            None,
        );

        let mut collected: Vec<Bytes> = Vec::new();
        while let Some(Ok(bytes)) = wrapped.next().await {
            collected.push(bytes);
        }

        // Should replay original chunks unchanged.
        assert_eq!(collected.len(), original_count);
    }

    #[tokio::test]
    async fn streaming_detection_empty_stream() {
        use futures::StreamExt;

        let chunks: Vec<Result<Bytes, rquest::Error>> = vec![];
        let inner = futures::stream::iter(chunks);
        let mut wrapped = wrap_streaming_tool_detection(
            inner,
            "deepseek-chat".to_string(),
            "resp-test".to_string(),
            vec![],
            None,
            None,
        );

        let mut count = 0;
        while let Some(Ok(_)) = wrapped.next().await {
            count += 1;
        }
        assert_eq!(count, 0);
    }
}
