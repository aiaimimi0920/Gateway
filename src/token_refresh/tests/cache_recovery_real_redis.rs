//! Opt-in Redis proof; durable publication is real, the stale-cache crash state is explicitly seeded.
use super::{durable_real_redis::RedisAuthority, *};
use redis::AsyncCommands;

fn persistence(authority: &RedisAuthority) -> RouteConfigPersistence {
    let root = std::env::temp_dir().join(format!("p34-real-cache-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let routes = root.join("routes.yaml");
    std::fs::write(&routes, serde_yaml::to_string(&authority.document).unwrap()).unwrap();
    RouteConfigPersistence::for_test(&root, &routes).unwrap()
}

fn runtime(authority: &RedisAuthority, persistence: &RouteConfigPersistence) -> RouteConfigRuntime {
    RouteConfigRuntime::with_backend(
        Arc::new(RouteConfigStore::from_document(authority.document.clone()).unwrap()),
        persistence.clone(),
        authority.backend.clone(),
        true,
    )
}

async fn refresh(authority: &RedisAuthority, runtime: &RouteConfigRuntime) {
    refresh_once(
        runtime.route_config(),
        &client(),
        Some(runtime),
        Some(&authority.pool),
        &mut Pending::new(),
        Duration::from_secs(300),
        None,
    )
    .await;
}

fn stale_cache(endpoint: &str) -> String {
    serde_json::json!({"id":"account","kind":"account_credential",
        "project_id":"synthetic-project","user_id":"synthetic-user","provider":"openai",
        "api_key":"old-access","api_base_url":"https://example.invalid/v1",
        "created_at":"2020-01-01T00:00:00Z","updated_at":"2020-01-01T00:00:00Z",
        "expires_at":"2020-01-01T00:00:00Z","quota_remaining_tokens":42,
        "unknown_field":"preserved","account_payload":{"refresh_token":"old-refresh",
            "refresh_endpoint":endpoint,"refresh_client_id":"synthetic-client"}})
    .to_string()
}

#[tokio::test]
#[ignore = "requires explicit disposable Redis URL"]
async fn real_pooled_new_runtime_repairs_seeded_stale_cache_from_exact_publication() {
    let mock = MockOAuth::new(200, ROTATED, 0).await;
    let authority = RedisAuthority::new(&mock.url).await;
    let persistence = persistence(&authority);
    let first = runtime(&authority, &persistence);
    refresh(&authority, &first).await;
    assert_eq!(mock.count(), 1);
    // Seed the durable old-cache state of the crash window, not an actual process-kill claim.
    let key = crate::redis::keys::credential_key("account");
    let mut connection = authority.pool.get().await.unwrap();
    let _: () = connection.set(&key, stale_cache(&mock.url)).await.unwrap();
    drop(connection);
    drop(first);
    let rebuilt = runtime(&authority, &persistence);
    rebuilt.recover_startup().await.unwrap();
    refresh(&authority, &rebuilt).await;
    let mut connection = authority.pool.get().await.unwrap();
    let raw: String = connection.get(&key).await.unwrap();
    let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(value["api_key"], "new-access");
    assert_eq!(value["account_payload"]["refresh_token"], "new-refresh");
    assert_eq!(value["quota_remaining_tokens"], 42);
    assert_eq!(value["unknown_field"], "preserved");
    let ttl: i64 = connection.ttl(&key).await.unwrap();
    assert!((1..=3600).contains(&ttl));
    drop(connection);
    refresh(&authority, &rebuilt).await;
    assert_eq!(mock.count(), 1);
}

#[tokio::test]
#[ignore = "requires explicit disposable Redis URL"]
async fn real_cache_lua_rejects_each_changed_authority_grant_transaction_and_cache_raw() {
    let mock = MockOAuth::new(200, ROTATED, 0).await;
    let authority = RedisAuthority::new(&mock.url).await;
    let persistence = persistence(&authority);
    let active_runtime = runtime(&authority, &persistence);
    refresh(&authority, &active_runtime).await;
    let active = authority
        .redis
        .load_active_revision(&authority.pool)
        .await
        .unwrap()
        .unwrap();
    let publication = {
        let guard = persistence.try_writer_lock().unwrap();
        persistence
            .load_refresh_publications_locked(&guard, active.metadata().id())
            .unwrap()
            .pop()
            .unwrap()
    };
    let record = authority
        .redis
        .load_refresh_grant(&authority.pool, &publication.grant)
        .await
        .unwrap()
        .unwrap();
    let keys = [
        authority.redis.keys().active_revision_key().to_string(),
        authority.redis.keys().revision_key(active.metadata().id()),
        authority
            .redis
            .refresh_grant_key(&record.family_id(), Some(&record.material_id()))
            .unwrap(),
        authority
            .redis
            .keys()
            .transaction_key(publication.intent.prepared.tx_id()),
        crate::redis::keys::credential_key("account"),
    ];
    let mut connection = authority.pool.get().await.unwrap();
    let stale = stale_cache(&mock.url);
    let _: () = connection.set(&keys[4], &stale).await.unwrap();
    let observed: Vec<String> = redis::cmd("MGET")
        .arg(&keys)
        .query_async(&mut connection)
        .await
        .unwrap();
    let snapshot = active_runtime.route_config().snapshot();
    let (_, credential) = snapshot.oauth_refresh_target("oauth", "account").unwrap();
    let (updated, ttl) =
        super::super::server_cache::patch_from_grant(&stale, &record, &credential.payload)
            .unwrap()
            .unwrap();
    for changed in 0..keys.len() {
        let _: () = connection
            .set(&keys[changed], "independently-changed")
            .await
            .unwrap();
        let script = redis::Script::new(crate::console::redis_store::REFRESH_CACHE_REPAIR);
        let mut invocation = script.prepare_invoke();
        for key in &keys {
            invocation.key(key);
        }
        for raw in &observed {
            invocation.arg(raw);
        }
        invocation.arg(&updated).arg(ttl);
        let applied: i32 = invocation.invoke_async(&mut connection).await.unwrap();
        assert_eq!(applied, if changed == 4 { 0 } else { -1 });
        let raw: String = connection.get(&keys[4]).await.unwrap();
        assert_eq!(
            raw,
            if changed == 4 {
                "independently-changed"
            } else {
                &stale
            }
        );
        let _: () = connection
            .set(&keys[changed], &observed[changed])
            .await
            .unwrap();
    }
    drop(connection);
    authority
        .redis
        .repair_refresh_grant_cache(
            &authority.pool,
            &active,
            &record,
            &publication.intent.prepared,
        )
        .await
        .unwrap();
    let mut connection = authority.pool.get().await.unwrap();
    let raw: String = connection.get(&keys[4]).await.unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&raw).unwrap()["api_key"],
        "new-access"
    );
    assert_eq!(mock.count(), 1);
}
