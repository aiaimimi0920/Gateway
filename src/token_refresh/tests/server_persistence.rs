//! Exercise the real Server writeback transport with a random-loopback, exact-CAS RESP witness.
use super::grok_wire::{rotated, Mock};
use super::*;
use super::{
    cache_wire::Cache,
    server_policy::{edit, owner},
};
use serde_json::json;

async fn refresh(
    authority: &authority_fixture::Authority,
    mock: &Mock,
    cache: &Cache,
    pending: &mut Pending,
) {
    super::super::grok::TEST_ISSUER
        .scope(
            mock.issuer.clone(),
            refresh_once(
                authority.runtime.route_config(),
                &client(),
                Some(&authority.runtime),
                Some(&cache.pool),
                pending,
                Duration::from_secs(300),
                None,
            ),
        )
        .await;
}
fn narrowed() -> serde_json::Value {
    let mut reply = rotated();
    reply["scope"] = json!("profile");
    reply
}

#[tokio::test]
async fn server_cache_outage_policy_swap_and_recovery_only_retry_persistence_then_keep_scope_ceiling(
) {
    let authority = authority_fixture::Authority::new();
    let store = authority.runtime.route_config();
    let mock = Mock::new(200, narrowed(), 0).await;
    let cache = Cache::new(false).await;
    authority.backend.bind_cache(&cache.pool);
    let mut pending = Pending::new();
    refresh(&authority, &mock, &cache, &mut pending).await;
    assert_eq!(pending.len(), 1);
    edit(&authority, "policy").await;
    edit(&authority, "disable").await;
    edit(&authority, "enable").await;
    // A second scheduler's independent map must not bypass the pending generation's single-flight lock.
    super::super::grok::TEST_ISSUER
        .scope(
            mock.issuer.clone(),
            refresh_once(
                &store,
                &client(),
                Some(&authority.runtime),
                Some(&cache.pool),
                &mut Pending::new(),
                Duration::MAX,
                None,
            ),
        )
        .await;
    refresh(&authority, &mock, &cache, &mut pending).await;
    assert_eq!(mock.count(), 1);
    assert_eq!(cache.commits(), 0);
    cache.allow_writes();
    // The stored generation remains fresh; recovery retries storage without starting a second grant.
    refresh(&authority, &mock, &cache, &mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(cache.commits(), 1);
    let value = cache.value();
    assert_eq!(value["quota_remaining_tokens"], 42);
    assert_eq!(value["api_key"], "rotated-build-access");
    assert_eq!(
        value["account_payload"]["refresh_token"],
        "rotated-build-refresh"
    );
    assert_eq!(
        value["account_payload"]["grok_oauth"]["granted_scopes"],
        json!(["profile"])
    );
    let expanded = Mock::new(
        200,
        json!({"access_token":"must-not-apply","expires_in":3600,"scope":"openid profile"}),
        0,
    )
    .await;
    owner(&store).force_due();
    refresh(&authority, &expanded, &cache, &mut pending).await;
    assert_eq!(expanded.count(), 1);
    assert_eq!(
        owner(&store).runtime_override().0.as_deref(),
        Some("rotated-build-access")
    );
    assert_eq!(cache.commits(), 1);
    let calls = expanded.calls.lock().unwrap().clone();
    let form: HashMap<_, _> = url::form_urlencoded::parse(calls[0].0.as_bytes())
        .into_owned()
        .collect();
    assert_eq!(form["refresh_token"], "rotated-build-refresh");
    expanded.stop().await;
    mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn server_successful_cache_rotation_then_policy_change_uses_runtime_expected_material_on_next_round(
) {
    let authority = authority_fixture::Authority::new();
    let store = authority.runtime.route_config();
    let mock = Mock::new(200, narrowed(), 0).await;
    let cache = Cache::new(true).await;
    authority.backend.bind_cache(&cache.pool);
    let mut pending = Pending::new();
    refresh(&authority, &mock, &cache, &mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(cache.commits(), 1);
    let old = owner(&store);
    edit(&authority, "policy").await;
    assert!(!old.is_active());
    assert_eq!(
        store.snapshot().document().providers[0].credentials[0]
            .api_key
            .as_deref(),
        Some("rotated-build-access")
    );
    assert_eq!(
        owner(&store).runtime_override().0.as_deref(),
        Some("rotated-build-access")
    );
    let second = Mock::new(
        200,
        json!({"access_token":"second-access","refresh_token":"second-refresh","expires_in":3600}),
        0,
    )
    .await;
    owner(&store).force_due();
    refresh(&authority, &second, &cache, &mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(
        cache.commits(),
        2,
        "second raw CAS must use runtime, not startup token/context"
    );
    assert_eq!(cache.value()["api_key"], "second-access");
    assert_eq!(
        cache.value()["account_payload"]["grok_oauth"]["granted_scopes"],
        json!(["profile"])
    );
    second.stop().await;
    mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn server_pending_cannot_survive_delete_and_readd_of_identical_material() {
    let authority = authority_fixture::Authority::new();
    let store = authority.runtime.route_config();
    let initial = store.snapshot().document().clone();
    let mock = Mock::new(200, narrowed(), 0).await;
    let cache = Cache::new(false).await;
    authority.backend.bind_cache(&cache.pool);
    let mut pending = Pending::new();
    refresh(&authority, &mock, &cache, &mut pending).await;
    let old = owner(&store);
    edit(&authority, "delete").await;
    let mut restored = initial;
    restored.providers[0].credentials[0].enabled = Some(false);
    authority
        .runtime
        .commit_document(store.snapshot().revision().id(), restored, None)
        .await
        .unwrap();
    assert!(!old.is_active());
    assert!(!pending
        .values()
        .next()
        .unwrap()
        .as_result()
        .unwrap()
        .is_current(&store));
    cache.allow_writes();
    refresh(&authority, &mock, &cache, &mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(mock.count(), 1);
    assert_eq!(cache.commits(), 0);
    assert_eq!(cache.value()["api_key"], "synthetic-build-token");
    mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn local_rotation_save_advances_document_anchor_without_replacing_the_authorization() {
    let fixture = grok_wire::fixture().await;
    let old = owner(fixture.runtime.route_config());
    let mock = Mock::new(200, narrowed(), 0).await;
    grok_wire::refresh(&fixture, &mock, &mut Pending::new()).await;
    assert!(Arc::ptr_eq(&old, &owner(fixture.runtime.route_config())));
    assert!(old.is_active());
    assert_eq!(
        fixture.saved().refresh_token.as_deref(),
        Some("rotated-build-refresh")
    );
    mock.stop().await;
}
