//! Probe bindings reuse candidate identity, including discovery and runtime OAuth refresh.
use super::*;
use serde_json::json;

fn store(discovery: bool, oauth: bool) -> RouteConfigStore {
    let url = "https://example.invalid/v1";
    let mut credential = json!({"id":"one", "api_key":"synthetic-one"});
    if oauth {
        credential["refresh_token"] = json!("synthetic-refresh");
        credential["refresh_endpoint"] = json!("https://example.invalid/token");
    }
    if discovery {
        credential["discovery"] = json!({"source_url":url, "api_base":url,
            "protocol":"chat_completions", "models":["upstream"], "verified_models":["upstream"],
            "checked_at":"2026-10-20T00:00:00Z", "binding":crate::provider_discovery::binding(url, "synthetic-one")});
    }
    let document = serde_json::from_value(json!({"providers":[{"id":"pool", "adapter":"openai_compatible",
        "protocol_profile":"openai_responses", "base_url":url, "credentials":[credential],
        "supported_models":["alias"], "model_map":{"alias":"upstream"}}], "model_routes":[], "aliases":{}})).unwrap();
    RouteConfigStore::from_document(document).unwrap()
}

#[test]
fn discovered_probe_profile_is_the_normal_candidate_profile_not_provider_profile() {
    let store = store(true, false);
    let route = store.snapshot();
    let target = route.select_credential_probe_target("one").unwrap();
    let identity = route.model_probe_identity(&target, "upstream").unwrap();
    let candidate = route.resolve_candidates(Some("alias")).remove(0);
    assert_eq!(candidate.protocol_profile, "openai_compatible_generic");
    assert_eq!(
        identity.protocol_profile.as_deref(),
        Some(candidate.protocol_profile.as_str())
    );
    assert_eq!(
        identity.provider_credential_ref,
        candidate.payload.credential_id
    );
    assert_eq!(identity.model, candidate.upstream_model.unwrap());
}

#[test]
fn same_revision_oauth_refresh_invalidates_the_old_probe_payload_binding() {
    let store = store(false, true);
    let route = store.snapshot();
    let target = route.select_credential_probe_target("one").unwrap();
    assert!(route.model_probe_identity(&target, "upstream").is_some());
    let refresh = route.compiled.providers[0].credential_pool[0]
        .refresh_config
        .as_ref()
        .unwrap();
    assert!(refresh
        .apply_refresh_if_active(
            0,
            "synthetic-new-access".into(),
            None,
            3600,
            "2026-10-20T01:00:00Z".into()
        )
        .is_some());
    assert_eq!(route.revision().id(), store.snapshot().revision().id());
    assert!(store
        .snapshot()
        .model_probe_identity(&target, "upstream")
        .is_none());
    let current = route.select_credential_probe_target("one").unwrap();
    assert!(route.model_probe_identity(&current, "upstream").is_some());
}
