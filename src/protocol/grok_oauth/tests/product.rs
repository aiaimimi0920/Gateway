//! Product identity must survive configuration, flow preparation and final plan mutations.
use super::*;
use crate::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    protocol::{call_flow, responses},
    routing::{
        candidate::ProviderAccountPayload,
        config::{RouteConfigStore, RouteConfigYaml},
    },
    upstream::client::UpstreamClient,
};

pub(super) fn document() -> Value {
    json!({"providers":[{"id":"build-pool","preset":business::PRESET_ID,
        "base_url":BUSINESS_BASE_URL,"explicit_credentials_only":true,
        "supported_models":["grok-contract"],
        "credentials":[{"id":"exact-build","api_key":"synthetic-build-token"}]}]})
}
pub(super) fn payload() -> ProviderAccountPayload {
    let store =
        RouteConfigStore::from_document(serde_json::from_value(document()).unwrap()).unwrap();
    store.snapshot().resolve_candidates(Some("grok-contract"))[0]
        .payload
        .clone()
}

#[test]
fn independent_preset_compiles_without_borrowing_api_or_web_identity() {
    let payload = payload();
    assert_eq!(payload.auth_mode.as_deref(), Some(business::AUTH_MODE));
    assert_eq!(payload.responses_path.as_deref(), Some("/responses"));
    assert!(payload.chat_completions_path.is_none());
    assert!(call_flow::prepare(&payload).is_ok());
    let mut empty = document();
    empty["providers"][0]["credentials"] = json!([]);
    let store = RouteConfigStore::from_document(serde_json::from_value(empty).unwrap()).unwrap();
    assert!(store
        .snapshot()
        .resolve_candidates(Some("grok-contract"))
        .is_empty());
}

#[test]
fn preset_anchor_rejects_destination_profile_path_and_disabled_credential_overrides() {
    for (pointer, value) in [
        ("/providers/0/base_url", json!("https://api.x.ai/v1")),
        ("/providers/0/auth_mode", json!("bearer")),
        ("/providers/0/protocol_profile", json!("xai")),
        ("/providers/0/adapter", json!("grok_compatible")),
        (
            "/providers/0/chat_completions_path",
            json!("/chat/completions"),
        ),
        ("/providers/0/request_client_profile", json!("codex_cli")),
        ("/providers/0/headers", json!({"Cookie":"private-web"})),
        (
            "/providers/0/call_flow",
            json!({"call_adapter":"anthropic"}),
        ),
        (
            "/providers/0/credentials/0/base_url",
            json!("https://grok.com"),
        ),
        (
            "/providers/0/credentials/0/refresh_endpoint",
            json!("https://evil.invalid/token"),
        ),
    ] {
        let mut value_document = document();
        value_document["providers"][0]["credentials"][0]["enabled"] = json!(false);
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        value_document
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.into(), value);
        let result =
            RouteConfigStore::from_document(serde_json::from_value(value_document).unwrap());
        assert!(result.is_err(), "must reject {pointer}");
    }
}

#[test]
fn marker_rejects_late_retargeting_and_foreign_fingerprints() {
    let original = payload();
    let mut changed = original.clone();
    changed.base_url = "https://api.x.ai/v1".into();
    assert!(call_flow::prepare(&changed).is_err());
    changed = original;
    changed.call_flow =
        Some(serde_json::from_value(json!({"fingerprint_adapter":"codex_cli"})).unwrap());
    assert!(call_flow::prepare(&changed).is_err());
}

#[test]
fn public_chat_and_responses_use_the_same_fixed_responses_wire() {
    let payload = payload();
    for req in [
        crate::protocol::openai::normalize_chat_completions(
            json!({"model":"grok-contract","messages":[{"role":"user","content":"hello"}]}),
        )
        .unwrap(),
        responses::normalize_responses(json!({"model":"grok-contract","input":"hello"})).unwrap(),
    ] {
        let plan =
            UpstreamClient::build_request_plan(&payload, &req, "grok-contract", true).unwrap();
        assert_eq!(plan.url, format!("{BUSINESS_BASE_URL}/responses"));
        assert_eq!(
            plan.response_kind,
            crate::protocol::canonical::EndpointKind::Responses
        );
        assert!(plan.body.as_ref().unwrap()["input"].is_array());
        assert!(plan.body.as_ref().unwrap().get("messages").is_none());
        assert!(!UpstreamClient::uses_native_codex(
            &payload,
            &req,
            "grok-contract"
        ));
    }
}

#[test]
fn final_plan_revalidation_rejects_method_target_and_kind_mutations() {
    let payload = payload();
    let req = responses::normalize_responses(json!({"input":"hello"})).unwrap();
    let mut plan =
        UpstreamClient::build_request_plan(&payload, &req, "grok-contract", true).unwrap();
    plan.url = "https://api.x.ai/v1/responses".into();
    assert!(business::validate_plan(&payload, &plan).is_err());
}

#[test]
fn keeping_a_build_token_cannot_change_its_product_or_destination() {
    let active: RouteConfigYaml = serde_json::from_value(document()).unwrap();
    let redacted = redact_route_document(&active).unwrap();
    let patches: Vec<_> = redacted
        .secrets
        .iter()
        .map(|s| SecretPatch::keep(&s.path))
        .collect();
    let mut draft = redacted.document.clone();
    draft.providers[0].preset = Some("xai-api".into());
    draft.providers[0].base_url = "https://api.x.ai/v1".into();
    assert!(resolve_secret_patches(&active, draft, &patches).is_err());
    let mut metadata = redacted.document;
    metadata.providers[0].label = Some("Renamed pool".into());
    assert!(resolve_secret_patches(&active, metadata, &patches).is_ok());
}
