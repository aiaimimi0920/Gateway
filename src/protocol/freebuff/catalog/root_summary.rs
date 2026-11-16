//! Foreground structured summaries adapted from the fixed runtime, not background/idle persistence.
//! Apache-2.0 source and native/ownership modifications: root_builder/README.md and REFERENCE.md.
mod prompt;
use super::{
    root_context::{estimate, RootPreview},
    BusinessError, RootHistory,
};
use serde_json::{json, Value};

pub(super) fn due(preview: RootPreview, history: &RootHistory) -> Result<bool, BusinessError> {
    let Some(reported) = preview.reported_tokens else {
        return Ok(false);
    };
    let limit = preview.forecast.limit;
    let threshold = (limit as u128 * 4 / 5) as u64;
    if reported <= threshold || history.messages().len() <= history.fresh_users().count() + 1 {
        return Ok(false);
    }
    if reported > limit {
        return Ok(true);
    }
    let fresh = history.fresh_users().try_fold(0u64, |n, value| {
        n.checked_add(estimate::message(value)?)
            .ok_or(BusinessError::RequestTooLarge)
    })?;
    let fixed = preview.fixed_tokens + fresh;
    let summary_budget = 6000.min(limit.saturating_sub(fixed) / 3).min(16384 / 2);
    Ok(fixed + summary_budget <= (threshold as u128 * 85 / 100) as u64)
}
pub(super) fn request(mut body: Value, limit: u64) -> Value {
    body["messages"].as_array_mut().expect("root builder messages").push(json!({
        "role":"user","content":format!("<system_instructions>{}</system_instructions>", prompt::instruction(2000.min(limit / 10)))
    }));
    body["stream"] = json!(true);
    body["max_tokens"] = json!(16384);
    let id = uuid::Uuid::new_v4().to_string();
    body["codebuff_metadata"] = json!({"compaction_summary":"1", "compaction_summary_id":id,
        "compaction_request_id":id,"compaction_request_group":"summary","compaction_trigger":"context_limit"});
    body
}
pub(super) fn from_completion(body: &Value) -> Result<String, BusinessError> {
    let choices = body["choices"]
        .as_array()
        .filter(|values| values.len() == 1)
        .ok_or(BusinessError::SummaryInvalid)?;
    let choice = &choices[0];
    if choice["finish_reason"] == "length" {
        return Err(BusinessError::SummaryOutputLimit);
    }
    // A filtered/tool-producing answer must not replace history even when its prose has headings.
    if choice["finish_reason"] != "stop"
        || choice["message"]
            .get("tool_calls")
            .is_some_and(|v| !v.is_null() && v.as_array().is_none_or(|v| !v.is_empty()))
    {
        return Err(BusinessError::SummaryInvalid);
    }
    let text = choice["message"]["content"]
        .as_str()
        .ok_or(BusinessError::SummaryInvalid)?;
    if text.len() > 1024 * 1024 {
        return Err(BusinessError::ResponseTooLarge);
    }
    let summary = extract(text);
    if summary.encode_utf16().count() < 200 {
        return Err(BusinessError::SummaryInvalid);
    }
    Ok(summary)
}
fn extract(reply: &str) -> String {
    let mut stripped = String::with_capacity(reply.len());
    let mut rest = reply;
    // Literal paired think blocks match the fixed non-greedy global pattern, without backtracking.
    while let Some(start) = rest.find("<think>") {
        let Some(end) = rest[start + 7..].find("</think>") else {
            break;
        };
        stripped.push_str(&rest[..start]);
        rest = &rest[start + 7 + end + 8..];
    }
    stripped.push_str(rest);
    let offset = stripped.find("## Objective").or_else(|| {
        let mut headings = stripped.match_indices("\n## ").map(|(i, _)| i + 1);
        let first = if stripped.starts_with("## ") {
            Some(0)
        } else {
            headings.next()
        };
        first.filter(|_| headings.take(5).count() == 5)
    });
    offset
        .map(|index| stripped[index..].trim().to_owned())
        .unwrap_or_default()
}
pub(super) fn memory(text: &str) -> Value {
    json!({"role":"user", "content":format!("<conversation_summary>\nThis is a summary of the conversation so far. The original messages have been condensed to save context space.\n\n<historical_memory>\n{text}\n</historical_memory>\n</conversation_summary>\n\nHistorical memory only: it replaces the earlier conversation. Messages after it are real messages. The latest user request can appear both in the memory and after it: the memory then records work already done on it, so continue that work, do not start over. If no user message follows, resume the latest request in the memory. Use real tool calls when further action is needed or give the final response when the work is complete.")})
}
#[cfg(test)]
mod tests;
