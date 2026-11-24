//! Real persistence and independent owners, without contacting the provider.
use super::*;
use crate::console::{ConsoleConfig, ConsoleConfigValues};
use serde_json::{json, Value};

pub(super) fn config() -> ConsoleConfig {
    let root = std::env::temp_dir().join(format!("luma-web-session-{}", uuid::Uuid::new_v4()));
    ConsoleConfig::from_values(ConsoleConfigValues {
        state_dir: Some(root.join("state")),
        routes_file: Some(root.join("routes.yaml")),
        ..Default::default()
    })
    .unwrap()
}
pub(super) fn payload() -> ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":"lumalabs_compatible", "base_url":"https://app.lumalabs.ai",
        "api_key":"synthetic-original", "credential_id":"account", "extra_body":{"realmId":"fixture"}})).unwrap()
}
fn receipt(attempt: &SessionAttempt, token: &str) -> Value {
    json!({"version":1,"binding":attempt.binding(),"reauthenticationRequired":false,
        "cookie":{"name":"wos-session","value":token,"domain":"app.lumalabs.ai","path":"/",
        "expires":now()+3600,"httpOnly":true,"secure":true,"sameSite":"Lax"}})
}
#[tokio::test]
async fn luma_web_session_reopen_uses_rotated_cookie_and_old_receipt_cannot_overwrite() {
    let config = config();
    let store = SessionStore::new(&config, None).unwrap();
    let old = store
        .begin("provider", &payload(), false)
        .await
        .unwrap()
        .unwrap();
    let successor = receipt(&old, "synthetic-next");
    store.finish(&old, successor.clone()).await.unwrap();
    drop(store);
    let store = SessionStore::new(&config, None).unwrap();
    assert!(store
        .begin("provider", &payload(), true)
        .await
        .unwrap()
        .is_none());
    let current = store
        .begin("provider", &payload(), false)
        .await
        .unwrap()
        .unwrap();
    assert!(current.token() == "synthetic-next");
    assert!(store.finish(&old, successor).await.is_err());
    store
        .finish(&current, receipt(&current, "synthetic-newest"))
        .await
        .unwrap();
    let current = store
        .begin("provider", &payload(), false)
        .await
        .unwrap()
        .unwrap();
    assert!(current.token() == "synthetic-newest");
}
#[tokio::test]
async fn luma_web_session_independent_owners_and_restart_never_replay_unconfirmed_send() {
    let config = config();
    let first = SessionStore::new(&config, None).unwrap();
    let second = SessionStore::new(&config, None).unwrap();
    let payload = payload();
    let (a, b) = tokio::join!(
        first.begin("provider", &payload, false),
        second.begin("provider", &payload, false)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    drop((first, second, a, b));
    let reopened = SessionStore::new(&config, None).unwrap();
    assert!(reopened.begin("provider", &payload, false).await.is_err());
    let mut changed = payload;
    changed.api_key = "new-browser-login".into();
    assert!(reopened.begin("provider", &changed, false).await.is_ok());
}
#[tokio::test]
async fn luma_web_session_validates_receipt_audience_owner_and_login_refusal() {
    let store = SessionStore::new(&config(), None).unwrap();
    let attempt = store
        .begin("provider", &payload(), false)
        .await
        .unwrap()
        .unwrap();
    for (path, value) in [
        ("/binding", json!("foreign")),
        ("/cookie/domain", json!("evil.invalid")),
        ("/cookie/httpOnly", json!(false)),
        ("/cookie/expires", json!(1)),
        ("/cookie/value", json!("cookie;injection")),
    ] {
        let mut bad = receipt(&attempt, "successor");
        *bad.pointer_mut(path).unwrap() = value;
        assert!(store.finish(&attempt, bad).await.is_err());
    }
    let mut rejected = receipt(&attempt, "successor");
    rejected["reauthenticationRequired"] = true.into();
    store.finish(&attempt, rejected).await.unwrap();
    let result = store.begin("provider", &payload(), false).await;
    assert!(
        matches!(result, Err(e) if e.code.as_deref() == Some("lumalabs_session_login_required"))
    );
}
#[test]
fn luma_web_session_identity_separates_accounts_realm_ua_and_original_material() {
    let original = payload();
    let key = identity("provider", &original).unwrap().0;
    let mut changed = original.clone();
    changed.default_model = Some("unrelated-model-policy".into());
    assert_eq!(key, identity("provider", &changed).unwrap().0);
    changed
        .headers
        .insert("User-Agent".into(), "different".into());
    assert_ne!(key, identity("provider", &changed).unwrap().0);
    changed = original.clone();
    changed
        .extra_body
        .as_mut()
        .unwrap()
        .insert("realmId".into(), json!("other"));
    assert_ne!(key, identity("provider", &changed).unwrap().0);
    assert_ne!(key, identity("other", &original).unwrap().0);
    changed.base_url = "https://evil.invalid".into();
    assert!(identity("provider", &changed).is_err());
}

#[tokio::test]
async fn luma_web_session_failed_save_retains_successor_for_storage_only_retry() {
    let config = config();
    let store = SessionStore::new(&config, None).unwrap();
    let attempt = store
        .begin("provider", &payload(), false)
        .await
        .unwrap()
        .unwrap();
    let persistence =
        crate::console::RouteConfigPersistence::new(&config.state_dir, &config.routes_file)
            .unwrap();
    let lock = persistence.try_writer_lock().unwrap();
    assert!(store
        .finish(&attempt, receipt(&attempt, "retained-successor"))
        .await
        .is_err());
    assert_eq!(store.pending.lock().len(), 1);
    drop(lock);
    let recovered = store
        .begin("provider", &payload(), false)
        .await
        .unwrap()
        .unwrap();
    assert!(recovered.token() == "retained-successor");
    assert!(store.pending.lock().is_empty());
}

#[tokio::test]
#[ignore = "requires isolated Redis: tools/luma/check-web-session-redis.ps1"]
async fn luma_web_session_redis_cas_survives_owner_restart() {
    let url = std::env::var("GATEWAY_LUMA_TEST_REDIS_URL").expect("isolated Redis URL");
    let mut config = config();
    config.redis_namespace = format!("luma-session-{}", uuid::Uuid::new_v4().simple());
    let pool = deadpool_redis::Config::from_url(url)
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .unwrap();
    let store = SessionStore::new(&config, Some(pool.clone())).unwrap();
    let second = SessionStore::new(&config, Some(pool.clone())).unwrap();
    let p = payload();
    let (a, b) = tokio::join!(
        store.begin("provider", &p, false),
        second.begin("provider", &p, false)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let attempt = a.ok().flatten().or_else(|| b.ok().flatten()).unwrap();
    store
        .finish(&attempt, receipt(&attempt, "redis-successor"))
        .await
        .unwrap();
    drop((store, second));
    let reopened = SessionStore::new(&config, Some(pool)).unwrap();
    let current = reopened
        .begin("provider", &p, false)
        .await
        .unwrap()
        .unwrap();
    assert!(current.token() == "redis-successor");
    assert!(reopened
        .finish(&attempt, receipt(&attempt, "stale-receipt"))
        .await
        .is_err());
    reopened
        .finish(&current, receipt(&current, "redis-final"))
        .await
        .unwrap();
    let current = reopened
        .begin("provider", &p, false)
        .await
        .unwrap()
        .unwrap();
    assert!(current.token() == "redis-final");
}
