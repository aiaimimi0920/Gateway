//! Exercise River through the existing formatter/fingerprint/sender owners only.
use super::*;
use axum::{extract::State, http::HeaderMap as AxumHeaders, routing::post, Json, Router};
use std::sync::{Arc, Mutex};

type Calls = Arc<Mutex<Vec<(String, AxumHeaders, Value)>>>;

async fn reply(
    State(calls): State<Calls>,
    uri: axum::http::Uri,
    headers: AxumHeaders,
    Json(body): Json<Value>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    calls
        .lock()
        .unwrap()
        .push((uri.path().into(), headers, body.clone()));
    let responses = uri.path().ends_with("/responses");
    if body["stream"] == true {
        let data = if responses {
            "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"OK\"}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_fixture\",\"status\":\"completed\"}}\n\n"
        } else {
            "data: {\"id\":\"chat_fixture\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"OK\"},\"finish_reason\":null}]}\n\ndata: [DONE]\n\n"
        };
        ([("content-type", "text/event-stream")], data).into_response()
    } else if responses {
        Json(json!({"id":"resp_fixture","object":"response","status":"completed","model":"org/exact",
            "output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"OK"}]}],
            "usage":{"input_tokens":2,"output_tokens":1,"total_tokens":3}})).into_response()
    } else {
        Json(json!({"id":"chat_fixture","model":"org/exact","choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":2,"completion_tokens":1,"total_tokens":3}})).into_response()
    }
}

#[tokio::test]
async fn river_chat_and_responses_buffered_sse_use_new_flow_and_exact_key() {
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/v1/chat/completions", post(reply))
        .route("/v1/responses", post(reply))
        .with_state(calls.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let preset = crate::preset::get_builtin_preset("river-api").unwrap();
    let overrides =
        serde_json::from_value(json!({"base_url":base,"api_key":"rv_synthetic"})).unwrap();
    let mut payload = crate::preset::compile_provider_account(&preset, &overrides);
    payload.call_flow = Some(
        serde_json::from_value(
            json!({"call_adapter":"auto","fingerprint_adapter":"auto","special_adapter":"auto"}),
        )
        .unwrap(),
    );
    let mut client = UpstreamClient::new(3);
    client.http = rquest::Client::builder().no_proxy().build().unwrap();
    for responses in [false, true] {
        for stream in [false, true] {
            let req = if responses {
                responses::normalize_responses(json!({"model":"org/exact","input":"hello","stream":stream,"store":false,
                    "tools":[{"type":"function","name":"lookup","parameters":{"type":"object","properties":{}}}]})).unwrap()
            } else {
                openai::normalize_chat_completions(json!({"model":"org/exact","stream":stream,
                    "messages":[{"role":"user","content":[{"type":"text","text":"hello"},{"type":"image_url","image_url":{"url":"https://example.com/image.png"}}]}],
                    "tools":[{"type":"function","function":{"name":"lookup","parameters":{"type":"object","properties":{}}}}]})).unwrap()
            };
            if stream {
                let UpstreamStreamingResponse::Http(response) = client
                    .execute_stream(&payload, &req, "org/exact", None)
                    .await
                    .unwrap()
                else {
                    panic!("HTTP required")
                };
                let output = response.text().await.unwrap();
                assert!(output.contains("OK"));
                assert!(output.contains(if responses {
                    "response.completed"
                } else {
                    "[DONE]"
                }));
            } else {
                let response = client
                    .execute(&payload, &req, "org/exact", None)
                    .await
                    .unwrap();
                assert_eq!(response.text, "OK");
                assert_eq!(response.model, "org/exact");
            }
        }
    }
    let recorded = calls.lock().unwrap().clone();
    assert_eq!(recorded.len(), 4);
    for (index, (path, headers, body)) in recorded.iter().enumerate() {
        assert_eq!(
            path,
            if index >= 2 {
                "/v1/responses"
            } else {
                "/v1/chat/completions"
            }
        );
        assert_eq!(headers["authorization"], "Bearer rv_synthetic");
        assert!(!headers.contains_key("cookie"));
        assert_eq!(body["model"], "org/exact");
        assert_eq!(body["stream"], index % 2 == 1);
        assert!(body["tools"].is_array());
        assert!(body.get("call_flow").is_none());
        assert!(!body.to_string().contains("rv_synthetic"));
        if index < 2 {
            assert!(body["messages"].to_string().contains("image_url"));
        }
    }
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}

#[test]
fn river_identity_and_compile_gate_do_not_use_substring_brand_matching() {
    use crate::{implementation_lines as lines, protocol::registry};
    assert_eq!(
        registry::default_protocol_profile_for_preset("river-api"),
        "river"
    );
    assert_eq!(
        lines::required_feature_for_protocol_profile("river"),
        Some("line-river-official-api")
    );
    assert!(lines::is_protocol_profile_compiled_in("river"));
    let preset = crate::preset::get_builtin_preset("river-api").unwrap();
    assert!(preset.default_model.is_none() && preset.session_auth.is_none());
    for (base, expected) in [
        ("https://api.river.ai/v1", Some("river")),
        ("https://api.river.ai.evil.invalid/v1", None),
        ("https://example.com/api.river.ai/v1", None),
    ] {
        let payload = serde_json::from_value(
            json!({"adapter":"openai_compatible","base_url":base,"api_key":"rv_synthetic"}),
        )
        .unwrap();
        assert_eq!(
            lines::line_for_payload(&payload).map(|line| line.canonical_protocol_profile()),
            expected
        );
    }
}

#[test]
fn river_keep_cannot_retarget_an_existing_key_without_secret_access() {
    use crate::console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch};
    let active = serde_json::from_value(
        json!({"providers":[{"id":"river","preset":"river-api","base_url":"https://api.river.ai/v1",
        "credentials":[{"id":"one","api_key":"rv_synthetic"}]}]}),
    )
    .unwrap();
    let redacted = redact_route_document(&active).unwrap();
    let patches = [SecretPatch::keep("/providers/0/credentials/0/api_key")];
    let mut metadata = redacted.document.clone();
    metadata.providers[0].label = Some("new label".into());
    assert!(resolve_secret_patches(&active, metadata, &patches).is_ok());
    let mut foreign = redacted.document;
    foreign.providers[0].base_url = "https://foreign.invalid/v1".into();
    assert_eq!(
        resolve_secret_patches(&active, foreign, &patches)
            .unwrap_err()
            .code(),
        "secret_keep_destination_changed"
    );
}
