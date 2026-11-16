use super::*;
use crate::{
    routing::{call_flow::prepare_route_payload, candidate::ProviderAccountPayload},
    upstream::client::UpstreamClient,
};
use serde_json::json;

fn fixture() -> (ProviderAccountPayload, serde_json::Value) {
    let context: AuthorizationContext = serde_json::from_value(test_support::context()).unwrap();
    let models = parse_catalog(
        &serde_json::to_vec(&json!({"data":[
            {"id":"c","dim":{"endpoint_type":"openai"}},
            {"id":"r","dim":{"endpoint_type":"openai-response"}},
            {"id":"a","dim":{"endpoint_type":"anthropic"}},
            {"id":"g","dim":{"endpoint_type":"gemini"}}
        ]}))
        .unwrap(),
    )
    .unwrap();
    let catalog = ModelCatalog::new(&context, models).unwrap();
    let raw = json!({"adapter":"openai_compatible","base_url":"https://dimagent.cn/v1",
        "api_key":"synthetic-access","auth_mode":"dim_user_oauth","dim_oauth":context,"dim_catalog":catalog});
    (serde_json::from_value(raw.clone()).unwrap(), raw)
}

#[test]
fn dim_cached_and_db_projection_keep_exact_wire_and_health_profile() {
    let (payload, raw) = fixture();
    for (model, family) in [
        ("c", "openai_chat"),
        ("r", "openai_responses"),
        ("a", "anthropic_messages"),
        ("g", "gemini_generate_content"),
    ] {
        let projected = prepare_route_payload(
            payload.clone(),
            Some(&raw),
            "openai",
            Some(model),
            Some(model),
        )
        .unwrap()
        .unwrap();
        assert_eq!(projected.protocol_family, family);
        assert_eq!(projected.supported_protocol_families, [family]);
        assert_eq!(
            projected.protocol_profile.as_deref(),
            Some(runtime::protocol_profile(family))
        );
    }
    assert!(prepare_route_payload(payload.clone(), None, "openai", Some("c"), Some("c")).is_err());
    let mut missing = raw.clone();
    missing.as_object_mut().unwrap().remove("dim_catalog");
    assert!(prepare_route_payload(
        payload.clone(),
        Some(&missing),
        "openai",
        Some("c"),
        Some("c")
    )
    .is_err());
    let mut foreign = raw.clone();
    foreign["dim_catalog"]["subject"] = json!("other-subject");
    assert!(
        prepare_route_payload(payload, Some(&foreign), "openai", Some("c"), Some("c")).is_err()
    );
}

#[test]
fn dim_final_plan_rejects_redirect_targets_key_query_media_and_configured_header_overrides() {
    let (payload, raw) = fixture();
    let request = crate::protocol::openai::normalize_chat_completions(
        json!({"model":"c","messages":[{"role":"user","content":"hi"}]}),
    )
    .unwrap();
    for model in ["c", "r", "a", "g"] {
        let projected = prepare_route_payload(
            payload.clone(),
            Some(&raw),
            "openai",
            Some(model),
            Some(model),
        )
        .unwrap()
        .unwrap();
        let plan =
            UpstreamClient::build_request_plan(&projected.payload, &request, model, true).unwrap();
        let mut redirected = crate::upstream::common::RequestPlan {
            url: "https://evil.invalid/v1/chat/completions".into(),
            method: plan.method.clone(),
            query: plan.query.clone(),
            body: plan.body.clone(),
            response_kind: plan.response_kind,
        };
        assert!(runtime::validate_plan(&projected.payload, &redirected).is_err());
        redirected.url = plan.url.clone();
        redirected
            .query
            .push(("key".into(), "synthetic-access".into()));
        assert!(runtime::validate_plan(&projected.payload, &redirected).is_err());
        redirected.query.clear();
        redirected.url = "https://dimagent.cn/v1/embeddings".into();
        assert!(runtime::validate_plan(&projected.payload, &redirected).is_err());
        let mut override_headers = projected.payload.clone();
        override_headers
            .headers
            .insert("authorization".into(), "Bearer other".into());
        assert!(
            runtime::validate(&override_headers, override_headers.canonical_adapter()).is_err()
        );
    }
}
