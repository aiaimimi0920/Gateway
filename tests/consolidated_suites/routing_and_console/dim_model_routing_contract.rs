//! A model directory is declaration evidence only; its exact wire family must reach the sender.
use neuro_gateway::{routing::config::RouteConfigStore, upstream::client::UpstreamClient};
use serde_json::json;

fn document() -> serde_json::Value {
    let models: Vec<_> = ["chat", "responses", "messages", "gemini"].into_iter().map(|protocol| json!({
        "id":format!("opaque-{protocol}"),"name":"Opaque model","protocol":protocol,
        "context_window":null,"max_output_tokens":null,"tools":true,"reasoning":null,"reasoning_efforts":[],
        "input_modalities":["text"],"output_modalities":["text"],"service_tiers":[],"deprecated":false,"zdr":false
    })).collect();
    json!({"providers":[{"id":"dim","preset":"dim-cn-oauth","auth_mode":"dim_user_oauth","explicit_credentials_only":true,
        "base_url":"https://dimagent.cn/v1","credentials":[{"id":"account","api_key":"synthetic-access","refresh_token":"synthetic-refresh",
        "supported_models":["opaque-chat","opaque-responses","opaque-messages","opaque-gemini"],
        "dim_oauth":{"site":"china","client_id":"permitted-gateway","requested_scopes":["openid"],
        "token_endpoint":"https://dimagent.cn/oauth/token","userinfo_endpoint":"https://dimagent.cn/oauth/userinfo","jwks_uri":"https://dimagent.cn/oauth/jwks.json",
        "subject":"synthetic-subject","user_id":"synthetic-user","assurance":"user_info_endpoint","nonce":"synthetic-nonce"},
        "dim_catalog":{"site":"china","client_id":"permitted-gateway","subject":"synthetic-subject","models":models}}]}]})
}
#[test]
fn dim_models_select_declared_wires_and_fixed_bearer_targets_without_verification_claims() {
    let store =
        RouteConfigStore::from_document(serde_json::from_value(document()).unwrap()).unwrap();
    for (protocol, adapter, family, path) in [
        (
            "chat",
            "openai_compatible",
            "openai_chat",
            "/v1/chat/completions",
        ),
        (
            "responses",
            "openai_compatible",
            "openai_responses",
            "/v1/responses",
        ),
        (
            "messages",
            "anthropic_compatible",
            "anthropic_messages",
            "/v1/messages",
        ),
        (
            "gemini",
            "gemini_api_compatible",
            "gemini_generate_content",
            "/v1beta/models/opaque-gemini:streamGenerateContent",
        ),
    ] {
        let model = format!("opaque-{protocol}");
        let candidates = store.snapshot().resolve_candidates(Some(&model));
        assert_eq!(candidates.len(), 1);
        let candidate = &candidates[0];
        assert_eq!(candidate.adapter, adapter);
        assert_eq!(candidate.protocol_family, family);
        let request = neuro_gateway::protocol::openai::normalize_chat_completions(
            json!({"model":model,"stream":true,
            "messages":[{"role":"user","content":"hello"}]}),
        )
        .unwrap();
        let plan =
            UpstreamClient::build_request_plan(&candidate.payload, &request, &model, true).unwrap();
        assert_eq!(plan.url, format!("https://dimagent.cn{path}"));
        assert!(plan.query.iter().all(|(key, _)| key != "key"));
        let headers = neuro_gateway::upstream::headers::build_upstream_headers(&candidate.payload);
        assert_eq!(headers["authorization"], "Bearer synthetic-access");
        assert!(!headers.contains_key("x-api-key"));
        assert!(!headers.contains_key("x-goog-api-key"));
    }
    assert!(store
        .snapshot()
        .resolve_candidates(Some("not-declared"))
        .is_empty());
    let credential = &store.get_providers()[0].credential_pool[0];
    assert!(credential.discovery.is_none());
    assert!(credential.payload.discovered_protocols.is_empty());
}
