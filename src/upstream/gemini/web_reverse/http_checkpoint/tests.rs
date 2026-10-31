//! Synthetic records only; no env mutation and no live credentials or browser export.
use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use rquest::header::SET_COOKIE;
use serde_json::json;

fn directory() -> PathBuf {
    #[cfg(windows)]
    let root = PathBuf::from(
        "C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-gemini-dual-mode-20261030-r1/r17",
    );
    #[cfg(not(windows))]
    let root = std::env::temp_dir().join("gateway-gemini-http-checkpoint-tests");
    root.join(format!("unit-{}", uuid::Uuid::new_v4()))
}
fn config(dir: &std::path::Path) -> Config {
    Config {
        directory: dir.to_owned(),
        key: [7u8; 32],
    }
}
fn payload() -> ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":"gemini_web_compatible", "base_url":"https://gemini.google.com", "api_key":"", "credential_id":"synthetic-id"})).unwrap()
}
fn headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(COOKIE, "pref=synthetic-seed".parse().unwrap());
    headers
}
fn cookies(value: &str) -> SessionCookies {
    let mut jar = SessionCookies::new("https://gemini.google.com/app", &headers()).unwrap();
    let mut update = HeaderMap::new();
    update.insert(
        SET_COOKIE,
        format!("pref={value}; Path=/; Secure").parse().unwrap(),
    );
    jar.receive("https://gemini.google.com/app", &update);
    jar
}
async fn load(
    dir: &std::path::Path,
    payload: &ProviderAccountPayload,
) -> (Checkpoint, Option<CookieSnapshot>) {
    Checkpoint::load_with(config(dir), &payload.base_url, payload, &headers())
        .await
        .unwrap()
}

#[test]
fn http_checkpoint_config_is_disabled_or_strictly_explicit() {
    assert!(Config::from_values("disabled", "invalid", "invalid")
        .unwrap()
        .is_none());
    let key = STANDARD.encode([7u8; 32]);
    for (driver, dir, key) in [
        ("s3", "", key.as_str()),
        ("local", "relative", key.as_str()),
        ("local", "\\\\server\\share", key.as_str()),
        ("local", "C:/absolute", "wrong"),
    ] {
        assert_eq!(
            Config::from_values(driver, dir, key)
                .err()
                .unwrap()
                .code
                .as_deref(),
            Some("gemini_web_http_auth_checkpoint_config_invalid")
        );
    }
}

#[test]
fn http_checkpoint_crypto_binds_state_and_rejects_corruption() {
    let cfg = config(&directory());
    let (_, bytes) = codec::encrypt(
        &cfg,
        "binding-a",
        Some(cookies("synthetic-rotated").snapshot()),
    )
    .unwrap();
    assert!(codec::decrypt(&cfg, "binding-a", bytes.clone()).is_ok());
    assert!(codec::decrypt(&cfg, "binding-b", bytes.clone()).is_err());
    let mut damaged = bytes;
    *damaged.last_mut().unwrap() ^= 1;
    assert!(codec::decrypt(&cfg, "binding-a", damaged).is_err());
    assert!(codec::decrypt(&cfg, "binding-a", vec![0; MAX_BYTES + 1]).is_err());
}

#[tokio::test]
async fn http_checkpoint_local_cas_restore_and_ciphertext_only() {
    let dir = directory();
    let (mut first, _) = load(&dir, &payload()).await;
    let (mut stale, _) = load(&dir, &payload()).await;
    first
        .save(Some(cookies("synthetic-rotated").snapshot()))
        .await
        .unwrap();
    assert_eq!(
        stale.save(None).await.unwrap_err().code.as_deref(),
        Some("gemini_web_http_auth_checkpoint_conflict")
    );
    let (mut current, state) = load(&dir, &payload()).await;
    let restored =
        SessionCookies::restore("https://gemini.google.com/app", state.unwrap()).unwrap();
    let mut sent = HeaderMap::new();
    restored
        .apply("https://gemini.google.com/app", &mut sent)
        .unwrap();
    assert_eq!(sent[COOKIE], "pref=synthetic-rotated");
    let bytes = std::fs::read(dir.join("http-auth-checkpoints-v1.sqlite3")).unwrap();
    for private in ["synthetic-rotated", "synthetic-id", "synthetic-seed"] {
        assert!(!bytes
            .windows(private.len())
            .any(|part| part == private.as_bytes()));
    }
    current.save(None).await.unwrap();
    let error = Checkpoint::load_with(config(&dir), &payload().base_url, &payload(), &headers())
        .await
        .err()
        .unwrap();
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_web_http_auth_checkpoint_session_invalid")
    );
    assert!(!error.retryable);
}

#[tokio::test]
async fn http_checkpoint_seed_identity_origin_and_key_namespaces_do_not_mix() {
    let dir = directory();
    let p = payload();
    let (mut saved, _) = load(&dir, &p).await;
    saved
        .save(Some(cookies("rotated").snapshot()))
        .await
        .unwrap();
    let mut other = p.clone();
    other.credential_id = Some("different-account".into());
    assert!(load(&dir, &other).await.1.is_none());
    other = p.clone();
    other.runtime_state_object_key = Some("different-source".into());
    assert!(load(&dir, &other).await.1.is_none());
    other = p.clone();
    other.base_url = "https://other.example".into();
    assert!(load(&dir, &other).await.1.is_none());
    let mut changed = headers();
    changed.insert(COOKIE, "pref=new-seed".parse().unwrap());
    assert!(
        Checkpoint::load_with(config(&dir), &p.base_url, &p, &changed)
            .await
            .unwrap()
            .1
            .is_none()
    );
    let mut changed_key = config(&dir);
    changed_key.key = [8u8; 32];
    assert!(
        Checkpoint::load_with(changed_key, &p.base_url, &p, &headers())
            .await
            .unwrap()
            .1
            .is_none()
    );
}

#[tokio::test]
async fn http_checkpoint_rejects_corrupt_record_and_rolls_back_interrupted_transaction() {
    let dir = directory();
    let (mut checkpoint, _) = load(&dir, &payload()).await;
    checkpoint
        .save(Some(cookies("rotated").snapshot()))
        .await
        .unwrap();
    let mut db = local::open(&config(&dir)).await.unwrap();
    use sqlx::Connection;
    let mut tx = db.begin_with("BEGIN IMMEDIATE").await.unwrap();
    sqlx::query("UPDATE http_auth_checkpoint_v1 SET payload=zeroblob(80)")
        .execute(&mut *tx)
        .await
        .unwrap();
    drop(tx);
    db.close().await.unwrap();
    assert!(load(&dir, &payload()).await.1.is_some());
    let mut db = local::open(&config(&dir)).await.unwrap();
    sqlx::query("UPDATE http_auth_checkpoint_v1 SET payload=zeroblob(80)")
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    assert_eq!(
        Checkpoint::load_with(config(&dir), &payload().base_url, &payload(), &headers())
            .await
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("gemini_web_http_auth_checkpoint_invalid_record")
    );
}

#[tokio::test]
async fn http_checkpoint_capacity_is_bounded_and_missing_identity_fails_closed() {
    let dir = directory();
    let mut p = payload();
    p.credential_id = None;
    assert!(
        Checkpoint::load_with(config(&dir), &p.base_url, &p, &headers())
            .await
            .is_err()
    );
    assert!(!dir.exists());
    let (mut checkpoint, _) = load(&dir, &payload()).await;
    let mut db = local::open(&config(&dir)).await.unwrap();
    sqlx::query("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<1024) INSERT INTO http_auth_checkpoint_v1 SELECT 'synthetic-'||x, 'opaque-revision', zeroblob(34) FROM n")
        .execute(&mut db).await.unwrap();
    use sqlx::Connection;
    db.close().await.unwrap();
    assert_eq!(
        checkpoint
            .save(Some(cookies("rotated").snapshot()))
            .await
            .unwrap_err()
            .code
            .as_deref(),
        Some("gemini_web_http_auth_checkpoint_capacity")
    );
}
