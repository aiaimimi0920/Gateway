use std::collections::HashMap;

use regex::Regex;
use serde_json::Value;

use crate::protocol::canonical::{CanonicalTool, CanonicalToolCall};
use crate::protocol::tool_choice::{self, CanonicalToolChoice};

use super::re_generic_xml_tag;

pub(super) fn resolve_tool_name(
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

pub(super) fn parse_malformed_tool_call_block(
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

pub(super) fn extract_inline_json_object(text: &str) -> Option<String> {
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

pub(super) fn extract_incomplete_outer_block<'a>(
    text: &'a str,
    tag_name: &str,
) -> Option<(usize, &'a str)> {
    let needle = format!("<{tag_name}");
    let start = text.find(&needle)?;
    let after_start = &text[start..];
    let close = after_start.find('>')?;
    let inner_start = start + close + 1;
    Some((start, &text[inner_start..]))
}

pub(super) fn xml_unescape(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Create a [`CanonicalToolCall`] from parsed name and arguments.
pub(super) fn make_tool_call(name: Option<String>, arguments: Option<String>) -> CanonicalToolCall {
    CanonicalToolCall {
        id: Some(format!("call_{}", uuid::Uuid::new_v4().simple())),
        call_type: "function".to_string(),
        name,
        arguments,
        raw: HashMap::new(),
    }
}
