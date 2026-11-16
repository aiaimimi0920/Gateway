//! Normal declaration, flattened ingress, feature-off and final material boundaries are independent.
use super::*;
use crate::routing::{
    candidate::{deserialize_provider_payload, ProviderAccountPayload},
    config::{RouteConfigStore, RouteConfigYaml},
};
use serde_json::{json, Value};
fn context() -> Value {
    json!({"grant_type":"desktop_authorization_code","client_id":"explicit-client", "client_secret":"private-client",
        "requested_scope":CLOUD_PLATFORM_SCOPE,"scope_reported":true,
        "target":{"project":"project-a","location":"global","quota_project":"quota-a"}})
}
fn raw_payload() -> Value {
    json!({"adapter":"gemini_api_compatible","auth_mode":business::AUTH_MODE,
        "base_url":"https://aiplatform.googleapis.com/v1/projects/project-a/locations/global/publishers/google",
        "api_key":"synthetic-access","auth_header_name":"authorization", "refresh_token":"private-refresh", "vertex_oauth":context()})
}
fn document() -> Value {
    json!({"providers":[{"id":"vertex", "preset":"google-agent-platform-official-api", "auth_mode":business::AUTH_MODE,
        "base_url":raw_payload()["base_url"], "explicit_credentials_only":true,
        "credentials":[{"id":"account","api_key":"synthetic-access","refresh_token":"private-refresh","vertex_oauth":context()}]}]})
}
fn compile(value: Value) -> anyhow::Result<RouteConfigStore> {
    let document: RouteConfigYaml = serde_json::from_value(value)?;
    RouteConfigStore::from_document(document)
}

#[cfg(feature = "line-google-agent-platform-official")]
#[test]
fn vertex_oauth_typed_declaration_and_final_payload_reject_wrong_product_or_inheritance() {
    assert!(compile(document()).is_ok());
    for (pointer, replacement) in [
        ("/providers/0/preset", json!("gemini-api")),
        ("/providers/0/auth_mode", json!("bearer")),
        ("/providers/0/auth_token", json!("shadow-access")),
        ("/providers/0/base_url", json!("https://foreign.invalid")),
        ("/providers/0/adapter", json!("openai_compatible")),
        ("/providers/0/extra_body", json!({"accessToken":"embedded"})),
        ("/providers/0/credentials/0/vertex_oauth", Value::Null),
        (
            "/providers/0/credentials/0/refresh_endpoint",
            json!("https://foreign.invalid/token"),
        ),
        (
            "/providers/0/credentials/0/refresh_client_id",
            json!("foreign-client"),
        ),
        (
            "/providers/0/credentials/0/headers",
            json!({"authorization":"shadow-access"}),
        ),
    ] {
        let mut raw = document();
        // Object indexing deliberately covers both existing and additive fields.
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        raw.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.into(), replacement);
        assert!(compile(raw).is_err(), "{pointer}");
    }
    let mut inherited = document();
    inherited["providers"][0]["api_key"] = json!("parent-token");
    inherited["providers"][0]["credentials"][0]
        .as_object_mut()
        .unwrap()
        .remove("api_key");
    assert!(compile(inherited).is_err());
}

#[cfg(feature = "line-google-agent-platform-official")]
#[test]
fn vertex_oauth_hosted_and_cache_ingress_validate_context_without_forwarding_private_material() {
    let raw = raw_payload();
    let payload = deserialize_provider_payload(&raw, None).unwrap();
    assert_eq!(payload.headers["x-goog-user-project"], "quota-a");
    let serialized = serde_json::to_string(&payload).unwrap();
    assert!(!serialized.contains("private-client") && !serialized.contains("private-refresh"));
    for (key, replacement) in [
        ("auth_mode", Value::Null),
        ("refresh_endpoint", json!("https://foreign.invalid/token")),
        ("auth_token", json!("shadow")),
        ("kiro_oauth", json!({})),
        ("headers", json!({"x-goog-user-project":"different-quota"})),
    ] {
        let mut changed = raw.clone();
        changed[key] = replacement;
        assert!(
            deserialize_provider_payload(&changed, None).is_err(),
            "{key}"
        );
    }
    let mut cached: ProviderAccountPayload = serde_json::from_value(raw.clone()).unwrap();
    cached.auth_mode = None;
    crate::routing::call_flow::hydrate_cached_policy(&mut cached, Some(&raw)).unwrap();
    assert_eq!(cached.auth_mode.as_deref(), Some(business::AUTH_MODE));
    assert_eq!(cached.headers["x-goog-user-project"], "quota-a");
}

#[test]
fn vertex_oauth_does_not_reinterpret_legacy_vertex_auth_or_custom_paths() {
    let mut raw = raw_payload();
    raw.as_object_mut().unwrap().remove("auth_mode");
    raw.as_object_mut().unwrap().remove("vertex_oauth");
    raw["auth_token"] = json!("legacy-bearer-format");
    raw["responses_path"] = json!("/legacy-custom-path");
    let payload: ProviderAccountPayload = serde_json::from_value(raw).unwrap();
    assert!(!business::owns_payload(&payload));
    assert!(business::validate(&payload, "gemini_api_compatible").is_ok());
}

#[cfg(not(feature = "line-google-agent-platform-official"))]
#[test]
fn vertex_oauth_typed_business_is_rejected_when_line_is_compiled_out() {
    let payload: ProviderAccountPayload = serde_json::from_value(raw_payload()).unwrap();
    assert_eq!(
        business::validate(&payload, "gemini_api_compatible")
            .unwrap_err()
            .code
            .as_deref(),
        Some("gateway_provider_line_compiled_out")
    );
    assert!(compile(document()).is_err());
}
