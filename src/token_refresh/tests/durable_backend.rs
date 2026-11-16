//! A backend implementing only the old authority contract cannot silently bypass durable admission.
use super::*;
use crate::console::{
    RouteConfigRedisActivationOutcome, RouteConfigRedisBackend, RouteConfigRedisRevision,
    RouteConfigRedisStoreError, TransactionRecord,
};

struct Unsupported;
#[async_trait::async_trait]
impl RouteConfigRedisBackend for Unsupported {
    async fn store_revision(
        &self,
        _: &RouteConfigRedisRevision,
    ) -> Result<(), RouteConfigRedisStoreError> {
        panic!("must not mutate")
    }
    async fn store_prepared_transaction(
        &self,
        _: &TransactionRecord,
    ) -> Result<(), RouteConfigRedisStoreError> {
        panic!("must not mutate")
    }
    async fn activate_revision(
        &self,
        _: Option<&str>,
        _: &RouteConfigRedisRevision,
        _: &TransactionRecord,
        _: &TransactionRecord,
    ) -> Result<RouteConfigRedisActivationOutcome, RouteConfigRedisStoreError> {
        panic!("must not mutate")
    }
    async fn load_active_revision(
        &self,
    ) -> Result<Option<RouteConfigRedisRevision>, RouteConfigRedisStoreError> {
        panic!("no load needed before rejected reservation")
    }
}

#[tokio::test]
async fn default_unsupported_backend_rejects_the_scheduler_before_any_send() {
    let mock = MockOAuth::new(200, ROTATED, 0).await;
    let fixture = LocalFixture::new(&mock.url).await;
    let persistence =
        RouteConfigPersistence::new(&fixture.config.state_dir, &fixture.config.routes_file)
            .unwrap();
    let runtime = RouteConfigRuntime::with_backend(
        fixture.runtime.route_config().clone(),
        persistence,
        Arc::new(Unsupported),
        true,
    );
    refresh_once(
        runtime.route_config(),
        &client(),
        Some(&runtime),
        None,
        &mut Pending::new(),
        Duration::from_secs(300),
        None,
    )
    .await;
    assert_eq!(mock.count(), 0);
    assert_eq!(fixture.saved().api_key.as_deref(), Some("old-access"));
}

#[tokio::test]
async fn preparing_antigravity_before_reserve_does_not_burn_a_valid_generic_family() {
    let mock = MockOAuth::new(200, ROTATED, 0).await;
    let fixture = LocalFixture::new(&mock.url).await;
    let snapshot = fixture.runtime.route_config().snapshot();
    let mut document = snapshot.document().clone();
    document.providers[0].adapter = Some(crate::protocol::antigravity::ADAPTER.into());
    fixture
        .runtime
        .commit_document(snapshot.revision().id(), document, None)
        .await
        .unwrap();
    fixture.refresh(&mut Pending::new()).await;
    assert_eq!(mock.count(), 0);
    let persistence =
        RouteConfigPersistence::new(&fixture.config.state_dir, &fixture.config.routes_file)
            .unwrap();
    assert_eq!(
        std::fs::read_dir(persistence.state_root().join("refresh-grants"))
            .unwrap()
            .count(),
        0,
        "pre-send validation must precede permanent reservation"
    );
    let snapshot = fixture.runtime.route_config().snapshot();
    let mut document = snapshot.document().clone();
    document.providers[0].adapter = Some("openai_compatible".into());
    fixture
        .runtime
        .commit_document(snapshot.revision().id(), document, None)
        .await
        .unwrap();
    fixture.refresh(&mut Pending::new()).await;
    assert_eq!(
        mock.count(),
        1,
        "correcting an unsent configuration remains recoverable"
    );
}
