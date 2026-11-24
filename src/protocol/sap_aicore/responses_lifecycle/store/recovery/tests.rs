use super::super::{mutation::MutationKind, RoutePin, State, UpstreamId};
use super::*;
use crate::{local_runtime::LocalRuntime, protocol::canonical::TokenUsage};

fn pin(key: &str, hosted: bool) -> RoutePin {
    RoutePin {
        public_model: "recovery-model".into(),
        source_key_ref: key.into(),
        platform_access_ref: hosted.then(|| "recovery-access".into()),
    }
}

async fn intent(store: &Store<'_>, key: &str) -> Receipt {
    store
        .begin_reserved(
            &"d".repeat(64),
            &"e".repeat(64),
            "recovery-account",
            "recovery-credential",
            &pin(key, matches!(store, Store::Hosted(_))),
            10,
        )
        .await
        .unwrap()
}

async fn terminal(store: &Store<'_>, row: &Receipt, amount: Option<u64>) -> Receipt {
    let usage = amount.map(|total| TokenUsage {
        prompt_tokens: total,
        completion_tokens: 0,
        total_tokens: total,
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
    });
    store
        .observe(
            row,
            &UpstreamId::parse(&format!("resp_{}", uuid::Uuid::new_v4().simple())).unwrap(),
            State::Failed(usage),
        )
        .await
        .unwrap()
        .unwrap()
}

pub(in crate::protocol::sap_aicore::responses_lifecycle::store) async fn exercise(
    writer: Store<'_>,
    reader: Store<'_>,
    key: &str,
) {
    let before = writer.remaining_tokens(key).await;
    let unsent = intent(&writer, key).await;
    let unknown = intent(&writer, key).await;
    let missing = terminal(&writer, &intent(&writer, key).await, None).await;
    let known = terminal(&writer, &intent(&writer, key).await, Some(5)).await;
    let zero = terminal(&writer, &intent(&writer, key).await, Some(0)).await;
    let pending_intent = intent(&writer, key).await;
    let pending = writer
        .observe(
            &pending_intent,
            &UpstreamId::parse(&format!("resp_{}", uuid::Uuid::new_v4().simple())).unwrap(),
            State::Pending,
        )
        .await
        .unwrap()
        .unwrap();
    let claim = writer
        .claim_mutation(&pending, MutationKind::Cancel)
        .await
        .unwrap();
    assert!(claim.is_some());
    let barrier = writer
        .observe(
            &pending,
            &UpstreamId::parse(pending.upstream_id.as_deref().unwrap()).unwrap(),
            State::Completed(Some(TokenUsage {
                prompt_tokens: 2,
                completion_tokens: 3,
                total_tokens: 5,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            })),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(writer.record_unsent(&known).await.is_err());
    assert!(writer.refund_proven_unsent(&unknown).await.is_err());
    writer.record_unsent(&unsent).await.unwrap();
    writer.record_unsent(&unsent).await.unwrap();
    let rows = reader.recoverable("").await.unwrap();
    assert_eq!(rows.len(), 4);
    assert!(!rows
        .iter()
        .any(|r| r.id == unknown.id || r.id == missing.id));
    for row in &rows {
        let (a, b) = tokio::join!(writer.recover(row), reader.recover(row));
        assert_ne!(a.unwrap(), b.unwrap());
    }
    assert_eq!(reader.remaining_tokens(key).await, before - 30);
    writer.refund_unsent(&unsent).await.unwrap();
    assert!(reader.recoverable("").await.unwrap().is_empty());
    let racing = intent(&writer, key).await;
    writer.record_unsent(&racing).await.unwrap();
    let (live, background) = tokio::join!(writer.refund_unsent(&racing), reader.recover(&racing));
    live.unwrap();
    background.unwrap();
    assert!(!reader.recover(&racing).await.unwrap());
    assert_eq!(reader.remaining_tokens(key).await, before - 30);
    let current = reader
        .read(&barrier.id, &barrier.owner_tag)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.settled, 1);
    assert!(reader
        .claim_mutation(&current, MutationKind::Delete)
        .await
        .unwrap()
        .is_none());
    let unknown = reader
        .read(&unknown.id, &unknown.owner_tag)
        .await
        .unwrap()
        .unwrap();
    let missing = reader
        .read(&missing.id, &missing.owner_tag)
        .await
        .unwrap()
        .unwrap();
    assert_eq!((unknown.phase.as_str(), unknown.settled), ("submitting", 0));
    assert!(unknown.upstream_id.is_none());
    assert_eq!(missing.settled, 0);
    assert!(missing.input_tokens.is_none());
    // Closed proven-unsent work is not new send permission and has no invented SAP terminal.
    let closed = reader
        .read(&unsent.id, &unsent.owner_tag)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (closed.phase.as_str(), closed.settled, closed.revision),
        ("submitting", 0, 0)
    );
    assert!(!reader.reserve_balance(&closed, key, 10).await.unwrap());
    assert_eq!(reader.remaining_tokens(key).await, before - 30);
    assert_eq!(
        reader
            .read(&known.id, &known.owner_tag)
            .await
            .unwrap()
            .unwrap()
            .settled,
        1
    );
    assert_eq!(
        reader
            .read(&zero.id, &zero.owner_tag)
            .await
            .unwrap()
            .unwrap()
            .settled,
        1
    );
    // Real database lock contention, not an immediate SQL error: cancellation must
    // leave a reusable pool and a durable hold that can be finalized exactly once.
    let locked = intent(&writer, key).await;
    writer.record_unsent(&locked).await.unwrap();
    let mut guard = writer.ledger().await.unwrap();
    guard.lock_receipt(&locked).await.unwrap();
    let timeout = worker::bounded(reader.recover(&locked)).await.unwrap_err();
    guard.commit().await.unwrap();
    assert_eq!(
        timeout.code.as_deref(),
        Some("sap_aicore_response_recovery_timeout")
    );
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        assert!(reader.recover(&locked).await.unwrap());
        assert!(!writer.recover(&locked).await.unwrap());
        assert_eq!(reader.remaining_tokens(key).await, before - 30);
    })
    .await
    .expect("cancelled database work must release its connection");
}

pub(super) async fn local_key(local: &LocalRuntime, mode: &str) -> String {
    local
        .save_access_key_with_quota(
            None,
            crate::db::UpsertAccessKeyInput {
                owner_type: "user".into(),
                owner_id: "recovery-fixture".into(),
                resolved_project_id: "local".into(),
                resolved_tenant_id: "local".into(),
                key_kind: "normal".into(),
                public_key_prefix: "sk-gw".into(),
                display_name: "Recovery fixture".into(),
                expires_at: None,
                bundle_ids: vec![],
                metadata: None,
            },
            Some(&crate::access_balance::quota::KeyQuotaInput {
                mode: mode.into(),
                limit: Some(100),
                currency: None,
            }),
        )
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn sap_aicore_recovery_sqlite_additive_restart_and_concurrent_ledgers() {
    let root = std::env::temp_dir().join(format!("sap-recovery-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&root).unwrap();
    let old = LocalRuntime::open(&root).await.unwrap();
    let key = local_key(&old, "token_prepaid").await;
    let store = Store::Local(&old.pool);
    let legacy = intent(&store, &key).await;
    // Simulate the old schema; historical unknowns must not acquire inferred proof.
    sqlx::raw_sql("DROP TABLE gateway_sap_response_unsent_proofs")
        .execute(&old.pool)
        .await
        .unwrap();
    assert!(store.recoverable("").await.is_err());
    old.close().await;
    let first = LocalRuntime::open(&root).await.unwrap();
    let store = Store::Local(&first.pool);
    let proven = intent(&store, &key).await;
    store.record_unsent(&proven).await.unwrap();
    first.close().await; // Crash equivalent between proof and refund, no held SQL transaction.
    let writer = LocalRuntime::open(&root).await.unwrap();
    let reader = LocalRuntime::open(&root).await.unwrap();
    let a = Store::Local(&writer.pool);
    let b = Store::Local(&reader.pool);
    assert_eq!(b.recoverable("").await.unwrap().len(), 1);
    let (x, y) = tokio::join!(a.recover(&proven), b.recover(&proven));
    assert_ne!(x.unwrap(), y.unwrap());
    assert_eq!(b.remaining_tokens(&key).await, 90);
    assert!(b.recoverable("").await.unwrap().is_empty());
    assert!(b
        .read(&legacy.id, &legacy.owner_tag)
        .await
        .unwrap()
        .unwrap()
        .upstream_id
        .is_none());
    exercise(a, b, &key).await;
    writer.close().await;
    reader.close().await;
    let reopened = LocalRuntime::open(&root).await.unwrap();
    assert!(Store::Local(&reopened.pool)
        .recoverable("")
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        Store::Local(&reopened.pool).remaining_tokens(&key).await,
        60
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA user_version")
            .fetch_one(&reopened.pool)
            .await
            .unwrap(),
        1
    );
    reopened.close().await;
}
