// ---------------------------------------------------------------------------
// Anthropic protocol adapter — pack / unpack / builder helpers
//
// Converts between CanonicalRelayRequest/Response and the Anthropic
// /v1/messages wire format.
//
// Also provides an OpenAI SSE → Anthropic SSE stream translator for cases
// where the client expects Anthropic event format but the upstream returns
// OpenAI-compatible chunks.
// ---------------------------------------------------------------------------

mod normalize;
mod request;
mod response;
mod to_anthropic;
mod upstream_accumulator;

pub use normalize::normalize_messages;
pub use request::{
    inspect_prompt_cache_telemetry, pack_anthropic, pack_anthropic_with_telemetry,
    PackedAnthropicRequest, PromptCacheTelemetry,
};
pub use response::{
    build_messages_delta, build_messages_stop, build_messages_success, unpack_anthropic_response,
};
pub use to_anthropic::translate_openai_sse_to_anthropic;
pub use upstream_accumulator::accumulate_anthropic_stream;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod normalize_tests;

#[cfg(test)]
mod request_tests;

#[cfg(test)]
mod response_tests;

#[cfg(test)]
mod to_anthropic_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{MessageRole, TokenUsage};
    use bytes::Bytes;
    use serde_json::json;

    // ── normalize_messages ────────────────────────────────────────────────

    #[test]
    fn normalize_basic_anthropic_request() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "system": "Be helpful.",
            "messages": [{"role": "user", "content": "Hello!"}],
            "max_tokens": 1024,
        });
        let req = normalize_messages(body).unwrap();
        assert_eq!(
            req.requested_model.as_deref(),
            Some("claude-3-5-sonnet-20241022")
        );
        // First message should be the extracted system prompt.
        assert_eq!(req.messages[0].role, MessageRole::System);
        assert_eq!(req.messages[1].role, MessageRole::User);
    }

    #[test]
    fn normalize_no_system_field() {
        let body = json!({
            "model": "claude-3-opus-20240229",
            "messages": [{"role": "user", "content": "Hi"}],
            "max_tokens": 512,
        });
        let req = normalize_messages(body).unwrap();
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, MessageRole::User);
    }

    #[test]
    fn normalize_captures_max_tokens_in_extra() {
        let body = json!({
            "model": "claude-3-haiku-20240307",
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 2048,
        });
        let req = normalize_messages(body).unwrap();
        assert_eq!(req.extra.get("max_tokens"), Some(&json!(2048)));
    }

    // ── unpack_anthropic_response ─────────────────────────────────────────

    #[test]
    fn unpack_standard_anthropic_response() {
        let body = json!({
            "id": "msg_01XFDUDYJgAACzvnptvVoYEL",
            "type": "message",
            "role": "assistant",
            "model": "claude-3-5-sonnet-20241022",
            "content": [{"type": "text", "text": "Hello! How can I help?"}],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 12,
                "output_tokens": 8,
                "cache_creation_input_tokens": 100,
                "cache_read_input_tokens": 40
            }
        });
        let resp = unpack_anthropic_response(&body).unwrap();
        assert_eq!(resp.text, "Hello! How can I help?");
        assert_eq!(resp.model, "claude-3-5-sonnet-20241022");
        assert_eq!(resp.finish_reason.as_deref(), Some("stop"));
        let usage = resp.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 12);
        assert_eq!(usage.completion_tokens, 8);
        assert_eq!(usage.total_tokens, 20);
        assert_eq!(usage.cache_creation_input_tokens, Some(100));
        assert_eq!(usage.cache_read_input_tokens, Some(40));
    }

    #[test]
    fn unpack_missing_content_returns_error() {
        let body = json!({"model": "claude-3-haiku-20240307"});
        assert!(unpack_anthropic_response(&body).is_err());
    }

    // ── builders ──────────────────────────────────────────────────────────

    #[test]
    fn build_messages_success_structure() {
        let usage = TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
            cache_creation_input_tokens: Some(120),
            cache_read_input_tokens: Some(80),
        };
        let resp = build_messages_success(
            "msg_abc",
            "claude-3-5-sonnet-20241022",
            "Hi!",
            Some(&usage),
            &[],
            None,
        );
        assert_eq!(resp["type"], "message");
        assert_eq!(resp["content"][0]["text"], "Hi!");
        assert_eq!(resp["usage"]["input_tokens"], 10);
        assert_eq!(resp["usage"]["output_tokens"], 20);
        assert_eq!(resp["usage"]["cache_creation_input_tokens"], 120);
        assert_eq!(resp["usage"]["cache_read_input_tokens"], 80);
    }

    #[test]
    fn build_messages_delta_structure() {
        let delta = build_messages_delta("Hello");
        assert_eq!(delta["type"], "content_block_delta");
        assert_eq!(delta["delta"]["type"], "text_delta");
        assert_eq!(delta["delta"]["text"], "Hello");
    }

    #[test]
    fn build_messages_stop_structure() {
        let stop = build_messages_stop(None);
        assert_eq!(stop["type"], "message_delta");
        assert_eq!(stop["delta"]["stop_reason"], "end_turn");
    }

    // ── tool_use parsing ─────────────────────────────────────────────────

    #[test]
    fn unpack_response_parses_tool_use_blocks() {
        let body = json!({
            "id": "msg_tc",
            "type": "message",
            "role": "assistant",
            "model": "claude-3-5-sonnet-20241022",
            "content": [
                {"type": "text", "text": "Let me check the weather."},
                {
                    "type": "tool_use",
                    "id": "toolu_abc123",
                    "name": "get_weather",
                    "input": {"location": "NYC", "unit": "celsius"}
                }
            ],
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 10, "output_tokens": 20}
        });
        let resp = unpack_anthropic_response(&body).unwrap();
        assert_eq!(resp.text, "Let me check the weather.");
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].id.as_deref(), Some("toolu_abc123"));
        assert_eq!(resp.tool_calls[0].name.as_deref(), Some("get_weather"));
        assert_eq!(resp.tool_calls[0].call_type, "function");
        // arguments should be JSON-encoded string of the input object
        let args: serde_json::Value =
            serde_json::from_str(resp.tool_calls[0].arguments.as_deref().unwrap()).unwrap();
        assert_eq!(args["location"], "NYC");
        assert_eq!(resp.finish_reason.as_deref(), Some("tool_calls"));
    }

    #[test]
    fn unpack_response_multiple_tool_use_blocks() {
        let body = json!({
            "id": "msg_multi",
            "model": "claude-3-5-sonnet-20241022",
            "content": [
                {
                    "type": "tool_use",
                    "id": "toolu_1",
                    "name": "search",
                    "input": {"q": "rust"}
                },
                {
                    "type": "tool_use",
                    "id": "toolu_2",
                    "name": "calculator",
                    "input": {"expr": "1+1"}
                }
            ],
            "stop_reason": "tool_use"
        });
        let resp = unpack_anthropic_response(&body).unwrap();
        assert_eq!(resp.tool_calls.len(), 2);
        assert_eq!(resp.tool_calls[0].id.as_deref(), Some("toolu_1"));
        assert_eq!(resp.tool_calls[1].id.as_deref(), Some("toolu_2"));
    }

    #[test]
    fn normalize_assistant_tool_use_into_canonical_tool_calls() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{
                "role": "assistant",
                "content": [
                    {"type": "text", "text": "Checking"},
                    {"type": "tool_use", "id": "toolu_1", "name": "lookup", "input": {"city": "Tokyo"}}
                ]
            }]
        });
        let req = normalize_messages(body).unwrap();
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, MessageRole::Assistant);
        assert_eq!(req.messages[0].text_content(), "Checking");
        assert_eq!(req.messages[0].tool_calls.len(), 1);
        assert_eq!(req.messages[0].tool_calls[0].id.as_deref(), Some("toolu_1"));
    }

    #[test]
    fn normalize_user_tool_result_into_tool_message() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "Here is the tool result"},
                    {"type": "tool_result", "tool_use_id": "toolu_1", "content": [{"type": "text", "text": "{\"ok\":true}"}]}
                ]
            }]
        });
        let req = normalize_messages(body).unwrap();
        assert_eq!(req.messages.len(), 2);
        assert_eq!(req.messages[0].role, MessageRole::User);
        assert_eq!(req.messages[1].role, MessageRole::Tool);
        assert_eq!(req.messages[1].tool_call_id.as_deref(), Some("toolu_1"));
        assert_eq!(req.messages[1].text_content(), "{\"ok\":true}");
    }

    #[test]
    fn normalize_tool_choice_any_to_required() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "tool_choice": { "type": "any" },
            "messages": [{
                "role": "user",
                "content": "hi"
            }]
        });
        let req = normalize_messages(body).unwrap();
        assert_eq!(req.tool_choice, Some(json!("required")));
    }

    // ── unknown field preservation ───────────────────────────────────────

    #[test]
    fn normalize_captures_unknown_fields_in_extra() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 1024,
            "context_management": {"enabled": true},
            "metadata": {"user_id": "u-123"},
            "custom_vendor_field": 42,
        });
        let req = normalize_messages(body).unwrap();
        assert_eq!(
            req.extra.get("context_management"),
            Some(&json!({"enabled": true}))
        );
        assert_eq!(
            req.extra.get("metadata"),
            Some(&json!({"user_id": "u-123"}))
        );
        assert_eq!(req.extra.get("custom_vendor_field"), Some(&json!(42)));
        assert_eq!(req.extra.get("max_tokens"), Some(&json!(1024)));
    }

    // ── OpenAI SSE → Anthropic SSE translation ─────────────────────────

    #[tokio::test]
    async fn translate_openai_stream_produces_anthropic_events() {
        use futures::StreamExt;

        let events = vec![
            "data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hi\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
        ];

        let chunks: Vec<Result<Bytes, rquest::Error>> = events
            .into_iter()
            .map(|s| Ok(Bytes::from(s.to_string())))
            .collect();

        let inner_stream = futures::stream::iter(chunks);
        let mut translated = Box::pin(translate_openai_sse_to_anthropic(
            inner_stream,
            "gpt-4o".to_string(),
        ));

        let mut collected: Vec<String> = Vec::new();
        while let Some(Ok(bytes)) = translated.next().await {
            collected.push(String::from_utf8(bytes.to_vec()).unwrap());
        }

        // Verify we have output events
        assert!(!collected.is_empty(), "expected some output events");

        let full_output = collected.join("");

        // Must contain all Anthropic event types
        assert!(
            full_output.contains("event: message_start"),
            "missing message_start"
        );
        assert!(
            full_output.contains("event: content_block_delta"),
            "missing content_block_delta"
        );
        assert!(
            full_output.contains("event: content_block_stop"),
            "missing content_block_stop"
        );
        assert!(
            full_output.contains("event: message_delta"),
            "missing message_delta"
        );
        assert!(
            full_output.contains("event: message_stop"),
            "missing message_stop"
        );

        // Verify the delta contains the text
        assert!(
            full_output.contains("\"text\":\"Hi\""),
            "missing text delta 'Hi'"
        );
    }

    #[tokio::test]
    async fn translate_openai_tool_stream_produces_tool_use_events() {
        use futures::StreamExt;

        let events = vec![
            "data: {\"id\":\"chatcmpl-2\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"weather\",\"arguments\":\"{\\\"city\\\":\"}}]},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-2\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"Hangzhou\\\"}\"}}]},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-2\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}],\"usage\":{\"prompt_tokens\":8,\"completion_tokens\":5,\"total_tokens\":13}}\n\n",
            "data: [DONE]\n\n",
        ];

        let chunks: Vec<Result<Bytes, rquest::Error>> = events
            .into_iter()
            .map(|entry| Ok(Bytes::from(entry.to_string())))
            .collect();

        let inner_stream = futures::stream::iter(chunks);
        let mut translated = Box::pin(translate_openai_sse_to_anthropic(
            inner_stream,
            "gpt-4o".to_string(),
        ));

        let mut collected: Vec<String> = Vec::new();
        while let Some(Ok(bytes)) = translated.next().await {
            collected.push(String::from_utf8(bytes.to_vec()).unwrap());
        }

        let full_output = collected.join("");
        assert!(full_output.contains("event: message_start"));
        assert!(full_output.contains("\"type\":\"tool_use\""));
        assert!(full_output.contains("\"name\":\"weather\""));
        assert!(full_output.contains("\"type\":\"input_json_delta\""));
        assert!(full_output.contains("{\\\"city\\\":"));
        assert!(full_output.contains("\\\"Hangzhou\\\"}"));
        assert!(full_output.contains("\"stop_reason\":\"tool_use\""));
        assert!(full_output.contains("\"output_tokens\":5"));
        assert!(full_output.contains("event: message_stop"));
    }
}
