//! Existing SQLite instance recovery preserves checkpoints and the original cancellation outcome.
use super::*;
use crate::upstream::luma_agents::{pipeline_fixture::Fixture, test_support::*};

#[tokio::test]
async fn luma_reconciliation_orphan_recovery_keeps_pending_charge_and_ignores_live_owner() {
    let mut latest = job("completed");
    latest["usage"] = json!({"credits_charged":12.5});
    let server = Server::start(vec![Reply::json(200, latest)]).await;
    within(
        &server.origin,
        Box::pin(async {
            let f = Fixture::new(&server, 5).await;
            let local = f.state.local_runtime.as_ref().unwrap();
            let store = store::Store::Local(local.clone());
            let snapshot = f.state.route_config.snapshot();
            let target = snapshot
                .credential_probe_targets_for_provider("luma-pipeline")
                .unwrap()
                .into_iter()
                .find(|t| t.credential_id == "exact")
                .unwrap();
            for (id, orphan) in [("orphan", true), ("live", false)] {
                local
                    .create_audit(crate::db::CreateRequestAuditInput {
                        project_id: "local".into(),
                        api_key_id: None,
                        user_credential_id: None,
                        access_key_id: Some(f.id.clone()),
                        source_access_key_id: None,
                        session_id: None,
                        route_policy_id: None,
                        provider_account_id: Some("luma-pipeline".into()),
                        protocol_family: "openai".into(),
                        endpoint_kind: "images.generations".into(),
                        requested_model: Some("uni-1".into()),
                        resolved_model: None,
                        model_alias: None,
                        stream: false,
                        route_attempt_count: 1,
                        response_id: id.into(),
                        previous_response_id: None,
                        route_trace: None,
                    })
                    .await
                    .unwrap();
                let checkpoint =
                    Checkpoint::new(&f.state, id, "luma-pipeline", "exact", &target.payload)
                        .unwrap();
                // Zero observed while queued is not final settlement; a later terminal charge must still be read.
                checkpoint
                    .persist(
                        json!({"id":ID,"model":"uni-1","type":"image","state":"queued",
                "usage":{"credits_charged":0},"mediaVerified":false}),
                    )
                    .await
                    .unwrap();
                if orphan {
                    sqlx::query("UPDATE request_audits SET owner='dead-luma-owner' WHERE id=?")
                        .bind(id)
                        .execute(&local.pool)
                        .await
                        .unwrap();
                }
            }
            assert!(store.due(now() + 61).await.unwrap().is_empty());
            local.maintain().await.unwrap();
            worker::tick(&f.state, &store, now() + 61).await.unwrap();
            let rows = local
                .list_audits(&crate::db::RequestAuditFilters::default())
                .await
                .unwrap();
            let orphan = rows.iter().find(|r| r.id == "orphan").unwrap();
            assert_eq!(orphan.status, "cancelled");
            assert_eq!(orphan.response_id, "orphan");
            let trace = orphan.route_trace.as_ref().unwrap();
            assert_eq!(
                trace["lumaAgentsGeneration"]["usage"]["credits_charged"],
                12.5
            );
            assert_eq!(trace["lumaAgentsGeneration"]["state"], "queued");
            assert_eq!(trace["lumaReconciliation"]["state"], "settled");
            assert_eq!(
                rows.iter().find(|r| r.id == "live").unwrap().status,
                "running"
            );
            assert_eq!(f.balance().await.remaining_messages, Some(2));
            assert_eq!(server.calls().len(), 1);
            f.finish().await;
        }),
    )
    .await;
    server.stop().await;
}
