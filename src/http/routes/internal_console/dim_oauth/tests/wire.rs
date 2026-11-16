//! Synthetic inference peers preserve the final validated path, headers and packed tool declarations.
use super::*;
use axum::{extract::OriginalUri, http::HeaderMap, routing::post, Json, Router};
use std::sync::Mutex;

pub(super) type Calls = Arc<Mutex<Vec<(String, HeaderMap, Value)>>>;
pub(super) fn router(calls: Calls) -> Router {
    let handler = move |OriginalUri(uri): OriginalUri,
                        headers: HeaderMap,
                        Json(body): Json<Value>| {
        let calls = calls.clone();
        async move {
            let path = uri.path().to_owned();
            assert_eq!(headers["authorization"], "Bearer synthetic-dim-access");
            assert!(!headers.contains_key("x-api-key"));
            assert!(!headers.contains_key("x-goog-api-key"));
            assert!(!uri.query().is_some_and(|q| q.contains("key=")));
            calls
                .lock()
                .unwrap()
                .push((path.clone(), headers, body.clone()));
            let stream = body["stream"] == true || path.ends_with(":streamGenerateContent");
            let (delta, terminal, buffered) = if path.ends_with("/chat/completions") {
                (
                    json!({"choices":[{"index":0,"delta":{"content":"OK"},"finish_reason":"stop"}]}),
                    "data: [DONE]\n\n".to_owned(),
                    json!({"id":"chat","model":"opaque-chat","choices":[{"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}]}),
                )
            } else if path.ends_with("/responses") {
                (json!({"type":"response.output_text.delta","delta":"OK"}),
                 "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp\",\"status\":\"completed\",\"output\":[]}}\n\n".to_owned(),
                 json!({"id":"resp","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"OK"}]}]}))
            } else if path.ends_with("/messages") {
                (json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"OK"}}),
                 "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\ndata: {\"type\":\"message_stop\"}\n\n".to_owned(),
                 json!({"id":"msg","type":"message","role":"assistant","model":"opaque-messages","content":[{"type":"text","text":"OK"}],"stop_reason":"end_turn","usage":{"input_tokens":1,"output_tokens":1}}))
            } else {
                let result = json!({"candidates":[{"content":{"role":"model","parts":[{"text":"OK"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":1,"candidatesTokenCount":1,"totalTokenCount":2}});
                (result.clone(), String::new(), result)
            };
            let (kind, body) = if stream {
                ("text/event-stream", format!("data: {delta}\n\n{terminal}"))
            } else {
                ("application/json", buffered.to_string())
            };
            ([("content-type", kind)], body)
        }
    };
    Router::new()
        .route("/v1/chat/completions", post(handler.clone()))
        .route("/v1/responses", post(handler.clone()))
        .route("/v1/messages", post(handler.clone()))
        .route("/v1beta/models/:method", post(handler))
}

pub(super) async fn probe_all(
    state: &Arc<AppState>,
    grant: &str,
    account: &str,
    peer: &fixture::Peer,
) {
    for (model, family) in [
        ("opaque-chat", "openai_chat"),
        ("opaque-responses", "openai_responses"),
        ("opaque-messages", "anthropic_messages"),
        ("opaque-gemini", "gemini_generate_content"),
    ] {
        let response = fixture::origin(
            peer.origin.clone(),
            call(
                state,
                "/v1/internal/gateway/console/providers/dim/probe",
                json!({"prompt":"Reply only OK","model":model,"credentialIds":[account]}),
                Some(grant),
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK, "{model}");
        let body = support::parse_json(response).await;
        assert_eq!(
            body["result"]["results"][0]["status"], "passed",
            "{model}: {body}"
        );
        assert!(
            body["result"]["results"][0]["message"]
                .as_str()
                .unwrap()
                .contains("OK"),
            "{model}: {body}"
        );
        fixture::assert_redacted(&body);
        let target = state
            .route_config
            .snapshot()
            .select_credential_probe_target(account)
            .unwrap();
        let identity = state
            .route_config
            .snapshot()
            .model_probe_identity(&target, model)
            .unwrap();
        assert_eq!(
            identity.protocol_profile.as_deref(),
            Some(crate::protocol::dim_oauth::runtime::protocol_profile(
                family
            ))
        );
    }
}

pub(super) async fn send_all(state: &AppState, peer: &fixture::Peer) {
    for model in [
        "opaque-chat",
        "opaque-responses",
        "opaque-messages",
        "opaque-gemini",
    ] {
        let candidate = state
            .route_config
            .snapshot()
            .resolve_candidates(Some(model))
            .into_iter()
            .find(|c| c.provider_account_id == "dim")
            .unwrap();
        let request = crate::protocol::openai::normalize_chat_completions(json!({"model":model,"stream":false,
            "messages":[{"role":"user","content":"hello"}],
            "tools":[{"type":"function","function":{"name":"weather","description":"weather","parameters":{"type":"object","properties":{}}}}]})).unwrap();
        let result = fixture::origin(
            peer.origin.clone(),
            state
                .upstream_client
                .execute(&candidate.payload, &request, model, None),
        )
        .await
        .unwrap();
        assert_eq!(result.text, "OK", "{model}");
        let expected_finish = if model == "opaque-responses" {
            "completed"
        } else {
            "stop"
        };
        assert_eq!(
            result.finish_reason.as_deref(),
            Some(expected_finish),
            "{model}"
        );
    }
    let calls = peer.inference.lock().unwrap();
    assert_eq!(calls.len(), 8);
    for (path, headers, body) in calls.iter().skip(4) {
        assert_eq!(headers["x-title"], "Neuro Gateway");
        let tools = body["tools"]
            .as_array()
            .expect("all four wires must serialize tool declarations");
        assert_eq!(tools.len(), 1, "{path}: {body}");
        assert!(tools[0].to_string().contains("weather"));
    }
}
