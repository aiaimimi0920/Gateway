//! Replacement overlays retain the last known receipt until atomic promotion, without implicit fallback.
use super::*;

async fn stored(db: &LocalRuntime, s: &CatalogSession) -> String {
    let attempt = prepare(db, s).await;
    let cap = capability(s, attempt.key());
    attempt.complete(s, &cap).await.unwrap()
}
async fn row(db: &LocalRuntime) -> (String, String, String) {
    sqlx::query_as("SELECT public_key,attempt_id,sealed FROM local_freebuff_registrations")
        .fetch_one(&db.pool)
        .await
        .unwrap()
}
async fn state(db: &LocalRuntime, s: &CatalogSession) -> RegistrationState {
    db.freebuff_registration_state("pool", "account", s)
        .await
        .unwrap()
}
async fn attempt(
    db: &LocalRuntime,
    s: &CatalogSession,
    intent: RegistrationIntent<'_>,
) -> RegistrationAttempt {
    let Prepared::Send(attempt) = db
        .prepare_freebuff_registration("pool", "account", s, intent)
        .await
        .unwrap()
    else {
        panic!("expected explicit attempt")
    };
    attempt
}
#[tokio::test]
async fn freebuff_device_state_absence_and_stale_acknowledgments_never_generate_key() {
    let (root, db) = setup().await;
    let s = session();
    assert_eq!(state(&db, &s).await, RegistrationState::Absent);
    let id = uuid::Uuid::new_v4().to_string();
    for intent in [
        RegistrationIntent::Retry(&id),
        RegistrationIntent::Replace(&id),
    ] {
        assert_eq!(
            db.prepare_freebuff_registration("pool", "account", &s, intent)
                .await
                .err()
                .unwrap()
                .code
                .as_deref(),
            Some("freebuff_device_registration_attempt_changed")
        );
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM local_freebuff_device_keys")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
#[tokio::test]
async fn freebuff_device_replacement_preserves_ready_across_rebuild_and_consumes_exact_confirmation(
) {
    let (root, db) = setup().await;
    let s = session();
    let id = stored(&db, &s).await;
    let original = row(&db).await;
    assert_eq!(state(&db, &s).await, RegistrationState::Stored(id.clone()));
    let replacement = attempt(&db, &s, RegistrationIntent::Replace(&id)).await;
    assert_eq!(replacement.key().public_key(), original.0);
    assert_eq!(row(&db).await, original);
    let RegistrationState::Unconfirmed(first) = state(&db, &s).await else {
        panic!("pending replacement")
    };
    assert_ne!(first, id);
    let other = LocalRuntime::open(&root).await.unwrap();
    assert_eq!(
        other
            .prepare_freebuff_registration("pool", "account", &s, RegistrationIntent::Retry(&first))
            .await
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("freebuff_device_registration_busy")
    );
    drop(replacement);
    other.close().await;
    db.close().await;
    let db = LocalRuntime::open(&root).await.unwrap();
    assert_eq!(
        state(&db, &s).await,
        RegistrationState::Unconfirmed(first.clone())
    );
    assert_eq!(
        db.freebuff_registration("pool", "account", &s)
            .await
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("freebuff_device_registration_unconfirmed")
    );
    assert!(db
        .prepare_freebuff_registration("pool", "account", &s, RegistrationIntent::Replace(&id))
        .await
        .is_err());
    let retry = attempt(&db, &s, RegistrationIntent::Retry(&first)).await;
    let RegistrationState::Unconfirmed(second) = state(&db, &s).await else {
        panic!("new marker")
    };
    assert_ne!(first, second);
    assert_eq!(row(&db).await, original);
    drop(retry);
    assert!(db
        .prepare_freebuff_registration("pool", "account", &s, RegistrationIntent::Retry(&first))
        .await
        .is_err());
    let retry = attempt(&db, &s, RegistrationIntent::Retry(&second)).await;
    let cap = capability(&s, retry.key());
    let new_id = retry.complete(&s, &cap).await.unwrap();
    assert_ne!(new_id, id);
    assert_eq!(
        state(&db, &s).await,
        RegistrationState::Stored(new_id.clone())
    );
    assert!(db
        .freebuff_registration("pool", "account", &s)
        .await
        .unwrap()
        .is_some());
    let new = row(&db).await;
    assert_eq!(new.0, original.0);
    assert_eq!(new.1, new_id);
    assert_ne!(new.2, original.2);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM local_freebuff_registration_replacements")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
#[tokio::test]
async fn freebuff_device_replacement_failed_promotion_retains_previous_receipt_and_marker() {
    let (root, db) = setup().await;
    let s = session();
    let id = stored(&db, &s).await;
    let original = row(&db).await;
    let replacement = attempt(&db, &s, RegistrationIntent::Replace(&id)).await;
    let unknown = state(&db, &s).await;
    let cap = capability(&s, replacement.key());
    sqlx::raw_sql("CREATE TRIGGER reject_replacement BEFORE DELETE ON local_freebuff_registration_replacements BEGIN SELECT RAISE(ABORT, 'synthetic promotion failure'); END;")
        .execute(&db.pool).await.unwrap();
    assert!(replacement.complete(&s, &cap).await.is_err());
    assert_eq!(row(&db).await, original);
    assert_eq!(state(&db, &s).await, unknown);
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
#[tokio::test]
async fn freebuff_device_replacement_stale_completion_and_damaged_receipt_cannot_overwrite() {
    let (root, db) = setup().await;
    let s = session();
    let id = stored(&db, &s).await;
    let original = row(&db).await;
    let replacement = attempt(&db, &s, RegistrationIntent::Replace(&id)).await;
    let cap = capability(&s, replacement.key());
    let changed = uuid::Uuid::new_v4().to_string();
    sqlx::query("UPDATE local_freebuff_registration_replacements SET attempt_id=?")
        .bind(&changed)
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(replacement.complete(&s, &cap).await.is_err());
    assert_eq!(row(&db).await, original);
    sqlx::query("UPDATE local_freebuff_registrations SET sealed='damaged'")
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(db
        .freebuff_registration_state("pool", "account", &s)
        .await
        .is_err());
    assert!(db
        .prepare_freebuff_registration("pool", "account", &s, RegistrationIntent::Retry(&changed))
        .await
        .is_err());
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
