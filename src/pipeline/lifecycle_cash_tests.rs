//! Real cash admission cancellation preserves NotSent versus uncertain usage without network calls.
use super::{test_support::*, Owner, Semaphore, Stage};
use crate::access_balance::AccessBalanceStore;
use serde_json::json;
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

#[test]
fn pipeline_lifecycle_cash_admission_unclaimed_outputs_keep_correct_durable_outcome() {
    run(cash_drop);
}
async fn cash_drop() {
    for attempted in [false, true] {
        for phase in [0, 1, 2] {
            if attempted && phase == 0 {
                continue;
            }
            let fixture = Fixture::with_quota(
                json!({"providers":[{
                    "id":"cash-fixture","base_url":"https://example.invalid/v1",
                    "api_key":"synthetic-only","supported_models":["synthetic-model"]
                }]}),
                "cash_prepaid",
                1_000_000,
            )
            .await;
            sqlx::query("INSERT INTO provider_pricing(provider_id,payload) VALUES(?,?)")
                .bind("cash-fixture")
                .bind(
                    json!({"staticInputMicrosPer1kTokens":100,"staticOutputMicrosPer1kTokens":200})
                        .to_string(),
                )
                .execute(&fixture.local.pool)
                .await
                .unwrap();
            let limit = Arc::new(Semaphore::new(1));
            let mut ctx = fixture.ctx();
            let id = ctx.req_id.to_string();
            let attempts = ctx.route_attempt_count.clone();
            let snapshot = fixture.console.state.route_config.snapshot();
            ctx.candidates = snapshot.resolve_candidates(Some("synthetic-model"));
            assert_eq!(ctx.candidates.len(), 1);
            assert_eq!(
                ctx.candidates[0].payload.canonical_adapter(),
                "openai_compatible"
            );
            ctx.cash_route_snapshot = Some(snapshot);
            let mut owner = Owner::begin(ctx, &fixture.console.state, limit.clone())
                .await
                .unwrap();
            for stage in [Stage::Audit, Stage::Filter] {
                let (next, result) = owner.step(stage).await.unwrap();
                owner = next;
                result.unwrap();
            }
            let pause = Arc::new(Pause::default());
            let returning = Arc::new(tokio::sync::Notify::new());
            if phase == 0 {
                owner.hooks.before = Some((Stage::Cash, pause.clone()));
            } else {
                owner.hooks.after = Some((Stage::Cash, pause.clone()));
            }
            owner.hooks.returning = Some(returning.clone());
            let mut pending: PinFuture<'_> = Box::pin(async {
                let (_, result) = owner.step(Stage::Cash).await.unwrap();
                result.unwrap();
                panic!("Unclaimed cash admission reached caller");
            });
            ready(&pause, &mut pending).await;
            let store = AccessBalanceStore::Sqlite(&fixture.local);
            if phase > 0 {
                let receipts = store.cash_ledger(&fixture.key).await.unwrap();
                assert_eq!(receipts.len(), 1);
                assert_eq!(receipts[0].request_id, id);
                assert_eq!(receipts[0].status, "reserved");
                assert!(receipts[0].reserved_micros > 0);
            }
            // Only simulate the attempt observation; this test never polls an upstream transport.
            attempts.store(u32::from(attempted), Ordering::Relaxed);
            if phase == 2 {
                pause.release();
                returning.notified().await;
            }
            drop(pending);
            if phase != 2 {
                pause.release();
            }
            let returned = tokio::time::timeout(Duration::from_secs(3), limit.acquire_owned())
                .await
                .unwrap()
                .unwrap();
            let receipt = store.cash_ledger(&fixture.key).await.unwrap().remove(0);
            assert_eq!(receipt.request_id, id);
            assert_eq!(
                receipt.status,
                if attempted { "unresolved" } else { "released" }
            );
            assert_eq!(
                receipt.reason.as_deref(),
                Some(if attempted {
                    "usage_unavailable"
                } else {
                    "not_sent"
                })
            );
            assert_eq!(receipt.amount_micros, (!attempted).then_some(0));
            let balance = store.cash_balance(&fixture.key).await.unwrap().unwrap();
            assert_eq!(balance.spent_micros, 0);
            assert_eq!(
                balance.reserved_micros,
                if attempted {
                    receipt.reserved_micros
                } else {
                    0
                }
            );
            assert_eq!(balance.pending_requests, u64::from(attempted));
            assert_eq!(
                fixture
                    .audit("cancelled")
                    .await
                    .route_trace
                    .as_ref()
                    .unwrap()["routeAttemptCount"]
                    .as_u64(),
                Some(u64::from(attempted))
            );
            drop(returned);
            fixture.close().await;
        }
    }
}
