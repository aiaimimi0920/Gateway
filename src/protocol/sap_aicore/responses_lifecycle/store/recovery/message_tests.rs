//! Missing usage does not erase a proven one-message charge or permit a token refund.
use super::super::{mutation::MutationKind, RoutePin, State, UpstreamId};
use super::*;
use crate::local_runtime::LocalRuntime;

struct Evidence {
    completed: Receipt,
    retained: Vec<Receipt>,
    messages: i64,
    tokens: i64,
}

async fn intent(store: &Store<'_>, key: &str, estimate: u64) -> Receipt {
    store
        .begin_reserved(
            &"d".repeat(64),
            &"e".repeat(64),
            "message-account",
            "message-credential",
            &RoutePin {
                public_model: "recovery-model".into(),
                source_key_ref: key.into(),
                platform_access_ref: matches!(store, Store::Hosted(_))
                    .then(|| "message-access".into()),
            },
            estimate,
        )
        .await
        .unwrap()
}

async fn observe(store: &Store<'_>, row: &Receipt, state: State) -> Receipt {
    let id = row
        .upstream_id
        .clone()
        .unwrap_or_else(|| format!("resp_{}", uuid::Uuid::new_v4().simple()));
    store
        .observe(row, &UpstreamId::parse(&id).unwrap(), state)
        .await
        .unwrap()
        .unwrap()
}

async fn messages(store: &Store<'_>, key: &str) -> i64 {
    match store {
        Store::Local(pool) => sqlx::query_scalar("SELECT json_extract(a.payload,'$.remainingMessages')
            FROM local_balance_accounts a JOIN local_balance_keys k ON k.account_id=a.id WHERE k.key_id=$1")
            .bind(key).fetch_one(*pool).await.unwrap(),
        Store::Hosted(pool) => sqlx::query_scalar("SELECT remaining_messages FROM gateway_access_key_balances WHERE access_key_id=$1")
            .bind(key).fetch_one(*pool).await.unwrap(),
    }
}

async fn seed(store: &Store<'_>, message_key: &str, token_key: &str) -> Evidence {
    let before = messages(store, message_key).await;
    let pending = observe(store, &intent(store, message_key, 10).await, State::Pending).await;
    assert!(store
        .claim_mutation(&pending, MutationKind::Cancel)
        .await
        .unwrap()
        .is_some());
    let completed = observe(store, &pending, State::Completed(None)).await;
    let unknown = intent(store, message_key, 10).await;
    let failed = observe(
        store,
        &intent(store, message_key, 10).await,
        State::Failed(None),
    )
    .await;
    let pending = observe(store, &intent(store, message_key, 10).await, State::Pending).await;
    let token = observe(
        store,
        &intent(store, token_key, 10).await,
        State::Completed(None),
    )
    .await;
    let zero = observe(
        store,
        &intent(store, token_key, 0).await,
        State::Completed(None),
    )
    .await;
    // Fault injection: even an inconsistent zero hold cannot broaden recovery.
    // Normal token reservation always charges at least one.
    run!(store, pool => sqlx::query(
        "UPDATE gateway_sap_response_balance_holds SET reserved=0 WHERE response_id=$1")
        .bind(&zero.id).execute(*pool).await.map(|_| ()))
    .unwrap();
    assert_eq!(messages(store, message_key).await, before - 4);
    Evidence {
        completed,
        retained: vec![unknown, failed, pending, token, zero],
        messages: before - 4,
        tokens: store.remaining_tokens(token_key).await,
    }
}

async fn assert_settled(store: &Store<'_>, row: &Receipt) {
    let current = store.read(&row.id, &row.owner_tag).await.unwrap().unwrap();
    assert_eq!(current.phase, "completed");
    assert_eq!(current.settled, 1);
    assert_eq!(current.revision, row.revision + 1);
    assert!(current.input_tokens.is_none() && current.output_tokens.is_none());
    let open: i64 = run!(store, pool => sqlx::query_scalar(
        "SELECT open FROM gateway_sap_response_balance_holds WHERE response_id=$1")
        .bind(&row.id).fetch_one(*pool).await)
    .unwrap();
    assert_eq!(open, 0);
    // Accounting may finish, but an uncertain cancel is still a mutation barrier.
    assert!(store
        .claim_mutation(&current, MutationKind::Delete)
        .await
        .unwrap()
        .is_none());
}

async fn exercise(
    writer: &Store<'_>,
    reader: &Store<'_>,
    message_key: &str,
    token_key: &str,
    evidence: &Evidence,
) {
    let rows = reader.recoverable("").await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, evidence.completed.id);
    let (a, b) = tokio::join!(writer.recover(&rows[0]), reader.recover(&rows[0]));
    assert_ne!(a.unwrap(), b.unwrap());
    assert!(!writer.recover(&rows[0]).await.unwrap());
    assert_settled(reader, &evidence.completed).await;
    assert_eq!(messages(reader, message_key).await, evidence.messages);
    assert_eq!(reader.remaining_tokens(token_key).await, evidence.tokens);
    assert!(reader.recoverable("").await.unwrap().is_empty());
    for row in &evidence.retained {
        if row.terminal() {
            // Direct dispatch cannot bypass the scan's exact no-usage exception.
            assert!(!reader.recover(row).await.unwrap());
        }
        let current = reader.read(&row.id, &row.owner_tag).await.unwrap().unwrap();
        assert_eq!(current.settled, 0);
        assert_eq!(current.phase, row.phase);
        assert!(current.input_tokens.is_none() && current.output_tokens.is_none());
        let open: i64 = run!(reader, pool => sqlx::query_scalar(
            "SELECT open FROM gateway_sap_response_balance_holds WHERE response_id=$1")
            .bind(&row.id).fetch_one(*pool).await)
        .unwrap();
        assert_eq!(open, 1);
    }
}

#[tokio::test]
async fn sap_aicore_message_recovery_survives_restart_and_concurrent_ledgers_without_usage() {
    let root = std::env::temp_dir().join(format!(
        "sap-message-recovery-{}",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let first = LocalRuntime::open(&root).await.unwrap();
    let message_key = tests::local_key(&first, "message_prepaid").await;
    let token_key = tests::local_key(&first, "token_prepaid").await;
    let evidence = seed(&Store::Local(&first.pool), &message_key, &token_key).await;
    first.close().await; // Durable completion exists, but settlement never ran.
    let writer = LocalRuntime::open(&root).await.unwrap();
    let reader = LocalRuntime::open(&root).await.unwrap();
    exercise(
        &Store::Local(&writer.pool),
        &Store::Local(&reader.pool),
        &message_key,
        &token_key,
        &evidence,
    )
    .await;
    writer.close().await;
    reader.close().await;
    let reopened = LocalRuntime::open(&root).await.unwrap();
    let store = Store::Local(&reopened.pool);
    assert_settled(&store, &evidence.completed).await;
    assert!(store.recoverable("").await.unwrap().is_empty());
    assert_eq!(messages(&store, &message_key).await, evidence.messages);
    assert_eq!(store.remaining_tokens(&token_key).await, evidence.tokens);
    reopened.close().await;
}

pub(in crate::protocol::sap_aicore::responses_lifecycle::store) async fn hosted(
    writer: &sqlx::PgPool,
    reader: &sqlx::PgPool,
) {
    // A distinct key avoids changing the mode of previous token fixtures' open holds.
    sqlx::raw_sql(
        "INSERT INTO gateway_access_keys (id,status) VALUES ('message-fixture','active');
        INSERT INTO gateway_access_key_balances
          (access_key_id,balance_mode,status,total_messages,remaining_messages,updated_at)
          VALUES ('message-fixture','message_prepaid','active',100,100,now());",
    )
    .execute(writer)
    .await
    .unwrap();
    let writer = Store::Hosted(writer);
    let reader = Store::Hosted(reader);
    let evidence = seed(&writer, "message-fixture", "fixture").await;
    exercise(&writer, &reader, "message-fixture", "fixture", &evidence).await;
}
