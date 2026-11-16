//! Direct and restart cache publication compare exact original Nous authority, not only the token pair.
use super::nous_fixture::*;
use crate::console::refresh_grants::*;
use serde_json::{json, Value};

#[tokio::test]
async fn nous_cache_repair_requires_context_pair_base_and_exact_expiry() {
    let f = fixture().await;
    let request = request(&f);
    let tokens = receipt();
    let result = result(&request, &tokens);
    let record = GrantRecord::sending(&request)
        .with_receipt(&GrantReceipt::NousPortal { tokens })
        .unwrap()
        .with_result(&result)
        .unwrap();
    let snapshot = f.runtime.route_config().snapshot();
    let mut payload = snapshot
        .oauth_refresh_target("oauth", "account")
        .unwrap()
        .1
        .payload
        .clone();
    let old_base = payload.base_url.clone();
    payload.api_key = result.material.access.clone();
    payload.expires_at = result.expires_at.clone();
    let raw = json!({"id":"account","kind":"account_credential","provider":"openai",
        "project_id":"internal-project","user_id":"internal-user","api_key":request.material.access,
        "api_base_url":old_base,"created_at":"2020-01-01T00:00:00Z",
        "updated_at":"2020-01-01T00:00:00Z","expires_at":EXPIRY,"quota_remaining_tokens":42,
        "account_payload":{"nous_portal":context(),"refresh_token":"nous-old-refresh","expires_at":EXPIRY}});
    let patch = |value: &Value| {
        super::super::server_cache::patch_from_grant(&value.to_string(), &record, &payload).unwrap()
    };
    let (updated, ttl) = patch(&raw).unwrap();
    assert!(ttl > 0);
    let updated: Value = serde_json::from_str(&updated).unwrap();
    assert_eq!(updated["quota_remaining_tokens"], 42);
    assert_eq!(
        updated["account_payload"]["nous_portal"],
        serde_json::to_value(&result.material.nous_portal).unwrap()
    );
    for (pointer, value) in [
        (
            "/api_base_url",
            json!("https://inference-api.nousresearch.com/v1"),
        ),
        ("/expires_at", json!("2100-01-01T00:00:00.5Z")),
        ("/account_payload/expires_at", json!("2035-01-01T00:00:00Z")),
        ("/account_payload/refresh_token", Value::Null),
        (
            "/account_payload/nous_portal/client_id",
            json!("other-client"),
        ),
        (
            "/account_payload/nous_portal/access_digest",
            json!("b".repeat(64)),
        ),
    ] {
        let mut changed = raw.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert!(patch(&changed).is_none(), "{pointer}");
    }
}

#[tokio::test]
async fn nous_direct_cache_cas_rejects_preexisting_expiry_drift_and_preserves_quotas() {
    let f = fixture().await;
    let request = request(&f);
    let result = result(&request, &receipt());
    let token = super::super::transport::TokenResponse {
        access_token: result.material.access.clone(),
        refresh_token: Some(result.material.refresh.clone()),
        expires_in: result.expires_in,
        absolute_expiry: result.expires_at.clone(),
        nous_portal: result.material.nous_portal.clone(),
        lobsterai: None,
        grok_oauth: None,
        dim_oauth: None,
        vertex_oauth: None,
        accio_sgcookie: None,
    };
    let witness = super::cache_wire::Cache::new(true).await;
    let raw = json!({"id":"exact-build","api_key":request.material.access,
        "api_base_url":"https://welcome-api.nousresearch.com/v1","expires_at":EXPIRY,"quota_remaining_tokens":42,
        "account_payload":{"nous_portal":context(),"refresh_token":"nous-old-refresh","expires_at":EXPIRY}});
    for pointer in ["/expires_at", "/account_payload/expires_at"] {
        let mut changed = raw.clone();
        *changed.pointer_mut(pointer).unwrap() = json!("2035-01-01T00:00:00Z");
        witness.replace_raw(changed.clone());
        super::super::server_cache::persist_with_nous(
            &witness.pool,
            "exact-build",
            &request.material.access,
            &request.material.refresh,
            None,
            None,
            None,
            None,
            request.material.nous_portal.as_ref(),
            None,
            &token,
            result.expires_at.as_deref().unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(witness.commits(), 0, "{pointer}");
        assert_eq!(witness.value(), changed);
    }
    witness.replace_raw(raw);
    super::super::server_cache::persist_with_nous(
        &witness.pool,
        "exact-build",
        &request.material.access,
        &request.material.refresh,
        None,
        None,
        None,
        None,
        request.material.nous_portal.as_ref(),
        None,
        &token,
        result.expires_at.as_deref().unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(witness.commits(), 1);
    assert_eq!(witness.value()["quota_remaining_tokens"], 42);
    assert_eq!(
        witness.value()["account_payload"]["nous_portal"],
        serde_json::to_value(&token.nous_portal).unwrap()
    );
    witness.stop().await;
}
