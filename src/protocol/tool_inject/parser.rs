use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use crate::protocol::canonical::{CanonicalTool, CanonicalToolCall};

mod recovery;
use recovery::{
    extract_incomplete_outer_block, extract_inline_json_object, make_tool_call,
    parse_malformed_tool_call_block, resolve_tool_name, xml_unescape,
};

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

/// Strip `<think>...</think>` blocks from text, handling nested tags.
pub(super) fn strip_think_blocks(text: &str) -> String {
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
