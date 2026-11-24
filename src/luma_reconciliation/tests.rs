//! Normal public generation, SQLite audit finalization and bounded read-only reconciliation.
use super::*;
use crate::upstream::luma_agents::{
    pipeline_fixture::{self, Fixture, IMAGE},
    test_support::*,
};

async fn completed(f: &Fixture) -> crate::db::GatewayRequestAuditView {
    let response = Box::pin(f.open("images/generations", pipeline_fixture::body(IMAGE))).await;
    assert_eq!(response.status(), 200);
    pipeline_fixture::drain(response.into_body()).await;
    f.audit().await
}

async fn row(f: &Fixture, id: &str) -> Value {
    let raw: String = sqlx::query_scalar("SELECT payload FROM request_audits WHERE id = ?")
        .bind(id)
        .fetch_one(&f.state.local_runtime.as_ref().unwrap().pool)
        .await
        .unwrap();
    serde_json::from_str(&raw).unwrap()
}

#[tokio::test]
async fn luma_reconciliation_backfills_exact_actual_credits_without_changing_outcome_or_balance() {
    for credits in [json!(0), json!(12.5)] {
        let mut latest = job("completed");
        latest["usage"] = json!({"credits_charged":credits});
        let server = Server::start(vec![
            Reply::json(201, job("completed")),
            Reply::png(),
            Reply::json(200, latest),
        ])
        .await;
        within(
            &server.origin,
            Box::pin(async {
                let f = Fixture::new(&server, 5).await;
                let audit = completed(&f).await;
                let store = store::Store::from_state(&f.state).unwrap();
                let before = row(&f, &audit.id).await;
                assert_eq!(
                    before["routeTrace"]["lumaReconciliation"]["state"],
                    "pending"
                );
                worker::tick(&f.state, &store, now() + 61).await.unwrap();
                let after = row(&f, &audit.id).await;
                assert_eq!(
                    after["routeTrace"]["lumaAgentsGeneration"]["usage"]["credits_charged"],
                    credits
                );
                assert_eq!(
                    after["routeTrace"]["lumaReconciliation"]["state"],
                    "settled"
                );
                for field in [
                    "status",
                    "responseId",
                    "totalTokens",
                    "completedAt",
                    "providerAccountId",
                ] {
                    assert_eq!(before[field], after[field]);
                }
                assert_eq!(f.balance().await.remaining_messages, Some(1));
                worker::tick(&f.state, &store, now() + 1000).await.unwrap();
                let calls = server.calls();
                assert_eq!(calls.len(), 3);
                assert_eq!(calls[2].method, "GET");
                assert_eq!(calls[2].path, format!("/v1/generations/{ID}"));
                assert_eq!(
                    calls[2].headers["authorization"],
                    format!("Bearer {}", pipeline_fixture::KEY)
                );
                f.finish().await;
            }),
        )
        .await;
        server.stop().await;
    }
}

#[tokio::test]
async fn luma_reconciliation_bounds_unknown_and_rejects_changed_identity() {
    for mode in ["unknown", "foreign", "revision", "unauthorized"] {
        let mut latest = job("completed");
        if mode == "foreign" {
            latest["model"] = json!("foreign-model");
            latest["usage"] = json!({"credits_charged":5});
        }
        let server = Server::start(vec![
            Reply::json(201, job("completed")),
            Reply::png(),
            Reply::json(if mode == "unauthorized" { 401 } else { 200 }, latest),
        ])
        .await;
        within(
            &server.origin,
            Box::pin(async {
                let f = Fixture::new(&server, 5).await;
                let audit = completed(&f).await;
                let store = store::Store::from_state(&f.state).unwrap();
                let previous = row(&f, &audit.id).await["routeTrace"].clone();
                let mut changed = previous.clone();
                if mode == "revision" {
                    changed["lumaReconciliation"]["binding"]["revision"] = json!("old");
                }
                if mode == "unknown" {
                    changed["lumaReconciliation"]["attempts"] = json!(11);
                }
                assert!(store.update(&audit.id, &previous, &changed).await.unwrap());
                worker::tick(&f.state, &store, now() + 61).await.unwrap();
                let after = row(&f, &audit.id).await;
                assert!(
                    after["routeTrace"]["lumaAgentsGeneration"]["usage"]["credits_charged"]
                        .is_null()
                );
                assert_eq!(
                    after["routeTrace"]["lumaReconciliation"]["state"],
                    if mode == "unknown" {
                        "exhausted"
                    } else {
                        "blocked"
                    }
                );
                worker::tick(&f.state, &store, now() + 10000).await.unwrap();
                assert_eq!(server.calls().len(), if mode == "revision" { 2 } else { 3 });
                f.finish().await;
            }),
        )
        .await;
        server.stop().await;
    }
}

#[tokio::test]
async fn luma_reconciliation_claim_survives_reopen_and_rejects_stale_writer() {
    let server = Server::start(vec![Reply::json(201, job("completed")), Reply::png()]).await;
    within(
        &server.origin,
        Box::pin(async {
            let f = Fixture::new(&server, 5).await;
            let audit = completed(&f).await;
            let local = f.state.local_runtime.as_ref().unwrap();
            let store = store::Store::Local(local.clone());
            let previous = row(&f, &audit.id).await["routeTrace"].clone();
            let mut claimed = previous.clone();
            claimed["lumaReconciliation"]["nextAt"] = json!(now() + 600);
            assert!(store.update(&audit.id, &previous, &claimed).await.unwrap());
            assert!(!store.update(&audit.id, &previous, &claimed).await.unwrap());
            assert!(store.due(now() + 61).await.unwrap().is_empty());
            // Open a second owner using the same actual SQLite filename, without retiring the first.
            let databases: Vec<(i32, String, String)> = sqlx::query_as("PRAGMA database_list")
                .fetch_all(&local.pool)
                .await
                .unwrap();
            let path = std::path::Path::new(
                &databases
                    .iter()
                    .find(|(_, name, _)| name == "main")
                    .unwrap()
                    .2,
            )
            .parent()
            .unwrap();
            let second = crate::local_runtime::LocalRuntime::open(path)
                .await
                .unwrap();
            let recovered = store::Store::Local(second.clone());
            assert_eq!(recovered.due(now() + 601).await.unwrap().len(), 1);
            assert!(!recovered
                .update(&audit.id, &previous, &claimed)
                .await
                .unwrap());
            second.close().await;
            f.finish().await;
        }),
    )
    .await;
    server.stop().await;
}
