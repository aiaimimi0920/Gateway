//! Local public-ingress contracts only; never authenticates or sends Google traffic.
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use neuro_gateway::{
    preset::get_builtin_preset,
    protocol::canonical::EndpointKind,
    routing::candidate::{ProviderAccountPayload, ProviderExecutionMode},
};
use serde_json::json;
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;

#[test]
fn stitch_web_has_an_independent_native_http_preset_not_gemini_or_flow() {
    let preset = get_builtin_preset("stitch-web").unwrap();
    assert_eq!(preset.adapter, "stitch_web_compatible");
    assert_eq!(
        preset.execution_mode,
        Some(ProviderExecutionMode::DirectHttp)
    );
    assert!(preset.default_model.is_none());
    let payload:ProviderAccountPayload=serde_json::from_value(json!({"adapter":"stitch_web_compatible","base_url":"https://stitch.withgoogle.com",
        "api_key":"","execution_mode":"browser_backed","endpoint_execution_modes":{"chat_completions":"browser_backed"}})).unwrap();
    assert_eq!(
        payload.resolve_execution_mode(EndpointKind::ChatCompletions),
        ProviderExecutionMode::DirectHttp
    );
}

#[tokio::test]
async fn stitch_web_public_chat_ingress_reaches_its_owner_and_never_falls_back_to_a_browser() {
    let document=serde_json::from_value(json!({"providers":[{"id":"stitch-pool","preset":"stitch-web","base_url":"https://stitch.withgoogle.com",
        "explicit_credentials_only":true,"supported_models":["stitch-design"],"call_flow":{"special_adapter":"stitch_web","fingerprint_adapter":"session"},
        "credentials":[{"id":"fixture"}]}],"model_routes":[{"pattern":"stitch-design","provider_ids":["stitch-pool"]}],"aliases":{}})).unwrap();
    let fixture = support::ConsoleStateFixture::new(document, true);
    let response=neuro_gateway::http::router::build_router(fixture.state.clone()).oneshot(Request::post("/v1/chat/completions")
        .header("authorization","Bearer public-api-contract-secret").header("content-type","application/json")
        .body(Body::from(json!({"model":"stitch-design","messages":[{"role":"user","content":"Synthetic UI design; no credentials, no network"}]}).to_string())).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = support::parse_json(response).await;
    assert_eq!(
        body["error"]["code"], "stitch_web_session_missing",
        "{body}"
    );
}
