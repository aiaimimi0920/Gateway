//! Matrix tests protect late merges, shortcut selection, and clear/keep behavior.
use super::{
    product::{document, payload},
    *,
};
use crate::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    protocol::{call_flow, canonical::EndpointKind, responses},
    routing::{
        candidate::ProviderAccountPayload,
        config::{RouteConfigStore, RouteConfigYaml},
    },
    upstream::client::UpstreamClient,
};

#[test]
fn late_payload_mutations_are_rejected_without_a_flow_block() {
    let original = serde_json::to_value(payload()).unwrap();
    for (field, value) in [
        ("adapter", json!("custom_http")),
        ("auth_mode", json!("bearer")),
        ("auth_header_name", json!("Cookie")),
        ("auth_token", json!("foreign")),
        ("session_auth", json!({"transport":"cookie"})),
        ("execution_mode", json!("browser_backed")),
        ("request_client_profile", json!("claude_cli")),
        ("responses_path", json!("/responses/compact")),
        ("messages_path", json!("/messages")),
        ("headers", json!({"HOST":"evil.invalid"})),
        ("extra_body", json!({"refresh_token":"private"})),
    ] {
        let mut changed = original.clone();
        changed[field] = value;
        let changed: ProviderAccountPayload = serde_json::from_value(changed).unwrap();
        assert!(call_flow::prepare(&changed).is_err(), "must reject {field}");
    }
    for base in [
        "http://cli-chat-proxy.grok.com/v1",
        "https://cli-chat-proxy.grok.com/v1?token=x",
        "https://u:p@cli-chat-proxy.grok.com/v1",
        "https://cli-chat-proxy.grok.com/v1#fragment",
        "https://cli-chat-proxy.grok.com.evil.invalid/v1",
    ] {
        let mut changed = payload();
        changed.base_url = base.into();
        assert!(call_flow::prepare(&changed).is_err());
    }
}

#[test]
fn only_text_endpoints_reach_the_responses_codec() {
    let payload = payload();
    let mut req = responses::normalize_responses(json!({"input":"hello"})).unwrap();
    for kind in [
        EndpointKind::Embeddings,
        EndpointKind::AudioSpeech,
        EndpointKind::ImagesGenerations,
        EndpointKind::Search,
        EndpointKind::ResearchCreate,
    ] {
        req.endpoint_kind = kind;
        assert!(
            UpstreamClient::build_request_plan(&payload, &req, "grok-contract", false).is_err()
        );
    }
    for stream in [false, true] {
        req.endpoint_kind = EndpointKind::Responses;
        req.stream = stream;
        assert!(!UpstreamClient::uses_native_codex(
            &payload,
            &req,
            "grok-contract"
        ));
        assert!(
            UpstreamClient::build_request_plan(&payload, &req, "grok-contract", stream).is_ok()
        );
    }
}

#[test]
fn final_plan_rejects_all_non_responses_mutations_and_auth_body_defaults() {
    let payload = payload();
    let req = responses::normalize_responses(json!({"input":"hello"})).unwrap();
    for case in 0..7 {
        let mut plan =
            UpstreamClient::build_request_plan(&payload, &req, "grok-contract", true).unwrap();
        match case {
            0 => plan.method = rquest::Method::GET,
            1 => plan.query.push(("token".into(), "private".into())),
            2 => plan.response_kind = EndpointKind::ChatCompletions,
            3 => plan.body = None,
            4 => plan.body.as_mut().unwrap()["stream"] = json!("true"),
            5 => plan.body.as_mut().unwrap()["input"] = Value::Null,
            _ => plan.body.as_mut().unwrap()["messages"] = json!([]),
        }
        assert!(business::validate_plan(&payload, &plan).is_err());
    }
    for key in [
        "access_token",
        "refresh_token",
        "client_id",
        "scope",
        "principal_id",
        "messages",
    ] {
        let mut payload = payload.clone();
        payload.extra_body = Some([(key.into(), json!("private"))].into());
        assert!(UpstreamClient::build_request_plan(&payload, &req, "grok-contract", true).is_err());
    }
}

#[test]
fn missing_material_is_storable_but_never_sendable_and_profiles_are_distinct() {
    let mut value = document();
    value["providers"][0]["credentials"][0]["api_key"] = json!("");
    let cfg: RouteConfigYaml = serde_json::from_value(value).unwrap();
    let store = RouteConfigStore::from_document(cfg.clone()).unwrap();
    assert_eq!(
        store.snapshot().get_providers()[0].protocol_profile,
        business::PROFILE
    );
    assert_eq!(
        store.snapshot().get_providers()[0].protocol_family,
        crate::protocol::registry::OPENAI_RESPONSES_FAMILY
    );
    let mut empty = payload();
    empty.api_key.clear();
    let req = responses::normalize_responses(json!({"input":"hello"})).unwrap();
    assert!(UpstreamClient::build_request_plan(&empty, &req, "grok-contract", false).is_err());
    for token in ["bad\ntoken", "not a token"] {
        let mut changed = cfg.clone();
        changed.providers[0].credentials[0].api_key = Some(token.into());
        assert!(RouteConfigStore::from_document(changed).is_err());
    }
}

#[test]
fn keep_rejects_disabled_consumer_retarget_but_allows_stable_id_reordering() {
    let mut value = document();
    value["providers"][0]["credentials"][0]["enabled"] = json!(false);
    let cfg: RouteConfigYaml = serde_json::from_value(value).unwrap();
    let redacted = redact_route_document(&cfg).unwrap();
    let patches: Vec<_> = redacted
        .secrets
        .iter()
        .map(|s| SecretPatch::keep(&s.path))
        .collect();
    let mut changed = redacted.document.clone();
    changed.providers[0].credentials[0].base_url = Some("https://api.x.ai/v1".into());
    assert!(resolve_secret_patches(&cfg, changed, &patches).is_err());
    assert!(resolve_secret_patches(&cfg, redacted.document, &patches).is_ok());
}
