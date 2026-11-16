//! Storage-unit fixtures complement the real claim/public tests; no synthetic execution authority.
use super::*;
use crate::protocol::freebuff::catalog::run_journal::{RunPhase, RunTotals};

fn record() -> RunRecord {
    RunRecord {
        version: 1,
        client_id: "original-client".into(),
        run_id: Some("original-run".into()),
        phase: RunPhase::Started,
        totals: Some(RunTotals {
            total_steps: 0,
            direct_credits: 0,
            total_credits: 0,
        }),
        prior_totals: None,
        last_step: None,
        last_error: None,
        finish_status: None,
        finish_http_status: None,
    }
}
async fn setup() -> (std::path::PathBuf, LocalRuntime, RunStore) {
    let root = std::env::temp_dir().join(format!(
        "gateway-local-freebuff-run-{}",
        uuid::Uuid::new_v4()
    ));
    let db = LocalRuntime::open(&root).await.unwrap();
    let store = RunStore {
        runtime: db.clone(),
        binding: "synthetic-storage-binding".into(),
        attempt: uuid::Uuid::new_v4().to_string(),
        claim_sealed: "synthetic-claim-preimage".into(),
        sealed: None,
    };
    sqlx::query(
        "INSERT INTO local_freebuff_session_claims(binding,attempt_id,sealed) VALUES (?,?,?)",
    )
    .bind(&store.binding)
    .bind(&store.attempt)
    .bind(&store.claim_sealed)
    .execute(&db.pool)
    .await
    .unwrap();
    (root, db, store)
}
#[tokio::test]
async fn freebuff_run_store_seals_original_ids_and_preserves_them_on_reopen() {
    let (root, db, mut store) = setup().await;
    store.save_record(&record()).await.unwrap();
    let sealed = store.sealed.clone().unwrap();
    for secret in ["original-run", "original-client"] {
        assert!(!sealed.contains(secret));
    }
    let seal = std::fs::read(root.join("access-key-seal.json")).unwrap();
    db.close().await;
    let db = LocalRuntime::open(&root).await.unwrap();
    let bytes = db
        .secret_seal
        .open_bytes(
            run_aad(&store.binding, &store.attempt).as_bytes(),
            &sealed,
            32768,
            16384,
        )
        .unwrap();
    let restored: RunRecord = serde_json::from_slice(&bytes).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored.run_id.as_deref(), Some("original-run"));
    assert_eq!(restored.client_id, "original-client");
    assert_eq!(
        std::fs::read(root.join("access-key-seal.json")).unwrap(),
        seal
    );
    assert!(db
        .secret_seal
        .open_bytes(
            run_aad("foreign-binding", &store.attempt).as_bytes(),
            &sealed,
            32768,
            16384
        )
        .is_err());
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
#[tokio::test]
async fn freebuff_run_store_cas_rejects_stale_state_and_changed_original_claim() {
    let (root, db, mut store) = setup().await;
    store.save_record(&record()).await.unwrap();
    let mut stale = RunStore {
        runtime: db.clone(),
        binding: store.binding.clone(),
        attempt: store.attempt.clone(),
        claim_sealed: store.claim_sealed.clone(),
        sealed: store.sealed.clone(),
    };
    let mut next = record();
    next.phase = RunPhase::ModelPending;
    next.totals = None;
    store.save_record(&next).await.unwrap();
    assert!(stale.save_record(&record()).await.is_err());
    sqlx::query("UPDATE local_freebuff_session_claims SET sealed='changed'")
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(store.save_record(&record()).await.is_err());
    let kept: String = sqlx::query_scalar("SELECT sealed FROM local_freebuff_runs")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(Some(kept), store.sealed);
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
#[tokio::test]
async fn freebuff_run_store_only_rows_prevent_replacing_a_missing_seal() {
    let root = std::env::temp_dir().join(format!(
        "gateway-local-freebuff-run-seal-{}",
        uuid::Uuid::new_v4()
    ));
    let db = LocalRuntime::open(&root).await.unwrap();
    sqlx::query("INSERT INTO local_freebuff_runs(binding,attempt_id,claim_sealed,sealed) VALUES ('synthetic','original','claim','unresolved')").execute(&db.pool).await.unwrap();
    db.close().await;
    std::fs::rename(
        root.join("access-key-seal.json"),
        root.join("saved-seal.json"),
    )
    .unwrap();
    let db = LocalRuntime::open(&root).await.unwrap();
    assert!(!root.join("access-key-seal.json").exists());
    assert!(db.secret_seal.seal(b"test", b"test", 16).is_err());
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
