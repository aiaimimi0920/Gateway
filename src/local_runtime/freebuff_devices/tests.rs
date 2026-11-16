//! Real isolated SQLite/seal and OS-lock proofs; no upstream or live credential access.
use super::*;
use crate::protocol::freebuff::catalog::{BusinessOrigin, ClientSurface};
use registration::{RegistrationAttempt, RegistrationPreparation as Prepared};
use serde_json::json;
#[path = "recovery_tests.rs"]
mod recovery_tests;

async fn setup() -> (std::path::PathBuf, LocalRuntime) {
    let root = std::env::temp_dir().join(format!(
        "gateway-local-freebuff-device-{}",
        uuid::Uuid::new_v4()
    ));
    let db = LocalRuntime::open(&root).await.unwrap();
    (root, db)
}
fn session() -> CatalogSession {
    CatalogSession::new(
        BusinessOrigin::Codebuff,
        "synthetic-account",
        ClientSurface::Cli,
    )
    .unwrap()
}
async fn prepare(db: &LocalRuntime, session: &CatalogSession) -> RegistrationAttempt {
    match db
        .prepare_freebuff_registration("pool", "account", session, RegistrationIntent::Register)
        .await
        .unwrap()
    {
        Prepared::Send(attempt) => attempt,
        _ => panic!("expected new attempt"),
    }
}
fn capability(session: &CatalogSession, key: Arc<DeviceKey>) -> DeviceRegistration {
    let receipt = json!({"version":1,"scope":session.registration_scope(),"public_key":key.public_key(),"key_id":"synthetic-key-id"});
    session
        .restore_registration(key, &serde_json::to_vec(&receipt).unwrap())
        .unwrap()
}
async fn key_row(db: &LocalRuntime) -> (String, String) {
    sqlx::query_as("SELECT public_key,sealed FROM local_freebuff_device_keys")
        .fetch_one(&db.pool)
        .await
        .unwrap()
}
#[tokio::test]
async fn freebuff_device_only_missing_seal_fails_closed_and_original_recovers() {
    let (root, db) = setup().await;
    let session = session();
    drop(prepare(&db, &session).await);
    let row = key_row(&db).await;
    let path = root.join("access-key-seal.json");
    let seal = std::fs::read(&path).unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM local_access_key_secrets")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    db.close().await;
    std::fs::remove_file(&path).unwrap();
    let db = LocalRuntime::open(&root).await.unwrap();
    let result = db
        .prepare_freebuff_registration("pool", "new", &session, RegistrationIntent::Register)
        .await;
    assert_eq!(
        result.err().unwrap().code.as_deref(),
        Some("freebuff_device_secret_unavailable")
    );
    assert!(!path.exists());
    assert_eq!(key_row(&db).await, row);
    db.close().await;
    std::fs::write(&path, seal).unwrap();
    let db = LocalRuntime::open(&root).await.unwrap();
    let next = db
        .prepare_freebuff_registration("pool", "new", &session, RegistrationIntent::Register)
        .await
        .unwrap();
    let Prepared::Send(next) = next else {
        panic!("restored key can register a different exact account")
    };
    assert_eq!(next.key().public_key(), row.0);
    drop(next);
    assert_eq!(key_row(&db).await, row);
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
#[tokio::test]
async fn freebuff_device_two_instances_share_single_key_and_os_registration_fence() {
    let (root, db) = setup().await;
    let other = LocalRuntime::open(&root).await.unwrap();
    async fn create(db: &LocalRuntime) -> String {
        let mut tx = db.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        let key = device_key(&mut tx, &db.secret_seal, true)
            .await
            .unwrap()
            .unwrap();
        tx.commit().await.unwrap();
        key.public_key()
    }
    let (a, b) = tokio::join!(create(&db), create(&other));
    assert_eq!(a, b);
    assert!(sqlx::query(
        "INSERT INTO local_freebuff_device_keys(id,public_key,sealed) VALUES (2,'other','bad')"
    )
    .execute(&db.pool)
    .await
    .is_err());
    let session = session();
    let first = prepare(&db, &session).await;
    assert_eq!(
        other
            .prepare_freebuff_registration(
                "pool",
                "another",
                &session,
                RegistrationIntent::Register
            )
            .await
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("freebuff_device_registration_busy")
    );
    // Dropping the request releases only the kernel lock; it does not erase its durable unknown marker.
    drop(first);
    let Prepared::Existing(RegistrationState::Unconfirmed(id)) = other
        .prepare_freebuff_registration("pool", "account", &session, RegistrationIntent::Register)
        .await
        .unwrap()
    else {
        panic!("marker must survive cancellation")
    };
    let Prepared::Send(retry) = other
        .prepare_freebuff_registration("pool", "account", &session, RegistrationIntent::Retry(&id))
        .await
        .unwrap()
    else {
        panic!("explicit retry")
    };
    assert_eq!(retry.key().public_key(), a);
    drop(retry);
    assert_eq!(
        db.prepare_freebuff_registration(
            "pool",
            "account",
            &session,
            RegistrationIntent::Retry(&id)
        )
        .await
        .err()
        .unwrap()
        .code
        .as_deref(),
        Some("freebuff_device_registration_attempt_changed")
    );
    other.close().await;
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
#[tokio::test]
async fn freebuff_device_receipt_is_exactly_bound_and_swaps_never_fall_back() {
    let (root, db) = setup().await;
    let s = session();
    let attempt = prepare(&db, &s).await;
    let cap = capability(&s, attempt.key());
    attempt.complete(&s, &cap).await.unwrap();
    for (provider, credential, origin, token, client) in [
        (
            "other",
            "account",
            BusinessOrigin::Codebuff,
            "synthetic-account",
            ClientSurface::Cli,
        ),
        (
            "pool",
            "other",
            BusinessOrigin::Codebuff,
            "synthetic-account",
            ClientSurface::Cli,
        ),
        (
            "pool",
            "account",
            BusinessOrigin::CodebuffWww,
            "synthetic-account",
            ClientSurface::Cli,
        ),
        (
            "pool",
            "account",
            BusinessOrigin::Codebuff,
            "other-token",
            ClientSurface::Cli,
        ),
        (
            "pool",
            "account",
            BusinessOrigin::Codebuff,
            "synthetic-account",
            ClientSurface::Desktop,
        ),
    ] {
        let foreign = CatalogSession::new(origin, token, client).unwrap();
        assert!(db
            .freebuff_registration(provider, credential, &foreign)
            .await
            .unwrap()
            .is_none());
    }
    assert!(db
        .freebuff_registration("pool", "account", &s)
        .await
        .unwrap()
        .is_some());
    let (public, private) = key_row(&db).await;
    let receipt: String = sqlx::query_scalar("SELECT sealed FROM local_freebuff_registrations")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert!(!receipt.contains("synthetic-key-id"));
    sqlx::query("UPDATE local_freebuff_registrations SET sealed=?")
        .bind(&private)
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(db
        .freebuff_registration("pool", "account", &s)
        .await
        .is_err());
    assert!(db
        .prepare_freebuff_registration("pool", "account", &s, RegistrationIntent::Register)
        .await
        .is_err());
    sqlx::query("UPDATE local_freebuff_registrations SET sealed=?")
        .bind(&receipt)
        .execute(&db.pool)
        .await
        .unwrap();
    let foreign_aad = format!("gateway-access-key:v1:{public}:unused");
    let unrelated = db
        .secret_seal
        .seal(foreign_aad.as_bytes(), b"synthetic-token", 512)
        .unwrap();
    sqlx::query("UPDATE local_freebuff_device_keys SET sealed=?")
        .bind(unrelated)
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(db
        .freebuff_registration("pool", "account", &s)
        .await
        .is_err());
    assert_eq!(key_row(&db).await.0, public);
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
#[tokio::test]
async fn freebuff_device_foreign_completion_and_failed_save_preserve_unconfirmed_marker() {
    let (root, db) = setup().await;
    let s = session();
    let attempt = prepare(&db, &s).await;
    let foreign =
        CatalogSession::new(BusinessOrigin::Codebuff, "new-token", ClientSurface::Cli).unwrap();
    let cap = capability(&foreign, attempt.key());
    assert!(attempt.complete(&foreign, &cap).await.is_err());
    let Prepared::Existing(RegistrationState::Unconfirmed(id)) = db
        .prepare_freebuff_registration("pool", "account", &s, RegistrationIntent::Register)
        .await
        .unwrap()
    else {
        panic!("must remain unknown")
    };
    let Prepared::Send(attempt) = db
        .prepare_freebuff_registration("pool", "account", &s, RegistrationIntent::Retry(&id))
        .await
        .unwrap()
    else {
        panic!("explicit retry")
    };
    let cap = capability(&s, attempt.key());
    sqlx::raw_sql("CREATE TRIGGER reject_device_save BEFORE UPDATE OF sealed ON local_freebuff_registrations WHEN NEW.sealed IS NOT NULL BEGIN SELECT RAISE(ABORT, 'synthetic save failure'); END;").execute(&db.pool).await.unwrap();
    assert!(attempt.complete(&s, &cap).await.is_err());
    assert!(db
        .freebuff_registration("pool", "account", &s)
        .await
        .unwrap()
        .is_none());
    assert!(matches!(
        db.prepare_freebuff_registration("pool", "account", &s, RegistrationIntent::Register)
            .await
            .unwrap(),
        Prepared::Existing(RegistrationState::Unconfirmed(_))
    ));
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
