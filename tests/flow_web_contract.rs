//! Synthetic contracts only; no login, Google requests or generation credits.
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use neuro_gateway::preset::get_builtin_preset;
use neuro_gateway::protocol::canonical::EndpointKind;
use neuro_gateway::routing::candidate::{ProviderAccountPayload, ProviderExecutionMode};
use serde_json::json;
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;

#[test]
fn flow_web_has_an_independent_http_only_preset_without_claimed_models() {
    let preset = get_builtin_preset("flow-web").expect("Google Flow must have its own preset");
    assert_eq!(preset.adapter, "flow_web_compatible");
    assert_eq!(
        preset.execution_mode,
        Some(ProviderExecutionMode::DirectHttp)
    );
    assert!(preset.default_model.is_none());
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":"flow_web_compatible", "base_url":"https://flow.google.com", "api_key":"",
        "execution_mode":"browser_backed", "endpoint_execution_modes":{"images_generations":"browser_backed"}
    })).unwrap();
    for endpoint in [
        EndpointKind::ImagesGenerations,
        EndpointKind::VideosGenerations,
    ] {
        assert_eq!(
            payload.resolve_execution_mode(endpoint),
            ProviderExecutionMode::DirectHttp
        );
    }
}

#[tokio::test]
async fn flow_web_public_media_ingress_never_submits_without_fresh_http_proof() {
    let document = serde_json::from_value(json!({"providers":[{
        "id":"flow-web","preset":"flow-web","base_url":"https://flow.google.com",
        "explicit_credentials_only":true,"supported_models":["unverified-flow-model"],
        "call_flow":{"special_adapter":"flow_web","fingerprint_adapter":"session"},
        "credentials":[{"id":"fixture","runtime_state_object_key":"credential-runtime/flow-web/fixture/http-cookies.json"}]
    }],"model_routes":[{"pattern":"unverified-flow-model","provider_ids":["flow-web"]}],"aliases":{}})).unwrap();
    let fixture = support::ConsoleStateFixture::new(document, true);
    for path in ["/v1/images/generations", "/v1/videos/generations"] {
        let response = neuro_gateway::http::router::build_router(fixture.state.clone()).oneshot(
            Request::post(path).header("authorization", "Bearer public-api-contract-secret")
                .header("content-type", "application/json").body(Body::from(json!({
                    "model":"unverified-flow-model","prompt":"Synthetic only; must not reach Google"
                }).to_string())).unwrap()
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = support::parse_json(response).await;
        assert_eq!(
            body["error"]["code"], "flow_web_fresh_proof_unavailable",
            "{body}"
        );
        assert!(body.get("data").is_none());
    }
}
