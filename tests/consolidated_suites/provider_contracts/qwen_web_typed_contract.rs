//! Typed channel ownership remains present when optional reverse transport is disabled.
use neuro_gateway::{
    protocol::{call_flow, openai},
    routing::candidate::ProviderAccountPayload,
};
use serde_json::json;
fn payload() -> ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":"qwen_web_compatible","base_url":"https://chat.qwen.ai/api/v1","api_key":"synthetic-token",
        "call_flow":{"special_adapter":"qwen_web","fingerprint_adapter":"session"}})).unwrap()
}
#[test]
fn qwen_web_typed_preparation_keeps_its_executor_and_material() {
    let p = payload();
    let flow = call_flow::prepare(&p).unwrap();
    assert_eq!(flow.executor, "qwen_web_compatible");
    assert_eq!(flow.payload.api_key, "synthetic-token");
}
#[cfg(not(feature = "line-qwen-web-reverse"))]
#[tokio::test]
async fn qwen_web_typed_disabled_line_does_not_fall_back_to_legacy_transport() {
    let request = openai::normalize_chat_completions(
        json!({"model":"synthetic-model","messages":[{"role":"user","content":"test"}]}),
    )
    .unwrap();
    let client = neuro_gateway::upstream::client::UpstreamClient::new(1);
    let p = payload();
    let error = client
        .execute(&p, &request, "synthetic-model", None)
        .await
        .unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("qwen_web_current_line_disabled")
    );
    let error = client
        .execute_stream(&p, &request, "synthetic-model", None)
        .await
        .err()
        .unwrap();
    assert_eq!(
        error.code.as_deref(),
        Some("qwen_web_current_line_disabled")
    );
}
