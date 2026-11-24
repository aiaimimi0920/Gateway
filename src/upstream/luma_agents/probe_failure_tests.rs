//! Media probes reuse sender failure/cancellation ownership without public Key accounting.
use super::{pipeline_fixture::*, probe_tests::options, test_support::*};
use crate::provider_runtime::{
    probe_console_target_with_request, ConsoleProbeRequest, ProviderPayloadProbeStatus,
};
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn luma_agents_probe_http_failures_never_retry_fallback_or_leak_upstream_secrets() {
    Box::pin(async {
        for status in [307, 401, 403, 429, 503] {
            let server = Server::start(vec![Reply::json(
                status,
                json!({"private":format!("upstream-private {KEY}")}),
            )])
            .await;
            let fixture = Box::pin(Fixture::new(&server, 5)).await;
            let target = fixture
                .state
                .route_config
                .snapshot()
                .select_credential_probe_target("exact")
                .unwrap();
            let request =
                ConsoleProbeRequest::parse(options("image", IMAGE).to_string().as_bytes())
                    .unwrap()
                    .unwrap();
            within(&server.origin, async {
                let (_, report) =
                    probe_console_target_with_request(&fixture.state, &target, Some(&request))
                        .await;
                assert_eq!(report.status, ProviderPayloadProbeStatus::Failed);
                let row = fixture.audit().await;
                assert_eq!(row.status, "failed");
                assert_eq!(
                    row.upstream_status,
                    Some(i32::from(if status == 307 { 502 } else { status }))
                );
                assert_eq!(row.requested_model.as_deref(), Some(IMAGE));
                assert_eq!(row.resolved_model.as_deref(), Some("uni-1"));
                assert!(row.total_tokens.is_none());
                assert!(row.route_trace.as_ref().unwrap()["lumaAgentsGeneration"].is_null());
                let saved = format!("{}{}", report.message, serde_json::to_string(&row).unwrap());
                assert!(!saved.contains(KEY) && !saved.contains("upstream-private"));
                assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                assert_eq!(server.calls().len(), 1);
                assert_eq!(server.calls()[0].method, "POST");
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}

fn held(phase: usize) -> Vec<Reply> {
    let mut replies = vec![Reply::json(201, job("queued"))];
    if phase >= 1 {
        replies.push(Reply::json(200, job("completed")));
    }
    if phase >= 2 {
        replies.push(Reply::png());
    }
    replies.last_mut().unwrap().hold = true;
    replies
}
#[tokio::test]
async fn luma_agents_probe_submit_poll_asset_deadlines_close_source_and_preserve_key_balance() {
    Box::pin(async {
        for phase in 0..3 {
            let server = Server::start(held(phase)).await;
            let fixture = Box::pin(Fixture::new(&server, 1)).await;
            let target = fixture
                .state
                .route_config
                .snapshot()
                .select_credential_probe_target("exact")
                .unwrap();
            let request =
                ConsoleProbeRequest::parse(options("image", IMAGE).to_string().as_bytes())
                    .unwrap()
                    .unwrap();
            within(&server.origin, async {
                let (_, report) =
                    probe_console_target_with_request(&fixture.state, &target, Some(&request))
                        .await;
                assert_eq!(report.status, ProviderPayloadProbeStatus::Failed);
                let row = fixture.audit().await;
                assert_eq!(row.status, "failed");
                super::cancellation_audit_tests::assert_phase(&row, phase);
                assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                assert_eq!(server.calls().len(), phase + 1);
                assert_eq!(
                    server.calls().iter().filter(|c| c.method == "POST").count(),
                    1
                );
                server.wait_closed().await;
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}
#[tokio::test]
async fn luma_agents_probe_cancellation_closes_submit_poll_asset_and_does_not_penalize_health() {
    Box::pin(async {
        for phase in 0..3 {
            let server = Server::start(held(phase)).await;
            let fixture = Box::pin(Fixture::new(&server, 5)).await;
            let target = fixture
                .state
                .route_config
                .snapshot()
                .select_credential_probe_target("exact")
                .unwrap();
            let request =
                ConsoleProbeRequest::parse(options("image", IMAGE).to_string().as_bytes())
                    .unwrap()
                    .unwrap();
            within(&server.origin, async {
                let mut call = Box::pin(probe_console_target_with_request(
                    &fixture.state,
                    &target,
                    Some(&request),
                ));
                assert!(
                    tokio::time::timeout(Duration::from_millis(200), call.as_mut())
                        .await
                        .is_err()
                );
                assert_eq!(server.calls().len(), phase + 1);
                drop(call);
                let row = fixture.audit().await;
                assert_eq!(row.status, "cancelled");
                super::cancellation_audit_tests::assert_phase(&row, phase);
                assert_eq!(row.requested_model.as_deref(), Some(IMAGE));
                assert_eq!(row.model_alias.as_deref(), Some(IMAGE));
                assert_eq!(
                    row.route_trace.as_ref().unwrap()["consoleMediaProbe"]["chargeConfirmed"],
                    true
                );
                assert!(row.total_tokens.is_none());
                assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                let states = fixture
                    .state
                    .local_runtime
                    .as_ref()
                    .unwrap()
                    .list_model_states(crate::db::CredentialModelStateFilters::default())
                    .await
                    .unwrap();
                assert!(states.iter().all(|s| s.failure_count == 0), "{states:?}");
                server.wait_closed().await;
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}
