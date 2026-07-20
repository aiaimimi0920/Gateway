use serde_json::Value;

use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalToolCall, TokenUsage};
use crate::protocol::{gemini_api, gemini_canvas, tool_inject};

pub(crate) fn build_gemini_canvas_text_success_body(
    req: &CanonicalRelayRequest,
    model: &str,
    raw_text: &str,
    usage: Option<&TokenUsage>,
    direct_tool_calls: &[CanonicalToolCall],
) -> Value {
    let conversation_hint = gemini_canvas::latest_nonempty_user_text(req).or_else(|| {
        let text = req.messages_text().trim().to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    });
    let parsed = if direct_tool_calls.is_empty() {
        tool_inject::parse_tool_calls_from_text_with_context(
            raw_text,
            &req.tools,
            req.tool_choice.as_ref(),
            conversation_hint.as_deref(),
        )
    } else {
        tool_inject::ToolCallParseResult {
            tool_calls: direct_tool_calls.to_vec(),
            clean_text: raw_text.to_string(),
            had_tool_calls: true,
        }
    };
    let finish_reason = if parsed.tool_calls.is_empty() {
        Some("stop")
    } else {
        Some("tool_calls")
    };
    gemini_api::build_generate_content_success(
        model,
        parsed.clean_text.trim(),
        usage,
        &parsed.tool_calls,
        finish_reason,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::protocol::canonical::{
        CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole,
        ProtocolFamily, TokenUsage,
    };

    fn make_request(protocol: ProtocolFamily, endpoint: EndpointKind) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: protocol,
            endpoint_kind: endpoint,
            requested_model: Some("gpt-4o".to_string()),
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
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: serde_json::json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn build_gemini_canvas_text_success_body_trims_text_and_emits_usage() {
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        let usage = TokenUsage {
            prompt_tokens: 11,
            completion_tokens: 7,
            total_tokens: 18,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        };

        let body = build_gemini_canvas_text_success_body(
            &req,
            "gemini-2.5-pro",
            "  hello from canvas  ",
            Some(&usage),
            &[],
        );

        assert_eq!(body["modelVersion"], serde_json::json!("gemini-2.5-pro"));
        assert_eq!(
            body["candidates"][0]["finishReason"],
            serde_json::json!("STOP")
        );
        assert_eq!(
            body["candidates"][0]["content"]["parts"][0]["text"],
            serde_json::json!("hello from canvas")
        );
        assert_eq!(
            body["usageMetadata"]["promptTokenCount"],
            serde_json::json!(11)
        );
        assert_eq!(
            body["usageMetadata"]["candidatesTokenCount"],
            serde_json::json!(7)
        );
        assert_eq!(
            body["usageMetadata"]["totalTokenCount"],
            serde_json::json!(18)
        );
    }

    #[test]
    fn build_gemini_canvas_text_success_body_preserves_direct_tool_calls() {
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        let tool_calls = vec![CanonicalToolCall {
            id: Some("call_weather".to_string()),
            call_type: "function".to_string(),
            name: Some("weather".to_string()),
            arguments: Some("{\"city\":\"Hangzhou\"}".to_string()),
            raw: HashMap::new(),
        }];

        let body =
            build_gemini_canvas_text_success_body(&req, "gemini-2.5-pro", "", None, &tool_calls);

        let parts = body["candidates"][0]["content"]["parts"]
            .as_array()
            .expect("parts array");
        assert_eq!(parts.len(), 1);
        assert_eq!(
            parts[0]["functionCall"]["id"],
            serde_json::json!("call_weather")
        );
        assert_eq!(
            parts[0]["functionCall"]["name"],
            serde_json::json!("weather")
        );
        assert_eq!(
            parts[0]["functionCall"]["args"]["city"],
            serde_json::json!("Hangzhou")
        );
        assert_eq!(
            body["candidates"][0]["finishReason"],
            serde_json::json!("STOP")
        );
    }
}
