//! Opt-in actual scheduler/Pooled/Lua tests; only a disposable explicit loopback Redis is accepted.
use super::*;
use crate::console::{
    PooledRouteConfigRedisBackend, RouteConfigRedisActivationOutcome, RouteConfigRedisRevision,
    RouteConfigRedisStore, TransactionPhase, TransactionRecord,
};

pub(super) struct RedisAuthority {
    pub(super) pool: deadpool_redis::Pool,
    pub(super) backend: Arc<PooledRouteConfigRedisBackend>,
    pub(super) redis: RouteConfigRedisStore,
    revision: RouteConfigRedisRevision,
    pub(super) document: RouteConfigYaml,
}

impl RedisAuthority {
    pub(super) async fn new(endpoint: &str) -> Self {
        let url = std::env::var("GATEWAY_REFRESH_GRANT_REDIS_TEST_URL")
            .expect("explicit disposable Redis URL required");
        assert_eq!(url::Url::parse(&url).unwrap().host_str(), Some("127.0.0.1"));
        let pool = deadpool_redis::Config::from_url(url)
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .unwrap();
        let redis =
            RouteConfigRedisStore::new(format!("p33_{}", uuid::Uuid::new_v4().simple())).unwrap();
        let document = serde_json::from_value(serde_json::json!({"providers":[{
        "id":"oauth", "base_url":"https://example.invalid/v1", "credentials":[{
            "id":"account", "api_key":"old-access", "refresh_token":"old-refresh",
            "refresh_endpoint":endpoint, "refresh_client_id":"synthetic-client",
            "expires_at":"2020-01-01T00:00:00Z", "token_expires_in_secs":3600
        }]}]}))
        .unwrap();
        let store = RouteConfigStore::from_document(document).unwrap();
        let snapshot = store.snapshot();
        let document = snapshot.document().clone();
        let revision =
            RouteConfigRedisRevision::new(snapshot.revision().clone(), document.clone()).unwrap();
        let prepared = TransactionRecord::new_prepared(
            format!("tx-{}", uuid::Uuid::new_v4()),
            None,
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
        redis.store_revision(&pool, &revision).await.unwrap();
        redis
            .store_prepared_transaction(&pool, &prepared)
            .await
            .unwrap();
        assert_eq!(
            redis
                .activate_revision(&pool, None, &revision, &prepared, &activated)
                .await
                .unwrap(),
            RouteConfigRedisActivationOutcome::Activated
        );
        let backend = Arc::new(PooledRouteConfigRedisBackend::new(
            pool.clone(),
            redis.clone(),
        ));
        Self {
            pool,
            backend,
            redis,
            revision,
            document,
        }
    }

    fn runtime(&self) -> RouteConfigRuntime {
        let root =
            std::env::temp_dir().join(format!("p33-redis-scheduler-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let routes = root.join("routes.yaml");
        std::fs::write(&routes, serde_yaml::to_string(&self.document).unwrap()).unwrap();
        let persistence = RouteConfigPersistence::for_test(&root, &routes).unwrap();
        let store = Arc::new(RouteConfigStore::from_document(self.document.clone()).unwrap());
        RouteConfigRuntime::with_backend(store, persistence, self.backend.clone(), true)
    }

    async fn refresh(&self, runtime: &RouteConfigRuntime, pending: &mut Pending) {
        refresh_once(
            runtime.route_config(),
            &client(),
            Some(runtime),
            Some(&self.pool),
            pending,
            Duration::from_secs(300),
            None,
        )
        .await;
    }
}

#[tokio::test]
#[ignore = "requires explicit disposable Redis URL"]
async fn real_pooled_scheduler_separate_runtimes_confirm_only_one_http_sender() {
    let mock = MockOAuth::new(200, ROTATED, 150).await;
    let authority = RedisAuthority::new(&mock.url).await;
    let a = authority.runtime();
    let b = authority.runtime();
    let (mut left, mut right) = (Pending::new(), Pending::new());
    tokio::join!(
        authority.refresh(&a, &mut left),
        authority.refresh(&b, &mut right)
    );
    assert_eq!(mock.count(), 1);
    assert!(left.is_empty() && right.is_empty());
    let active = authority
        .redis
        .load_active_revision(&authority.pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        active.document().providers[0].credentials[0]
            .api_key
            .as_deref(),
        Some("new-access")
    );
    let request = crate::console::refresh_grants::GrantRequest::from_revision(
        &authority.revision,
        "oauth",
        "account",
    )
    .unwrap();
    let crate::console::refresh_grants::GrantReservation::Existing(record) = authority
        .redis
        .reserve_refresh_grant(&authority.pool, &authority.revision, &request)
        .await
        .unwrap()
    else {
        panic!("original consumption record must remain")
    };
    assert_eq!(
        record.phase(),
        crate::console::refresh_grants::GrantPhase::Published
    );
}

#[tokio::test]
#[ignore = "requires explicit disposable Redis URL"]
async fn real_pooled_scheduler_unknown_result_survives_new_runtime_without_reexchange() {
    let mock = MockOAuth::new(503, "unknown consumption", 0).await;
    let authority = RedisAuthority::new(&mock.url).await;
    let first = authority.runtime();
    authority.refresh(&first, &mut Pending::new()).await;
    assert_eq!(mock.count(), 1);
    drop(first);
    let restarted = authority.runtime();
    restarted.recover_startup().await.unwrap();
    authority.refresh(&restarted, &mut Pending::new()).await;
    assert_eq!(mock.count(), 1);
    let snapshot = restarted.route_config().snapshot();
    assert_eq!(
        snapshot.document().providers[0].credentials[0]
            .api_key
            .as_deref(),
        Some("old-access")
    );
}
