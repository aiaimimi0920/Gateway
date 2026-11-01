//! Synthetic contracts: no Google credentials, browser, or upstream traffic.
use neuro_gateway::{
    preset::get_builtin_preset,
    protocol::call_flow::prepare,
    protocol::canonical::EndpointKind,
    routing::candidate::{ProviderAccountPayload, ProviderExecutionMode},
};
use serde_json::json;
#[path = "console_contract_support/mod.rs"]
mod support;

#[test]
fn antigravity_has_its_own_http_executor_and_fingerprint() {
    let preset = get_builtin_preset("antigravity-oauth").expect("independent Antigravity preset");
    assert_eq!(preset.adapter, "antigravity_oauth_compatible");
    assert_eq!(
        neuro_gateway::protocol::registry::default_protocol_profile_for_preset(&preset.id),
        "antigravity_oauth"
    );
    assert!(preset.default_model.is_none());
    let payload: ProviderAccountPayload = serde_json::from_value(json!({"adapter":preset.adapter,
        "base_url":"https://daily-cloudcode-pa.googleapis.com/v1internal", "api_key":"fixture",
        "execution_mode":"browser_backed", "call_flow":{"special_adapter":"antigravity_oauth", "fingerprint_adapter":"antigravity"}})).unwrap();
    assert!(prepare(&payload).unwrap().executor_owns_fingerprint);
    assert_eq!(
        payload.resolve_execution_mode(EndpointKind::ChatCompletions),
        ProviderExecutionMode::DirectHttp
    );
}

#[test]
fn antigravity_fingerprint_cannot_leak_to_other_executors() {
    for adapter in ["openai_compatible", "stitch_web_compatible"] {
        let payload: ProviderAccountPayload = serde_json::from_value(json!({"adapter":adapter,
            "base_url":"https://example.invalid", "api_key":"fixture", "call_flow":{"fingerprint_adapter":"antigravity"}})).unwrap();
        assert!(prepare(&payload).is_err());
    }
}

#[test]
fn antigravity_public_chat_reaches_its_owner_without_browser_or_network() {
    // Match other full-ingress debug contracts, not Windows libtest's small default stack.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(public_chat());
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn public_chat() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let document=serde_json::from_value(json!({"providers":[{"id":"pool","preset":"antigravity-oauth",
        "base_url":"https://daily-cloudcode-pa.googleapis.com/v1internal", "explicit_credentials_only":true,
        "supported_models":["catalog-model"],"call_flow":{"special_adapter":"antigravity_oauth","fingerprint_adapter":"antigravity"},
        "credentials":[{"id":"synthetic","api_key":"fixture","supported_models":["catalog-model"]}]}],
        "model_routes":[{"pattern":"catalog-model","provider_ids":["pool"]}]})).unwrap();
    let fixture = support::ConsoleStateFixture::new(document, true);
    for stream in [false, true] {
        let response=Box::pin(neuro_gateway::http::router::build_router(fixture.state.clone()).oneshot(
            Request::post("/v1/chat/completions").header("authorization","Bearer public-api-contract-secret")
                .header("content-type","application/json").body(Body::from(json!({"model":"catalog-model","stream":stream,
                    "messages":[{"role":"user","content":"Synthetic; must not send Google traffic"}]}).to_string())).unwrap())).await.unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = support::parse_json(response).await;
        assert_eq!(
            body["error"]["code"], "antigravity_project_missing",
            "{body}"
        );
    }
}
