//! Refresh ownership stays with the existing single-flight worker; waiting only reads/rebinds.
use crate::routing::config::RouteConfigStore;
use serde_json::json;

fn store() -> RouteConfigStore {
    RouteConfigStore::from_document(serde_json::from_value(json!({"providers":[{
        "id":"fixture", "preset":"openai", "base_url":"https://example.invalid/v1",
        "supported_models":["fixture-model"],
        "credentials":[{"id":"account", "api_key":"old-fixture-token", "refresh_token":"fixture-refresh",
            "refresh_endpoint":"https://example.invalid/token", "expires_at":"2000-01-01T00:00:00Z"}]
    }]})).unwrap()).unwrap()
}

#[test]
fn expired_credential_waits_then_resolution_uses_the_current_override() {
    let store = store();
    let snapshot = store.snapshot();
    let (_, credential) = snapshot.oauth_refresh_target("fixture", "account").unwrap();
    let refresh = credential.refresh_config.as_ref().unwrap();
    assert_eq!(
        refresh.dispatch_readiness().unwrap_err().code.as_deref(),
        Some("upstream_credential_cooling")
    );
    let (generation, _) = refresh.refresh_snapshot(std::time::Duration::ZERO).unwrap();
    refresh
        .apply_refresh_if_active(
            generation,
            "new-fixture-token".into(),
            None,
            3600,
            super::super::routing::config::future_rfc3339_after_secs(3600),
        )
        .unwrap();
    refresh.dispatch_readiness().unwrap();
    let candidates = store
        .snapshot()
        .resolve_candidates_with_constraint(Some("fixture-model"), None)
        .candidates;
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].payload.api_key.as_str(), "new-fixture-token");
}

#[test]
fn permanent_refresh_error_never_turns_into_infinite_reauthorization_retries() {
    let store = store();
    let snapshot = store.snapshot();
    let (_, credential) = snapshot.oauth_refresh_target("fixture", "account").unwrap();
    let refresh = credential.refresh_config.as_ref().unwrap();
    let (generation, _) = refresh.refresh_snapshot(std::time::Duration::ZERO).unwrap();
    refresh.record_refresh_failure(generation, true);
    let error = refresh.dispatch_readiness().unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("oauth_reauthorization_required")
    );
    assert!(!super::recoverable(&error));
    assert!(refresh.refresh_snapshot(std::time::Duration::MAX).is_none());
}
