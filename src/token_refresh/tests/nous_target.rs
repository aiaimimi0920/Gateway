//! Fixed-target rotation changes only the authorized credential and keeps all dispatch material coherent.
use super::{nous_fixture::*, nous_scheduler_fixture::*, *};
use crate::{console::refresh_grants::*, protocol::nous_portal::*};
use serde_json::{json, Value};

pub(super) fn subscription_reply() -> String {
    let mut value: Value = serde_json::from_str(&reply()).unwrap();
    value["inference_base_url"] = json!(InferenceTarget::Subscription.base_url());
    value.to_string()
}
#[tokio::test]
async fn nous_target_rotation_updates_runtime_projection_and_restart_without_changing_pool_default()
{
    let mut f = fixture().await;
    let snapshot = f.runtime.route_config().snapshot();
    let mut doc = snapshot.document().clone();
    let mut other = doc.providers[0].credentials[0].clone();
    other.id = Some("unaffected".into());
    other.enabled = Some(false);
    doc.providers[0].credentials.push(other);
    f.runtime
        .commit_document(snapshot.revision().id(), doc, None)
        .await
        .unwrap();
    let original = request(&f);
    let pinned = f.runtime.route_config().snapshot();
    let old_candidate = pinned.resolve_candidates(Some("test-model"))[0].clone();
    let owner = pinned
        .oauth_refresh_target("oauth", "account")
        .unwrap()
        .1
        .refresh_config
        .clone()
        .unwrap();
    let mock = Mock::new(vec![(200, subscription_reply())], 0).await;
    force_due(&f);
    refresh(&f, &mock, &mut Pending::new()).await;
    let current = f.runtime.route_config().snapshot();
    let fresh = current.resolve_candidates(Some("test-model"))[0].clone();
    assert_eq!(
        fresh.payload.base_url,
        InferenceTarget::Subscription.base_url()
    );
    assert_eq!(
        fresh.payload.api_key,
        access("synthetic-subject", 4102444900)
    );
    assert_eq!(
        fresh.payload.expires_at.as_deref(),
        Some("2100-01-01T00:01:40Z")
    );
    assert_eq!(
        old_candidate.payload.base_url,
        InferenceTarget::Welcome.base_url()
    );
    assert_eq!(old_candidate.payload.api_key, original.material.access);
    let from_old_snapshot = pinned.resolve_candidates(Some("test-model"))[0].clone();
    assert_eq!(from_old_snapshot.payload.base_url, fresh.payload.base_url);
    assert_eq!(from_old_snapshot.payload.api_key, fresh.payload.api_key);
    assert_eq!(
        from_old_snapshot.payload.expires_at,
        fresh.payload.expires_at
    );
    assert!(Arc::ptr_eq(
        &owner,
        current
            .oauth_refresh_target("oauth", "account")
            .unwrap()
            .1
            .refresh_config
            .as_ref()
            .unwrap()
    ));
    assert_eq!(
        current.document().providers[0].base_url,
        InferenceTarget::Welcome.base_url()
    );
    assert!(current.document().providers[0].credentials[1]
        .base_url
        .is_none());
    assert_eq!(load(&f, &original).phase(), GrantPhase::Published);
    f.runtime = RouteConfigRuntime::new_local(&f.config).unwrap();
    f.runtime.recover_startup().await.unwrap();
    let restarted = f
        .runtime
        .route_config()
        .resolve_candidates(Some("test-model"));
    assert_eq!(restarted[0].payload.base_url, fresh.payload.base_url);
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}

#[tokio::test]
async fn nous_cache_recovery_compares_old_target_and_publishes_the_exact_new_target() {
    let f = fixture().await;
    let request = request(&f);
    let tokens = ReceivedTokens::from_json(subscription_reply().as_bytes()).unwrap();
    let result = result(&request, &tokens);
    let record = GrantRecord::sending(&request)
        .with_receipt(&GrantReceipt::NousPortal { tokens })
        .unwrap()
        .with_result(&result)
        .unwrap();
    let snapshot = f.runtime.route_config().snapshot();
    let mut target = snapshot
        .oauth_refresh_target("oauth", "account")
        .unwrap()
        .1
        .payload
        .clone();
    target.api_key = result.material.access.clone();
    target.expires_at = result.expires_at.clone();
    target.base_url = InferenceTarget::Subscription.base_url().into();
    let raw = json!({"id":"account","kind":"account_credential","provider":"openai",
        "project_id":"internal-project","user_id":"internal-user","api_key":request.material.access,
        "api_base_url":InferenceTarget::Welcome.base_url(),"created_at":"2020-01-01T00:00:00Z",
        "updated_at":"2020-01-01T00:00:00Z","expires_at":EXPIRY,"quota_remaining_tokens":42,
        "account_payload":{"base_url":InferenceTarget::Welcome.base_url(),"nous_portal":context(),
        "refresh_token":"nous-old-refresh","expires_at":EXPIRY}});
    let patch = |raw: &Value, target: &crate::routing::candidate::ProviderAccountPayload| {
        super::super::server_cache::patch_from_grant(&raw.to_string(), &record, target).unwrap()
    };
    let (updated, _) = patch(&raw, &target).unwrap();
    let updated: Value = serde_json::from_str(&updated).unwrap();
    assert_eq!(updated["api_base_url"], target.base_url);
    assert_eq!(updated["account_payload"]["base_url"], target.base_url);
    assert_eq!(updated["quota_remaining_tokens"], 42);
    let mut wrong_target = target.clone();
    wrong_target.base_url = InferenceTarget::Welcome.base_url().into();
    assert!(patch(&raw, &wrong_target).is_none());
    for pointer in ["/api_base_url", "/account_payload/base_url"] {
        let mut wrong = raw.clone();
        *wrong.pointer_mut(pointer).unwrap() = json!(InferenceTarget::Subscription.base_url());
        assert!(patch(&wrong, &target).is_none(), "{pointer}");
    }
}
