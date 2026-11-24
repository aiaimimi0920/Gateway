//! Bounded cursor progress, bad-row isolation, durable proof denial and drain boundaries.
use super::*;
use crate::protocol::sap_aicore::responses_lifecycle::store::{recovery, RoutePin, Store};

#[tokio::test]
async fn sap_aicore_recovery_worker_pages_past_bad_rows_and_stops_on_drain() {
    Box::pin(async {
        let wire = Arc::new(Wire::default());
        let (_, fixture, server) = prepare(wire.clone()).await;
        let local = fixture.state.local_runtime.as_ref().unwrap();
        let key: String = sqlx::query_scalar("SELECT key_id FROM local_balance_keys")
            .fetch_one(&local.pool).await.unwrap();
        sqlx::query("UPDATE local_balance_accounts SET payload=json_set(payload,
            '$.remainingMessages',100,'$.totalMessages',100)")
            .execute(&local.pool).await.unwrap();
        let store = Store::Local(&local.pool);
        let pin = RoutePin { public_model: MODEL.into(), source_key_ref: key.clone(), platform_access_ref: None };
        let mut ids = Vec::new();
        for _ in 0..34 {
            let row = store.begin_reserved(&"a".repeat(64), &"b".repeat(64), "pool", "credential", &pin, 1)
                .await.unwrap();
            store.record_unsent(&row).await.unwrap();
            ids.push(row.id);
        }
        ids.sort();
        // Corrupt only the fixture's first mode: the worker must retain that hold and
        // report a conflict rather than guessing an account type or starving later work.
        sqlx::query("UPDATE gateway_sap_response_balance_holds SET mode='token_prepaid' WHERE response_id=$1")
            .bind(&ids[0]).execute(&local.pool).await.unwrap();
        let mut cursor = String::new();
        recovery::tick(&fixture.state, &mut cursor).await.unwrap();
        assert_eq!(cursor, ids[31]);
        assert_eq!(fixture.remaining().await, 97);
        recovery::tick(&fixture.state, &mut cursor).await.unwrap();
        assert!(cursor.is_empty());
        assert_eq!(fixture.remaining().await, 99);
        recovery::tick(&fixture.state, &mut cursor).await.unwrap();
        assert_eq!(fixture.remaining().await, 99);
        // Restoring the correct balance mode is insufficient after drain: no new work.
        sqlx::query("UPDATE gateway_sap_response_balance_holds SET mode='message_prepaid' WHERE response_id=$1")
            .bind(&ids[0]).execute(&local.pool).await.unwrap();
        fixture.state.lifecycle.begin_drain("fixture");
        recovery::tick(&fixture.state, &mut cursor).await.unwrap();
        assert_eq!(fixture.remaining().await, 99);
        assert!(wire.requests.lock().unwrap().is_empty());
        fixture.finish().await;
        server.abort();
        let _ = server.await;
    }).await;
}

#[tokio::test]
async fn sap_aicore_recovery_proof_failure_does_not_infer_refund_permission() {
    Box::pin(async {
        let wire = Arc::new(Wire::default());
        let (_, fixture, server) = prepare(wire.clone()).await;
        let pool = &fixture.state.local_runtime.as_ref().unwrap().pool;
        let key: String = sqlx::query_scalar("SELECT key_id FROM local_balance_keys")
            .fetch_one(pool)
            .await
            .unwrap();
        let store = Store::Local(pool);
        let row = store
            .begin_reserved(
                &"a".repeat(64),
                &"b".repeat(64),
                "pool",
                "credential",
                &RoutePin {
                    public_model: MODEL.into(),
                    source_key_ref: key,
                    platform_access_ref: None,
                },
                1,
            )
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TRIGGER deny_proof BEFORE INSERT ON gateway_sap_response_unsent_proofs
            BEGIN SELECT RAISE(ABORT,'proof unavailable'); END;
            CREATE TRIGGER deny_refund BEFORE UPDATE OF open ON gateway_sap_response_balance_holds
            BEGIN SELECT RAISE(ABORT,'refund unavailable'); END;",
        )
        .execute(pool)
        .await
        .unwrap();
        assert!(store.refund_unsent(&row).await.is_err());
        sqlx::raw_sql("DROP TRIGGER deny_proof; DROP TRIGGER deny_refund")
            .execute(pool)
            .await
            .unwrap();
        let mut cursor = String::new();
        recovery::tick(&fixture.state, &mut cursor).await.unwrap();
        assert_eq!(fixture.remaining().await, 1);
        assert!(store.refund_proven_unsent(&row).await.is_err());
        assert!(wire.requests.lock().unwrap().is_empty());
        fixture.finish().await;
        server.abort();
        let _ = server.await;
    })
    .await;
}
