use crate::protocol::canonical::TokenUsage;
use serde_json::Value;

pub(super) fn apply_qwen_web_response_value(
    data: Option<&Value>,
    reported_model: &mut String,
    text: &mut String,
    usage: &mut Option<TokenUsage>,
    finish_reason: &mut Option<String>,
) -> bool {
    let Some(data) = data else {
        return false;
    };
    if let Some(candidate_model) = data.get("model").and_then(Value::as_str) {
        *reported_model = candidate_model.to_string();
    }
    if let Some(token_usage) = parse_token_usage(data.get("usage")) {
        *usage = Some(token_usage);
    }
    let Some(choice) = data
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
    else {
        return false;
    };
    if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
        *finish_reason = Some(reason.to_string());
    }
    if let Some(content) = extract_choice_text(choice) {
        text.push_str(&content);
    }
    true
}

fn parse_token_usage(value: Option<&Value>) -> Option<TokenUsage> {
    let usage = value?;
    let prompt_tokens = usage.get("prompt_tokens").and_then(|v| v.as_u64())?;
    let completion_tokens = usage
        .get("completion_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or_default();
    let total_tokens = usage
        .get("total_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| prompt_tokens.saturating_add(completion_tokens));
    Some(TokenUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
    })
}

pub(super) fn extract_choice_text(choice: &Value) -> Option<String> {
    if let Some(delta) = choice.get("delta") {
        let phase = delta.get("phase").and_then(Value::as_str);
        if matches!(phase, Some("think") | Some("thinking_summary")) {
            return None;
        }
        if let Some(content) = extract_content_text(delta.get("content")) {
            return Some(content);
        }
    }
    extract_content_text(
        choice
            .get("message")
            .and_then(|message| message.get("content")),
    )
}

fn extract_content_text(content: Option<&Value>) -> Option<String> {
    match content? {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Array(parts) => {
            let text = parts
                .iter()
                .filter_map(|part| {
                    part.get("text")
                        .or_else(|| part.get("content"))
                        .and_then(Value::as_str)
                })
                .collect::<String>();
            (!text.is_empty()).then_some(text)
        }
        _ => None,
    }
}
