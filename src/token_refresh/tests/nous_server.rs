//! Synthetic Server authority exercises receipt ACK loss and the real periodic task's shutdown flush.
use super::{
    authority_fixture::{Authority, Stall},
    nous_fixture::*,
    nous_scheduler_fixture::*,
    *,
};
use crate::{console::refresh_grants::*, protocol::nous_portal::*};
use serde_json::json;

#[tokio::test]
async fn nous_worker_shutdown_flush_preserves_unacknowledged_receipt_then_publishes_once() {
    let f = fixture().await;
    let mut doc = f.runtime.route_config().snapshot().document().clone();
    doc.providers[0].id = "build-pool".into();
    doc.providers[0].credentials[0].id = Some("exact-build".into());
    doc.model_routes[0].provider_ids = vec!["build-pool".into()];
    let authority = Authority::with_document(false, doc);
    let snapshot = authority.runtime.route_config().snapshot();
    let revision = crate::console::RouteConfigRedisRevision::new(
        snapshot.revision().clone(),
        snapshot.document().clone(),
    )
    .unwrap();
    let original = GrantRequest::from_revision(&revision, "build-pool", "exact-build").unwrap();
    snapshot
        .oauth_refresh_target("build-pool", "exact-build")
        .unwrap()
        .1
        .refresh_config
        .as_ref()
        .unwrap()
        .force_due();
    let cache = super::cache_wire::Cache::new(true).await;
    cache.replace_raw(json!({"id":"exact-build","kind":"account_credential","provider":"openai",
        "project_id":"internal-project","user_id":"internal-user","api_key":original.material.access,
        "api_base_url":InferenceTarget::Welcome.base_url(),"created_at":"2020-01-01T00:00:00Z",
        "updated_at":"2020-01-01T00:00:00Z","expires_at":EXPIRY,"quota_remaining_tokens":42,
        "account_payload":{"auth_mode":business::AUTH_MODE,"nous_portal":context(),
        "refresh_token":"nous-old-refresh","expires_at":EXPIRY}}));
    authority.backend.bind_cache(&cache.pool);
    authority.backend.stall(Stall::AfterGrantReceipt);
    let mock = Mock::new(vec![(200, super::nous_target::subscription_reply())], 0).await;
    let state = super::worker_state_fixture::state(&authority, &cache);
    let controller = async {
        authority.backend.wait_stalled().await;
        authority.backend.resume();
        state.shutdown.request("synthetic receipt ACK lost");
    };
    tokio::time::timeout(
        Duration::from_secs(5),
        SERVER_AUTHORITY_BUDGET.scope(
            Duration::from_millis(100),
            TEST_ORIGIN.scope(mock.origin.clone(), async {
                tokio::join!(
                    start_token_refresh_task(state.clone(), 3600, 300),
                    controller
                );
            }),
        ),
    )
    .await
    .expect("Nous worker shutdown did not flush receipt");
    assert_eq!(mock.count(), 1);
    assert_eq!(
        authority.saved().refresh_token.as_deref(),
        Some("nous-new-refresh")
    );
    assert_eq!(
        authority.saved().base_url.as_deref(),
        Some(InferenceTarget::Subscription.base_url())
    );
    assert_eq!(cache.commits(), 1);
    assert_eq!(
        cache.value()["api_base_url"],
        InferenceTarget::Subscription.base_url()
    );
    assert_eq!(
        cache.value()["account_payload"]["base_url"],
        InferenceTarget::Subscription.base_url()
    );
    assert_eq!(cache.value()["quota_remaining_tokens"], 42);
    let record = authority.backend.grant_record(&original).unwrap();
    assert_eq!(record.phase(), GrantPhase::Published);
    assert!(record.receipt().is_some());
    mock.stop().await;
    cache.stop().await;
}
