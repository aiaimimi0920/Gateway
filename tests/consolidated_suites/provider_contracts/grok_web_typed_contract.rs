//! Typed Web sessions must not be sent through the historical HTTP/NDJSON shortcut.
use neuro_gateway::{protocol::openai, routing::candidate::ProviderAccountPayload, upstream::grok};
use serde_json::json;

#[test]
#[cfg(feature = "line-grok-web-reverse-api")]
fn grok_web_typed_plan_cannot_fall_back_to_legacy_http() {
    let mut payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":"grok_compatible", "base_url":"https://grok.com", "api_key":"synthetic-session",
        "call_flow":{"special_adapter":"grok_web","fingerprint_adapter":"session"}
    }))
    .unwrap();
    let request = openai::normalize_chat_completions(json!({
        "model":"grok-chat-fast", "messages":[{"role":"user","content":"合成问题"}]
    }))
    .unwrap();
    let error = grok::build_request_plan(&payload, &request, "grok-chat-fast")
        .err()
        .expect("Typed Grok Web requires the current WS dispatcher, never legacy POST");
    assert_eq!(
        error.code.as_deref(),
        Some("grok_web_request_plan_unsupported")
    );
    payload.call_flow = None;
    assert!(grok::build_request_plan(&payload, &request, "grok-chat-fast").is_ok());
}

#[tokio::test]
#[cfg(not(feature = "line-grok-web-reverse-api"))]
async fn grok_web_typed_disabled_line_never_falls_back_to_legacy_transport() {
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":"grok_compatible","base_url":"https://grok.com","api_key":"synthetic-session",
        "call_flow":{"special_adapter":"grok_web","fingerprint_adapter":"session"}
    }))
    .unwrap();
    let request = openai::normalize_chat_completions(
        json!({"model":"grok-chat-fast","messages":[{"role":"user","content":"test"}]}),
    )
    .unwrap();
    let error = neuro_gateway::upstream::client::UpstreamClient::new(1)
        .execute(&payload, &request, "grok-chat-fast", None)
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("grok_web_line_disabled"));
}
