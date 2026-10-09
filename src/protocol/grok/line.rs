// ---------------------------------------------------------------------------
// Grok protocol adapter — pack / unpack
//
// Converts between CanonicalRelayRequest/Response and the Grok (grok.com)
// web chat API wire format.
//
// Grok uses a custom NDJSON streaming protocol:
// - Endpoint: POST https://grok.com/rest/app-chat/conversations/new
// - Auth: SSO cookie (not Bearer token)
// - Request: custom JSON payload with message/modelName/modelMode fields
// - Response: NDJSON stream, each line is JSON with result.response.token
// ---------------------------------------------------------------------------

use bytes::Bytes;
use futures::Stream;
use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayResponse, TokenUsage};

mod packing;
pub use packing::{build_request_plan, pack_grok};

// ---------------------------------------------------------------------------
// accumulate_grok_stream
// ---------------------------------------------------------------------------

/// Accumulate a Grok NDJSON stream into a single canonical response.
///
/// Grok always returns NDJSON (even for "non-streaming" requests from the
/// gateway's perspective). Each line is a JSON object:
/// ```json
/// {"result":{"response":{"token":"Hello","isThinking":false}}}
/// ```
///
/// We read all lines, concatenate `token` values, and return a single response.
pub async fn accumulate_grok_stream(
    response: rquest::Response,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let body_str = response
        .text()
        .await
        .map_err(|e| GatewayError::server_error(format!("read grok body: {e}")))?;

    let mut content = String::new();
    let mut reported_model = model.to_string();
    let mut finish_reason = Some("stop".to_string());

    for line in body_str.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Strip SSE "data:" prefix if present
        let json_str = if let Some(data) = line.strip_prefix("data:") {
            data.trim()
        } else {
            line
        };

        if json_str == "[DONE]" {
            break;
        }

        let data: Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let resp = &data["result"]["response"];

        if let Some(token) = resp.get("token").and_then(|t| t.as_str()) {
            if !token.is_empty() {
                let is_thinking = resp
                    .get("isThinking")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if !is_thinking {
                    content.push_str(token);
                }
            }
        }

        if let Some(model_name) = resp
            .get("modelResponse")
            .and_then(|v| v.get("modelName"))
            .and_then(|v| v.as_str())
        {
            reported_model = model_name.to_string();
        }

        if resp.get("modelResponse").is_some() {
            finish_reason = Some("stop".to_string());
        }
    }

    Ok(CanonicalRelayResponse {
        model: reported_model,
        text: content,
        usage: Some(TokenUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        }),
        tool_calls: vec![],
        upstream_status: Some(200),
        finish_reason,
    })
}

// ---------------------------------------------------------------------------
// Grok NDJSON → OpenAI SSE translation (streaming)
// ---------------------------------------------------------------------------

/// Parse a single Grok NDJSON line and convert to OpenAI SSE format.
///
/// Returns `None` for non-content lines (metadata, image generation, etc).
/// Returns `Some(bytes)` for text tokens and the final `[DONE]` sentinel.
pub fn translate_grok_ndjson_to_openai_sse(
    line: &[u8],
    model: &str,
    response_id: &str,
    created: i64,
) -> Option<Vec<u8>> {
    let line_str = std::str::from_utf8(line).ok()?;
    let line_str = line_str.trim();

    if line_str.is_empty() {
        return None;
    }

    // Strip SSE "data:" prefix if present
    let json_str = line_str
        .strip_prefix("data:")
        .map(|s| s.trim())
        .unwrap_or(line_str);

    if json_str == "[DONE]" {
        return Some(b"data: [DONE]\n\n".to_vec());
    }

    let data: Value = serde_json::from_str(json_str).ok()?;
    let resp = data.get("result")?.get("response")?;

    // Extract text token
    if let Some(token) = resp.get("token").and_then(|t| t.as_str()) {
        if token.is_empty() {
            return None;
        }

        let is_thinking = resp
            .get("isThinking")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Skip thinking tokens for now (could be included via reasoning_content)
        if is_thinking {
            return None;
        }

        let chunk = json!({
            "id": response_id,
            "object": "chat.completion.chunk",
            "created": created,
            "model": model,
            "choices": [{
                "index": 0,
                "delta": {"content": token},
                "finish_reason": null
            }]
        });

        let sse_line = format!("data: {}\n\n", chunk);
        return Some(sse_line.into_bytes());
    }

    if resp.get("modelResponse").is_some() {
        let chunk = json!({
            "id": response_id,
            "object": "chat.completion.chunk",
            "created": created,
            "model": model,
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        });

        let sse_line = format!("data: {}\n\n", chunk);
        return Some(sse_line.into_bytes());
    }

    None
}

/// Wrap a Grok NDJSON byte stream and translate it to OpenAI SSE on-the-fly.
///
/// Grok streams newline-delimited JSON, not SSE. This adapter buffers partial
/// lines, translates each complete line into OpenAI `chat.completion.chunk`
/// frames, and guarantees a terminal `data: [DONE]`.
pub fn translate_grok_stream(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    let response_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let state = GrokTranslatorState {
        buffer: Vec::new(),
        model,
        response_id,
        created,
        emitted_done: false,
    };

    futures::stream::unfold(
        (
            Box::pin(inner)
                as std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
            state,
            false,
        ),
        |(mut stream, mut st, done)| async move {
            use futures::StreamExt;

            if done {
                return None;
            }

            loop {
                if let Some(pos) = st.buffer.iter().position(|&b| b == b'\n') {
                    let line: Vec<u8> = st.buffer.drain(..=pos).collect();
                    if let Some(translated) = translate_grok_ndjson_to_openai_sse(
                        &line,
                        &st.model,
                        &st.response_id,
                        st.created,
                    ) {
                        if translated == b"data: [DONE]\n\n".to_vec() {
                            st.emitted_done = true;
                            return Some((Ok(Bytes::from(translated)), (stream, st, true)));
                        }
                        return Some((Ok(Bytes::from(translated)), (stream, st, false)));
                    }
                    continue;
                }

                match stream.next().await {
                    Some(Ok(chunk)) => {
                        st.buffer.extend_from_slice(&chunk);
                    }
                    Some(Err(e)) => {
                        return Some((Err(e), (stream, st, true)));
                    }
                    None => {
                        if !st.buffer.is_empty() {
                            let remaining = std::mem::take(&mut st.buffer);
                            if let Some(translated) = translate_grok_ndjson_to_openai_sse(
                                &remaining,
                                &st.model,
                                &st.response_id,
                                st.created,
                            ) {
                                if translated == b"data: [DONE]\n\n".to_vec() {
                                    st.emitted_done = true;
                                    return Some((Ok(Bytes::from(translated)), (stream, st, true)));
                                }
                                return Some((Ok(Bytes::from(translated)), (stream, st, false)));
                            }
                        }

                        if !st.emitted_done {
                            st.emitted_done = true;
                            return Some((
                                Ok(Bytes::from_static(b"data: [DONE]\n\n")),
                                (stream, st, true),
                            ));
                        }

                        return None;
                    }
                }
            }
        },
    )
}

struct GrokTranslatorState {
    buffer: Vec<u8>,
    model: String,
    response_id: String,
    created: i64,
    emitted_done: bool,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::packing::resolve_grok_model;
    use super::*;
    use crate::protocol::canonical::{
        CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole,
        ProtocolFamily,
    };
    use crate::routing::candidate::ProviderAccountPayload;
    use crate::upstream::common::RequestPlan;
    use rquest::Method;
    use std::collections::HashMap;

    fn make_messages(texts: &[(&str, MessageRole)]) -> Vec<CanonicalMessage> {
        texts
            .iter()
            .map(|(text, role)| CanonicalMessage {
                role: *role,
                content: vec![ContentPart::Text {
                    text: text.to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            })
            .collect()
    }

    fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            discovered_protocols: Vec::new(),
            adapter: adapter.to_string(),
            base_url: base_url.to_string(),
            api_key: "sk-test".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: None,
            session_auth: None,
            keepalive: None,
        }
    }

    fn make_request(protocol: ProtocolFamily, endpoint: EndpointKind) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: protocol,
            endpoint_kind: endpoint,
            requested_model: Some("test-model".to_string()),
            stream: false,
            messages: make_messages(&[("hello", MessageRole::User)]),
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    fn assert_grok_chat_surface_plan(plan: &RequestPlan) {
        assert_eq!(plan.url, "https://grok.com/rest/app-chat/conversations/new");
        assert_eq!(plan.body.as_ref().unwrap()["modelName"], "grok-3");
        assert_eq!(plan.body.as_ref().unwrap()["temporary"], true);
        assert_eq!(plan.response_kind, EndpointKind::ChatCompletions);
    }

    #[test]
    fn pack_grok_single_user_message() {
        let req = CanonicalRelayRequest {
            protocol_family: crate::protocol::canonical::ProtocolFamily::OpenAi,
            endpoint_kind: crate::protocol::canonical::EndpointKind::ChatCompletions,
            requested_model: Some("grok-3".to_string()),
            stream: false,
            messages: make_messages(&[("Hello", MessageRole::User)]),
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: std::collections::HashMap::new(),
        };

        let body = pack_grok(&req, "grok-3");
        assert_eq!(body["message"], "Hello");
        assert_eq!(body["modelName"], "grok-3");
        assert_eq!(body["modelMode"], "MODEL_MODE_FAST");
        assert_eq!(body["temporary"], true);
    }

    #[test]
    fn pack_grok_multi_message_combines_with_roles() {
        let req = CanonicalRelayRequest {
            protocol_family: crate::protocol::canonical::ProtocolFamily::OpenAi,
            endpoint_kind: crate::protocol::canonical::EndpointKind::ChatCompletions,
            requested_model: Some("grok-3".to_string()),
            stream: false,
            messages: make_messages(&[
                ("You are helpful", MessageRole::System),
                ("Hi", MessageRole::User),
            ]),
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: std::collections::HashMap::new(),
        };

        let body = pack_grok(&req, "grok-3");
        let msg = body["message"].as_str().unwrap();
        assert!(msg.contains("System: You are helpful"));
        assert!(msg.contains("User: Hi"));
    }

    #[test]
    fn resolve_grok_model_thinking() {
        assert_eq!(
            resolve_grok_model("grok-3-thinking"),
            ("grok-3", Some("MODEL_MODE_EXPERT"))
        );
        assert_eq!(
            resolve_grok_model("grok-3-heavy"),
            ("grok-3", Some("MODEL_MODE_HEAVY"))
        );
        assert_eq!(
            resolve_grok_model("grok-3"),
            ("grok-3", Some("MODEL_MODE_FAST"))
        );
    }

    #[test]
    fn grok_request_plan_preserves_requested_response_kind() {
        let mut payload = make_payload("grok_compatible", "https://grok.com");
        payload.chat_completions_path = Some("/rest/app-chat/conversations/new".to_string());
        let req = make_request(
            crate::protocol::canonical::ProtocolFamily::Anthropic,
            crate::protocol::canonical::EndpointKind::Messages,
        );

        let plan = build_request_plan(&payload, &req, "grok-3-thinking").unwrap();
        assert_eq!(plan.method, Method::POST);
        assert_eq!(plan.url, "https://grok.com/rest/app-chat/conversations/new");
        assert_eq!(
            plan.response_kind,
            crate::protocol::canonical::EndpointKind::Messages
        );
        assert_eq!(plan.body.as_ref().unwrap()["modelName"], "grok-3");
        assert_eq!(
            plan.body.as_ref().unwrap()["modelMode"],
            json!("MODEL_MODE_EXPERT")
        );
    }

    #[test]
    fn plan_grok_compatible_url_and_body() {
        let mut payload = make_payload("grok_compatible", "https://grok.com");
        payload.chat_completions_path = Some("/rest/app-chat/conversations/new".to_string());
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        let plan = build_request_plan(&payload, &req, "grok-3").unwrap();
        assert_grok_chat_surface_plan(&plan);
    }

    #[test]
    fn translate_ndjson_token_to_sse() {
        let line = br#"{"result":{"response":{"token":"Hello","isThinking":false}}}"#;
        let result = translate_grok_ndjson_to_openai_sse(line, "grok-3", "chatcmpl-1", 1234);
        assert!(result.is_some());
        let sse = String::from_utf8(result.unwrap()).unwrap();
        assert!(sse.starts_with("data: "));
        assert!(sse.contains("\"content\":\"Hello\""));
        assert!(sse.contains("\"model\":\"grok-3\""));
    }

    #[test]
    fn translate_ndjson_thinking_token_skipped() {
        let line = br#"{"result":{"response":{"token":"thinking...","isThinking":true}}}"#;
        let result = translate_grok_ndjson_to_openai_sse(line, "grok-3", "chatcmpl-1", 1234);
        assert!(result.is_none());
    }

    #[test]
    fn translate_ndjson_empty_line_skipped() {
        let result = translate_grok_ndjson_to_openai_sse(b"", "grok-3", "chatcmpl-1", 1234);
        assert!(result.is_none());
    }

    #[test]
    fn translate_ndjson_model_response_emits_stop_chunk() {
        let line = br#"{"result":{"response":{"modelResponse":{"modelName":"grok-3"}}}}"#;
        let result = translate_grok_ndjson_to_openai_sse(line, "grok-3", "chatcmpl-1", 1234);
        assert!(result.is_some());
        let sse = String::from_utf8(result.unwrap()).unwrap();
        let chunk: Value = serde_json::from_str(sse.trim_start_matches("data: ").trim()).unwrap();
        assert_eq!(chunk["choices"][0]["finish_reason"], "stop");
    }

    #[tokio::test]
    async fn translate_stream_buffers_lines_and_emits_done() {
        use futures::StreamExt;

        let chunks: Vec<Result<Bytes, rquest::Error>> = vec![
            Ok(Bytes::from_static(
                br#"{"result":{"response":{"token":"Hel","isThinking":false}}}"#,
            )),
            Ok(Bytes::from_static(
                b"\n{\"result\":{\"response\":{\"token\":\"lo\",\"isThinking\":false}}}\n",
            )),
        ];

        let inner = futures::stream::iter(chunks);
        let mut translated = Box::pin(translate_grok_stream(inner, "grok-3".to_string()));
        let mut collected = Vec::new();

        while let Some(Ok(bytes)) = translated.next().await {
            collected.push(String::from_utf8(bytes.to_vec()).unwrap());
        }

        assert_eq!(collected.len(), 3, "expected 2 content chunks + DONE");
        assert!(collected[0].contains("\"content\":\"Hel\""));
        assert!(collected[1].contains("\"content\":\"lo\""));
        assert_eq!(collected[2], "data: [DONE]\n\n");
    }
}
