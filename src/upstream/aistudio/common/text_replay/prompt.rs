use serde_json::Value;

use super::normalize_string;
use super::tool_bridge::{build_tool_definition_prompt, extract_gemini_tool_metadata};

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

pub(super) fn collect_gemini_text_strings(node: &Value, acc: &mut Vec<String>) {
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

pub(super) fn extract_model_message_blocks(node: &Value, blocks: &mut Vec<Vec<String>>) {
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
