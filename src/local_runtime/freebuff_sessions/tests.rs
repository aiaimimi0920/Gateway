//! Isolated real SQLite startup and claim recovery; never reads live credentials.
use super::super::LocalRuntime;

#[tokio::test]
async fn freebuff_session_journal_is_initialized_without_rewriting_v1() {
    let root = std::env::temp_dir().join(format!(
        "gateway-local-freebuff-session-{}",
        uuid::Uuid::new_v4()
    ));
    let db = LocalRuntime::open(&root).await.unwrap();
    // Simulate an existing schema-1 database; only this isolated fixture's empty additive table is removed.
    sqlx::query("INSERT INTO provider_pricing(provider_id,payload) VALUES ('synthetic-keep','{\"keep\":true}')").execute(&db.pool).await.unwrap();
    let seal = std::fs::read(root.join("access-key-seal.json")).unwrap();
    sqlx::query("DROP TABLE IF EXISTS local_freebuff_session_claims")
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS local_freebuff_runs")
        .execute(&db.pool)
        .await
        .unwrap();
    db.close().await;
    let db = LocalRuntime::open(&root).await.unwrap();
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    let table: i64 = sqlx::query_scalar("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='local_freebuff_session_claims'").fetch_one(&db.pool).await.unwrap();
    let kept: String = sqlx::query_scalar(
        "SELECT payload FROM provider_pricing WHERE provider_id='synthetic-keep'",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(kept, "{\"keep\":true}");
    let runs: i64 = sqlx::query_scalar("SELECT count(*) FROM local_freebuff_runs")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(runs, 0, "old claims have no invented run or fee facts");
    assert_eq!(
        std::fs::read(root.join("access-key-seal.json")).unwrap(),
        seal
    );
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
    assert_eq!(version, 1);
    assert_eq!(
        table, 1,
        "session claims must be durable before a possible purchase"
    );
}
