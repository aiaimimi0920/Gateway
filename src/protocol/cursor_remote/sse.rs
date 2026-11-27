//! Buffered Cursor phases expressed as Chat deltas for the existing Responses owner.
//! Keep custom input opaque; the generic canonical SSE helper only emits functions.
use crate::protocol::{canonical::CanonicalRelayResponse, openai, sse_parse::format_sse_event};
use bytes::Bytes;
use serde_json::{json, Value};

pub(crate) fn frames(
    model: &str,
    response: &CanonicalRelayResponse,
    thinking: Option<&super::thinking::PublicThinking>,
) -> Vec<Bytes> {
    let id = format!("chatcmpl_cursor_{}", uuid::Uuid::new_v4().simple());
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let frame = |delta: Value, finish: Value| {
        Bytes::from(format_sse_event(
            None,
            &json!({
                "id":id,"object":"chat.completion.chunk","created":created,"model":model,
                "choices":[{"index":0,"delta":delta,"finish_reason":finish}],
            })
            .to_string(),
        ))
    };
    let mut frames = Vec::new();
    if let Some(thinking) = thinking {
        frames.push(frame(
            json!({"role":"assistant","reasoning_content":thinking.text()}),
            Value::Null,
        ));
    }
    if !response.text.is_empty() {
        frames.push(Bytes::from(format_sse_event(
            None,
            &openai::build_chat_completions_delta(&id, created as i64, model, &response.text)
                .to_string(),
        )));
    }
    for (index, call) in response.tool_calls.iter().enumerate() {
        let custom = call.call_type == "custom";
        let kind = if custom { "custom" } else { "function" };
        let input = if custom { "input" } else { "arguments" };
        let mut item = json!({"index":index,"id":call.id,"type":kind});
        item[kind] = json!({"name":call.name});
        // One complete native handoff; this does not pretend to be token streaming.
        item[kind][input] = json!(call.arguments);
        let mut delta = json!({"tool_calls":[item]});
        if response.text.is_empty() && thinking.is_none() && index == 0 {
            delta["role"] = json!("assistant");
        }
        frames.push(frame(delta, Value::Null));
    }
    frames.push(frame(json!({}), json!(response.finish_reason)));
    frames.push(Bytes::from(format_sse_event(None, "[DONE]")));
    frames
}
