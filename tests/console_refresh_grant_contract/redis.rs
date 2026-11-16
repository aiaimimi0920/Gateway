//! Opt-in real Lua execution against a newly created disposable loopback Redis; no FLUSH commands.
use super::*;
use redis::AsyncCommands;

struct RedisFixture {
    pool: deadpool_redis::Pool,
    store: RouteConfigRedisStore,
    local: Fixture,
}
impl RedisFixture {
    async fn new() -> Self {
        let url = std::env::var("GATEWAY_REFRESH_GRANT_REDIS_TEST_URL")
            .expect("explicit disposable Redis URL required");
        assert_eq!(url::Url::parse(&url).unwrap().host_str(), Some("127.0.0.1"));
        let pool = deadpool_redis::Config::from_url(url)
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .unwrap();
        let store =
            RouteConfigRedisStore::new(format!("grant_{}", uuid::Uuid::new_v4().simple())).unwrap();
        let f = Self {
            pool,
            store,
            local: Fixture::new(),
        };
        f.activate(None, &f.local.revision).await;
        f
    }

    async fn activate(&self, parent: Option<&str>, revision: &RouteConfigRedisRevision) {
        let prepared = TransactionRecord::new_prepared(
            format!("tx-{}", uuid::Uuid::new_v4()),
            parent,
            revision.metadata().id(),
            format!("revisions/{}", revision.metadata().id()),
            false,
            None,
            revision.metadata().yaml_digest(),
            time::OffsetDateTime::now_utc(),
        )
        .unwrap();
        let mut activated = prepared.clone();
        activated
            .transition(
                TransactionPhase::YamlReplaced,
                time::OffsetDateTime::now_utc(),
                None,
            )
            .unwrap();
        activated
            .transition(
                TransactionPhase::RedisActivated,
                time::OffsetDateTime::now_utc(),
                None,
            )
            .unwrap();
        self.store
            .store_revision(&self.pool, revision)
            .await
            .unwrap();
        self.store
            .store_prepared_transaction(&self.pool, &prepared)
            .await
            .unwrap();
        assert_eq!(
            self.store
                .activate_revision(&self.pool, parent, revision, &prepared, &activated)
                .await
                .unwrap(),
            RouteConfigRedisActivationOutcome::Activated
        );
    }

    async fn reserve(&self, request: &GrantRequest) -> GrantReservation {
        self.store
            .reserve_refresh_grant(&self.pool, &self.local.revision, request)
            .await
            .unwrap()
    }
}

#[tokio::test]
#[ignore = "requires explicit disposable Redis URL"]
async fn pooled_backend_accepts_bootstrap_metadata_but_preserves_the_real_immutable_revision_raw() {
    let f = RedisFixture::new().await;
    let expected = metadata_variant(&f.local.revision);
    let request = GrantRequest::from_revision(&expected, "oauth", "account").unwrap();
    let key = f
        .store
        .keys()
        .revision_key(f.local.revision.metadata().id());
    let mut connection = f.pool.get().await.unwrap();
    let before: String = connection.get(&key).await.unwrap();
    assert_ne!(before, expected.to_json().unwrap());
    let backend = PooledRouteConfigRedisBackend::new(f.pool.clone(), f.store.clone());
    assert!(matches!(
        backend
            .reserve_refresh_grant(&expected, &request)
            .await
            .unwrap(),
        GrantReservation::Reserved(_)
    ));
    let after: String = connection.get(&key).await.unwrap();
    assert_eq!(
        after, before,
        "reserve must CAS against, never rewrite, the real raw revision"
    );
}

#[tokio::test]
#[ignore = "requires explicit disposable Redis URL"]
async fn concurrent_instances_have_one_confirmed_sender_and_no_lease_expiry() {
    let f = RedisFixture::new().await;
    let request = f.local.request();
    let other_store = RouteConfigRedisStore::new(f.store.keys().namespace()).unwrap();
    let (left, right) = tokio::join!(
        f.reserve(&request),
        other_store.reserve_refresh_grant(&f.pool, &f.local.revision, &request)
    );
    let right = right.unwrap();
    assert_eq!(
        [&left, &right]
            .into_iter()
            .filter(|r| matches!(r, GrantReservation::Reserved(_)))
            .count(),
        1
    );
    assert!(matches!(
        f.reserve(&request).await,
        GrantReservation::Existing(_)
    ));
    let mut connection = f.pool.get().await.unwrap();
    for key in [
        f.store
            .refresh_grant_key(&request.family_id(), None)
            .unwrap(),
        f.store
            .refresh_grant_key(&request.family_id(), Some(&request.material_id()))
            .unwrap(),
    ] {
        assert_eq!(connection.ttl::<_, i64>(key).await.unwrap(), -1);
    }
}

#[tokio::test]
#[ignore = "requires explicit disposable Redis URL"]
async fn durable_result_replays_and_same_tokens_can_renew_only_after_proven_publication() {
    let f = RedisFixture::new().await;
    let request = f.local.request();
    let GrantReservation::Reserved(first) = f.reserve(&request).await else {
        panic!("not reserved")
    };
    let result = request
        .result("synthetic-access", None, 3600, "2030-01-01T00:00:00Z", None)
        .unwrap();
    let saved = f
        .store
        .store_refresh_grant_result(&f.pool, &first, &result)
        .await
        .unwrap();
    assert_eq!(
        f.store
            .store_refresh_grant_result(&f.pool, &first, &result)
            .await
            .unwrap(),
        saved
    );
    assert!(f
        .store
        .mark_refresh_grant_published(&f.pool, &f.local.revision, &saved)
        .await
        .is_err());
    let next = f
        .local
        .publish(
            "synthetic-access",
            "synthetic-refresh",
            "2030-01-01T00:00:00Z",
        )
        .await;
    f.activate(Some(f.local.revision.metadata().id()), &next)
        .await;
    let next_request = GrantRequest::from_revision(&next, "oauth", "account").unwrap();
    assert!(matches!(
        f.store
            .reserve_refresh_grant(&f.pool, &next, &next_request)
            .await
            .unwrap(),
        GrantReservation::Blocked
    ));
    let published = f
        .store
        .mark_refresh_grant_published(&f.pool, &next, &saved)
        .await
        .unwrap();
    assert_eq!(published.phase(), GrantPhase::Published);
    assert_eq!(
        f.store
            .store_refresh_grant_result(&f.pool, &first, &result)
            .await
            .unwrap(),
        published
    );
    assert!(matches!(
        f.store
            .reserve_refresh_grant(&f.pool, &next, &next_request)
            .await
            .unwrap(),
        GrantReservation::Reserved(_)
    ));
    assert!(
        matches!(f.reserve(&request).await, GrantReservation::Existing(ref old) if old == &published)
    );
}

#[tokio::test]
#[ignore = "requires explicit disposable Redis URL"]
async fn authority_cas_rejects_stale_snapshot_without_creating_a_send_intent() {
    let f = RedisFixture::new().await;
    let request = f.local.request();
    let next = f
        .local
        .publish("new-access", "new-refresh", "2030-01-01T00:00:00Z")
        .await;
    f.activate(Some(f.local.revision.metadata().id()), &next)
        .await;
    assert!(matches!(f.reserve(&request).await, GrantReservation::Stale));
    let key = f
        .store
        .refresh_grant_key(&request.family_id(), None)
        .unwrap();
    assert!(!f
        .pool
        .get()
        .await
        .unwrap()
        .exists::<_, bool>(key)
        .await
        .unwrap());
}

#[tokio::test]
#[ignore = "requires explicit disposable Redis URL"]
async fn unknown_family_blocks_edited_access_and_expiry_but_not_another_namespace() {
    let f = RedisFixture::new().await;
    let request = f.local.request();
    assert!(matches!(
        f.reserve(&request).await,
        GrantReservation::Reserved(_)
    ));
    let next = f
        .local
        .publish("edited-access", "synthetic-refresh", "2030-01-01T00:00:00Z")
        .await;
    f.activate(Some(f.local.revision.metadata().id()), &next)
        .await;
    let edited = GrantRequest::from_revision(&next, "oauth", "account").unwrap();
    assert!(matches!(
        f.store
            .reserve_refresh_grant(&f.pool, &next, &edited)
            .await
            .unwrap(),
        GrantReservation::Blocked
    ));
    let separate = RedisFixture {
        pool: f.pool.clone(),
        store: RouteConfigRedisStore::new(format!("grant_{}", uuid::Uuid::new_v4().simple()))
            .unwrap(),
        local: Fixture::new(),
    };
    separate.activate(None, &f.local.revision).await;
    assert!(matches!(
        separate
            .store
            .reserve_refresh_grant(&f.pool, &f.local.revision, &request)
            .await
            .unwrap(),
        GrantReservation::Reserved(_)
    ));
}

#[tokio::test]
#[ignore = "requires explicit disposable Redis URL"]
async fn malformed_or_oversized_head_and_unknown_record_do_not_reset_consumption() {
    let f = RedisFixture::new().await;
    let request = f.local.request();
    let head_key = f
        .store
        .refresh_grant_key(&request.family_id(), None)
        .unwrap();
    let mut connection = f.pool.get().await.unwrap();
    for raw in [
        "../escape".to_string(),
        "a".repeat(65),
        request.material_id(),
    ] {
        let _: () = connection.set(&head_key, raw).await.unwrap();
        assert!(f
            .store
            .reserve_refresh_grant(&f.pool, &f.local.revision, &request)
            .await
            .is_err());
    }
    let key = f
        .store
        .refresh_grant_key(&request.family_id(), Some(&request.material_id()))
        .unwrap();
    let _: () = connection
        .set(&key, "{\"version\":2,\"secret\":\"private-token\"}")
        .await
        .unwrap();
    let error = f
        .store
        .reserve_refresh_grant(&f.pool, &f.local.revision, &request)
        .await
        .unwrap_err();
    assert!(!error.to_string().contains("private-token"));
    let _: () = connection.set(&key, "x".repeat(524_289)).await.unwrap();
    assert!(f
        .store
        .reserve_refresh_grant(&f.pool, &f.local.revision, &request)
        .await
        .is_err());
}
