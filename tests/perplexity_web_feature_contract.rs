//! Public feature-off contract: recognition remains, optional execution cannot emit HTTP.
use neuro_gateway::{
    implementation_lines as lines, preset,
    protocol::{perplexity_web::material as product, responses},
    routing::candidate::ProviderAccountPayload,
    upstream::perplexity_web::Session,
};
use serde_json::json;

#[test]
fn perplexity_web_feature_maps_preset_profile_and_adapter_to_one_independent_line() {
    let enabled = cfg!(feature = "line-perplexity-web-reverse-api");
    assert_eq!(
        preset::get_builtin_preset(product::PRESET_ID).is_some(),
        enabled
    );
    assert_eq!(
        lines::is_protocol_profile_compiled_in("perplexity_web"),
        enabled
    );
    assert_eq!(lines::is_adapter_compiled_in(product::ADAPTER), enabled);
    assert_eq!(
        lines::required_feature_for_adapter(product::ADAPTER),
        Some("line-perplexity-web-reverse-api")
    );
}

#[cfg(not(feature = "line-perplexity-web-reverse-api"))]
#[tokio::test]
async fn perplexity_web_feature_disabled_dispatcher_refuses_without_contacting_a_loopback_trap() {
    let trap = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":product::ADAPTER,"auth_mode":product::AUTH_MODE,"base_url":product::BASE_URL,
        "api_key":"synthetic-session","headers":{"User-Agent":"synthetic-UA"},
        "session_auth":{"transport":"cookie","primaryCookieName":product::SESSION_COOKIE}
    }))
    .unwrap();
    assert!(Session::from_payload(&payload).is_ok());
    let request = responses::normalize_responses(json!({"input":"hello","model":"turbo"})).unwrap();
    let client = rquest::Client::builder()
        .no_proxy()
        .proxy(rquest::Proxy::all(format!("http://{}", trap.local_addr().unwrap())).unwrap())
        .timeout(std::time::Duration::from_millis(200))
        .build()
        .unwrap();
    let error = neuro_gateway::upstream::perplexity_web::execute(
        &client,
        std::time::Duration::from_secs(1),
        &payload,
        &request,
        "turbo",
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("gateway_provider_line_compiled_out")
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), trap.accept())
            .await
            .is_err()
    );
    let mut invalid = payload;
    invalid.base_url = "https://api.perplexity.ai".into();
    assert!(
        Session::from_payload(&invalid).is_err(),
        "Feature-off must not remove material/audience validation"
    );
}
