use std::collections::HashMap;

use serde_json::Value;

use super::*;
use crate::routing::candidate::ProviderAccountPayload;

fn make_payload() -> ProviderAccountPayload {
    ProviderAccountPayload {
        adapter: GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER.to_string(),
        base_url: "https://gemini.google.com".to_string(),
        api_key: "unused".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: Some(
            "credential-runtime/gemini-canvas/demo/storage-state.json".to_string(),
        ),
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
        extra_body: Some(HashMap::from([
            (
                "shareId".to_string(),
                Value::String("canvas-share-123".to_string()),
            ),
            (
                "canvasRelayWsEndpoint".to_string(),
                Value::String("ws://127.0.0.1:7861/ws".to_string()),
            ),
            (
                "canvasRelayClientLabel".to_string(),
                Value::String("canvas-browser-a".to_string()),
            ),
        ])),
        session_auth: None,
        keepalive: None,
    }
}

#[test]
fn relay_config_reads_runtime_and_browser_owned_fields() {
    let config = relay_config_from_payload(&make_payload()).expect("relay config");
    assert_eq!(
        config.runtime_state_object_key,
        "credential-runtime/gemini-canvas/demo/storage-state.json"
    );
    assert_eq!(config.share_id, "canvas-share-123");
    assert_eq!(
        config.relay_ws_endpoint.as_deref(),
        Some("ws://127.0.0.1:7861/ws")
    );
    assert_eq!(config.client_label.as_deref(), Some("canvas-browser-a"));
}

#[test]
fn browser_executor_input_keeps_request_spec_shape() {
    let relay = relay_config_from_payload(&make_payload()).expect("relay config");
    let spec = build_browser_relay_request_spec(
        "req-1",
        "req-1:attempt-1",
        "POST",
        Some(
            "https://generativelanguage.googleapis.com/v1beta/models/demo:generateContent"
                .to_string(),
        ),
        None,
        HashMap::from([("content-type".to_string(), "application/json".to_string())]),
        Some("{\"model\":\"demo\"}".to_string()),
    );
    let value = build_browser_executor_invocation_input(&relay, &spec);
    assert_eq!(
        value["provider"].as_str(),
        Some(GEMINI_CANVAS_BROWSER_RELAY_PROVIDER_KEY)
    );
    assert_eq!(
        value["input"]["requestSpec"]["requestId"].as_str(),
        Some("req-1")
    );
    assert_eq!(
        value["input"]["requestSpec"]["headers"]["content-type"].as_str(),
        Some("application/json")
    );
}

#[test]
fn relay_config_derives_share_id_from_share_url_when_id_missing() {
    let mut payload = make_payload();
    let extra = payload.extra_body.as_mut().expect("extra");
    extra.remove("shareId");
    extra.insert(
        "shareUrl".to_string(),
        Value::String("https://gemini.google.com/share/derived-share-456".to_string()),
    );

    let config = relay_config_from_payload(&payload).expect("relay config");
    assert_eq!(config.share_id, "derived-share-456");
}

#[test]
fn relay_config_falls_back_to_default_share_id_when_share_fields_missing() {
    let mut payload = make_payload();
    let extra = payload.extra_body.as_mut().expect("extra");
    extra.remove("shareId");
    extra.remove("shareUrl");

    let config = relay_config_from_payload(&payload).expect("relay config");
    assert_eq!(
        config.share_id,
        crate::protocol::gemini_canvas::GEMINI_CANVAS_DEFAULT_SHARE_ID
    );
}
