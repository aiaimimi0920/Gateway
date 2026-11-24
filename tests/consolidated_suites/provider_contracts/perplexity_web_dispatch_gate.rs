//! A dedicated Web executor must never opt into generic custom_http/raw passthrough.
use neuro_gateway::{
    error::FallbackHint,
    protocol::{perplexity_web::material as product, responses},
    routing::candidate::ProviderAccountPayload,
    upstream::client::UpstreamClient,
};
use serde_json::json;

#[test]
fn perplexity_web_dedicated_dispatcher_cannot_use_generic_custom_http() {
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":product::ADAPTER,"auth_mode":product::AUTH_MODE,"base_url":product::BASE_URL,
        "api_key":"synthetic-session", "headers":{"User-Agent":"synthetic-agent"},
        "session_auth":{"transport":"cookie","primaryCookieName":product::SESSION_COOKIE}
    }))
    .unwrap();
    for stream in [false, true] {
        let request = responses::normalize_responses(
            json!({"model":"turbo","input":"hello","stream":stream}),
        )
        .unwrap();
        let error = UpstreamClient::build_request_plan(&payload, &request, "turbo", stream)
            .expect_err("Web adapter must not forward raw JSON or a token as custom_http");
        assert_eq!(
            error.code.as_deref(),
            Some("perplexity_web_request_plan_unsupported")
        );
        assert!(!error.retryable);
        assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
    }
}
