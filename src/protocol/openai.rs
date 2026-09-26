// ---------------------------------------------------------------------------
// OpenAI protocol adapter — pack / unpack / builder helpers
//
// Converts between CanonicalRelayRequest/Response and the OpenAI
// chat/completions wire format.
// ---------------------------------------------------------------------------

mod normalize;
mod pack;
mod response_builders;
mod response_unpack;
mod stream_translate;
mod tool_calls;

pub use normalize::{
    normalize_audio_speech, normalize_audio_transcriptions, normalize_chat_completions,
    normalize_embeddings, normalize_legacy_completions,
};
pub use pack::pack_openai;
pub use response_builders::{
    build_chat_completions_delta, build_chat_completions_stop, build_chat_completions_success,
    build_legacy_completions_delta, build_legacy_completions_stop,
    build_legacy_completions_success,
};
pub use response_unpack::unpack_openai_response;
pub use stream_translate::translate_openai_chat_sse_to_legacy_completions;

use response_unpack::map_openai_finish_reason;

#[cfg(test)]
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalToolCall, ContentPart, MessageRole, TokenUsage,
};
#[cfg(test)]
use pack::pack_openai_message;
#[cfg(test)]
use serde_json::Value;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod normalize_tests;
#[cfg(test)]
mod pack_tests;
#[cfg(test)]
mod response_unpack_tests;
#[cfg(test)]
mod stream_translate_tests;
#[cfg(test)]
mod tool_calls_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── pack_openai ───────────────────────────────────────────────────────

    #[test]
    fn pack_standard_model_uses_max_tokens() {
        let body = json!({
            "model": "gpt-4o",
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 512,
        });
        let req = normalize_chat_completions(body).unwrap();
        let packed = pack_openai(&req, "gpt-4o", false);
        assert_eq!(packed["max_tokens"], json!(512));
        assert!(
            packed.get("max_completion_tokens").is_none()
                || packed["max_completion_tokens"].is_null()
        );
    }

    #[test]
    fn pack_reasoning_model_uses_max_completion_tokens() {
        let body = json!({
            "model": "o1-preview",
            "messages": [{"role": "user", "content": "reason about this"}],
            "max_tokens": 2000,
        });
        let req = normalize_chat_completions(body).unwrap();
        let packed = pack_openai(&req, "o1-preview", false);
        assert_eq!(packed["max_completion_tokens"], json!(2000));
    }

    #[test]
    fn pack_inserts_system_message_first() {
        let body = json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": "You are helpful."},
                {"role": "user", "content": "Hello"}
            ]
        });
        let req = normalize_chat_completions(body).unwrap();
        let packed = pack_openai(&req, "gpt-4o", false);
        let msgs = packed["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[0]["content"], "You are helpful.");
        assert_eq!(msgs[1]["role"], "user");
    }

    // ── build_chat_completions_success ────────────────────────────────────

    #[test]
    fn build_success_response_structure() {
        let usage = TokenUsage {
            prompt_tokens: 5,
            completion_tokens: 10,
            total_tokens: 15,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        };
        let resp = build_chat_completions_success(
            "chatcmpl-abc",
            1700000000,
            "gpt-4o",
            "Hi!",
            Some(&usage),
            &[],
            None,
        );
        assert_eq!(resp["id"], "chatcmpl-abc");
        assert_eq!(resp["object"], "chat.completion");
        assert_eq!(resp["choices"][0]["message"]["content"], "Hi!");
        assert_eq!(resp["choices"][0]["finish_reason"], "stop");
        assert_eq!(resp["usage"]["total_tokens"], 15);
    }

    #[test]
    fn build_success_response_propagates_finish_reason() {
        let resp = build_chat_completions_success(
            "chatcmpl-xyz",
            1700000000,
            "gpt-4o",
            "Done",
            None,
            &[],
            Some("tool_calls"),
        );
        assert_eq!(resp["choices"][0]["finish_reason"], "tool_calls");
    }

    #[test]
    fn build_success_response_defaults_to_tool_calls_when_tools_present() {
        let resp = build_chat_completions_success(
            "chatcmpl-tools",
            1700000000,
            "gpt-4o",
            "",
            None,
            &[CanonicalToolCall {
                id: Some("call_1".to_string()),
                call_type: "function".to_string(),
                name: Some("weather".to_string()),
                arguments: Some("{\"city\":\"Hangzhou\"}".to_string()),
                raw: std::collections::HashMap::new(),
            }],
            None,
        );
        assert_eq!(resp["choices"][0]["finish_reason"], "tool_calls");
    }

    #[test]
    fn pack_tool_message_stringifies_json_content() {
        let message = CanonicalMessage {
            role: MessageRole::Tool,
            content: vec![ContentPart::Json {
                value: json!({"city":"Hangzhou","condition":"sunny"}),
            }],
            name: Some("weather".to_string()),
            tool_call_id: Some("call_weather".to_string()),
            tool_calls: vec![],
        };
        let packed = pack_openai_message(&message);
        assert_eq!(packed["role"], "tool");
        assert_eq!(
            packed["content"],
            json!("{\"city\":\"Hangzhou\",\"condition\":\"sunny\"}")
        );
        assert_eq!(packed["tool_call_id"], "call_weather");
    }

    #[test]
    fn normalize_preserves_unknown_vendor_fields() {
        let body = json!({
            "model": "gpt-4o",
            "messages": [{"role": "user", "content": "hi"}],
            "temperature": 0.5,
            "my_vendor_flag": {"enabled": true},
            "another_ext": 42,
        });
        let req = normalize_chat_completions(body).unwrap();
        assert_eq!(
            req.extra.get("my_vendor_flag"),
            Some(&json!({"enabled": true}))
        );
        assert_eq!(req.extra.get("another_ext"), Some(&json!(42)));
        assert_eq!(req.extra.get("temperature"), Some(&json!(0.5)));
    }

    #[test]
    fn pack_unknown_vendor_fields_roundtrip() {
        let body = json!({
            "model": "gpt-4o",
            "messages": [{"role": "user", "content": "hi"}],
            "my_vendor_flag": true,
        });
        let req = normalize_chat_completions(body).unwrap();
        let packed = pack_openai(&req, "gpt-4o", false);
        assert_eq!(packed["my_vendor_flag"], json!(true));
    }

    #[test]
    fn pack_maps_anthropic_tool_choice_any_to_required() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{"role": "user", "content": "hi"}],
            "tool_choice": {"type": "any"}
        });
        let req = crate::protocol::anthropic::normalize_messages(body).unwrap();
        let packed = pack_openai(&req, "gpt-4o", false);
        assert_eq!(packed["tool_choice"], json!("required"));
    }

    #[test]
    fn pack_maps_anthropic_specific_tool_choice_to_openai_function_object() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{"role": "user", "content": "hi"}],
            "tool_choice": {"type": "tool", "name": "weather"}
        });
        let req = crate::protocol::anthropic::normalize_messages(body).unwrap();
        let packed = pack_openai(&req, "gpt-4o", false);
        assert_eq!(
            packed["tool_choice"],
            json!({"type": "function", "function": {"name": "weather"}})
        );
    }

    // ── build_chat_completions_delta ──────────────────────────────────────

    #[test]
    fn build_delta_chunk_structure() {
        let chunk = build_chat_completions_delta("chatcmpl-abc", 1700000000, "gpt-4o", "Hello");
        assert_eq!(chunk["object"], "chat.completion.chunk");
        assert_eq!(chunk["choices"][0]["delta"]["content"], "Hello");
        assert_eq!(chunk["choices"][0]["finish_reason"], Value::Null);
    }

    // ── build_chat_completions_stop ───────────────────────────────────────

    #[test]
    fn build_stop_chunk_structure() {
        let chunk = build_chat_completions_stop("chatcmpl-abc", 1700000000, "gpt-4o", None);
        assert_eq!(chunk["object"], "chat.completion.chunk");
        assert_eq!(chunk["choices"][0]["finish_reason"], "stop");
    }

    #[test]
    fn build_legacy_completion_success_structure() {
        let usage = TokenUsage {
            prompt_tokens: 4,
            completion_tokens: 6,
            total_tokens: 10,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        };
        let resp = build_legacy_completions_success(
            "cmpl-abc",
            1700000000,
            "gpt-5.4",
            "legacy ok",
            Some(&usage),
            None,
        );
        assert_eq!(resp["object"], "text_completion");
        assert_eq!(resp["choices"][0]["text"], "legacy ok");
        assert_eq!(resp["choices"][0]["finish_reason"], "stop");
        assert_eq!(resp["usage"]["total_tokens"], 10);
    }

    #[test]
    fn build_legacy_completion_stop_uses_custom_finish_reason() {
        let resp =
            build_legacy_completions_stop("cmpl-abc", 1700000000, "gpt-5.4", None, Some("length"));
        assert_eq!(resp["choices"][0]["finish_reason"], "length");
    }
}
