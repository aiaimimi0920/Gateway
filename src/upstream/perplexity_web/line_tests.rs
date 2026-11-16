//! Feature identity is independent of the official Sonar/Search lines and mutable provider IDs.
use crate::{implementation_lines as lines, preset, protocol::registry};

#[test]
fn perplexity_web_line_identity_and_registration_match_the_binary_feature() {
    let line = lines::RefactoredImplementationLine::PerplexityWebReverseApi;
    assert_eq!(line.canonical_protocol_profile(), "perplexity_web");
    assert_eq!(line.feature_name(), "line-perplexity-web-reverse-api");
    assert_eq!(
        lines::line_for_adapter("perplexity_web_compatible"),
        Some(line)
    );
    assert_eq!(
        lines::line_for_protocol_profile("perplexity-web"),
        Some(line)
    );
    assert_eq!(
        lines::is_adapter_compiled_in("perplexity_web_compatible"),
        cfg!(feature = "line-perplexity-web-reverse-api")
    );
    assert_eq!(
        preset::get_builtin_preset("perplexity-web").is_some(),
        cfg!(feature = "line-perplexity-web-reverse-api")
    );
    assert_eq!(
        registry::default_protocol_profile_for_preset("perplexity-web"),
        "perplexity_web"
    );
    assert_eq!(
        registry::default_protocol_profile_for_adapter("perplexity_web_compatible"),
        "perplexity_web"
    );
    assert_eq!(
        registry::default_protocol_profile_for_preset("perplexity-api"),
        "perplexity_chat"
    );
    assert_eq!(
        registry::default_protocol_profile_for_preset("perplexity-search"),
        "perplexity_search"
    );
}

#[cfg(not(feature = "line-perplexity-web-reverse-api"))]
#[tokio::test]
async fn perplexity_web_line_disabled_execution_retains_material_checks_but_has_no_http_transport()
{
    use crate::{
        protocol::{perplexity_web::material as product, responses},
        routing::candidate::ProviderAccountPayload,
    };
    let payload: ProviderAccountPayload = serde_json::from_value(serde_json::json!({
        "adapter":product::ADAPTER,"auth_mode":product::AUTH_MODE,"base_url":product::BASE_URL,
        "api_key":"synthetic-session","headers":{"User-Agent":"synthetic-UA"},
        "session_auth":{"transport":"cookie","primaryCookieName":product::SESSION_COOKIE}
    }))
    .unwrap();
    assert!(super::Session::from_payload(&payload).is_ok());
    let request =
        responses::normalize_responses(serde_json::json!({"input":"hello","model":"turbo"}))
            .unwrap();
    let error = super::execute(
        &rquest::Client::new(),
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
}
