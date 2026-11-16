//! Submit, same-job poll and verified-image read retain one caller-owned lifetime and no replay.
use super::{
    pipeline_fixture::*,
    probe_tests::{completed, options, parsed},
    test_support::*,
};
use crate::provider_runtime::*;
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn chataibot_probe_http_and_invalid_image_failures_never_restore_health_or_retry_or_leak() {
    Box::pin(async {
        for phase in 0..7 {
            let mut replies = completed();
            if phase < 5 {
                replies = vec![Reply {
                    status: [307, 401, 403, 429, 503][phase],
                    ..Reply::json(json!({"secret":format!("private-body {TOKEN}")}))
                }];
            } else if phase == 5 {
                replies[1] = Reply::json(
                    json!({"status":"error","message":format!("private-body {TOKEN}")}),
                );
                replies.pop();
            } else {
                replies[2].bytes = b"not-a-valid-image".to_vec();
            }
            let expected = replies.len();
            let server = Server::start(replies).await;
            let fixture = Box::pin(Fixture::new(5)).await;
            let target = fixture
                .state
                .route_config
                .snapshot()
                .select_credential_probe_target("exact")
                .unwrap();
            let input = parsed(options());
            within(
                &server.origin,
                Box::pin(async {
                    let (_, report) = Box::pin(probe_console_target_with_request(
                        &fixture.state,
                        &target,
                        Some(&input),
                    ))
                    .await;
                    assert_eq!(
                        report.status,
                        ProviderPayloadProbeStatus::Failed,
                        "{}",
                        report.message
                    );
                    assert!(report.answer.is_none());
                    let row = fixture.audit().await;
                    assert_eq!(row.status, "failed");
                    assert!(row.route_trace.as_ref().unwrap()["chataibotImage"].is_null());
                    assert!(row.total_tokens.is_none());
                    let saved =
                        format!("{}{}", report.message, serde_json::to_string(&row).unwrap());
                    for secret in [TOKEN, "private-body", "synthetic-private"] {
                        assert!(!saved.contains(secret), "{saved}");
                    }
                    assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                    assert_eq!(server.calls().len(), expected);
                    assert_eq!(
                        server.calls().iter().filter(|c| c.method == "POST").count(),
                        1
                    );
                    let states = fixture
                        .state
                        .local_runtime
                        .as_ref()
                        .unwrap()
                        .list_model_states(Default::default())
                        .await
                        .unwrap();
                    assert!(
                        states.iter().all(|row| row.last_success_at.is_none()),
                        "{states:?}"
                    );
                }),
            )
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}
fn held(phase: usize) -> Vec<Reply> {
    let mut replies = completed();
    replies.truncate(phase + 1);
    replies.last_mut().unwrap().hold = true;
    replies
}
#[tokio::test]
async fn chataibot_probe_deadlines_at_submit_poll_asset_close_source_without_key_charge() {
    Box::pin(async {
        for phase in 0..3 {
            let server = Server::start(held(phase)).await;
            let fixture = Box::pin(Fixture::new(1)).await;
            let target = fixture
                .state
                .route_config
                .snapshot()
                .select_credential_probe_target("exact")
                .unwrap();
            let input = parsed(options());
            within(
                &server.origin,
                Box::pin(async {
                    let (_, report) = Box::pin(probe_console_target_with_request(
                        &fixture.state,
                        &target,
                        Some(&input),
                    ))
                    .await;
                    assert_eq!(report.status, ProviderPayloadProbeStatus::Failed);
                    assert!(report.message.contains("chataibot_timeout"));
                    assert_eq!(fixture.audit().await.status, "failed");
                    assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                    assert_eq!(server.calls().len(), phase + 1);
                    server.wait_closed().await;
                }),
            )
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}
#[tokio::test]
async fn chataibot_probe_caller_drop_at_submit_poll_asset_cancels_audit_without_health_penalty() {
    Box::pin(async {
        for phase in 0..3 {
            let server=Server::start(held(phase)).await;let fixture=Box::pin(Fixture::new(5)).await;
            let target=fixture.state.route_config.snapshot().select_credential_probe_target("exact").unwrap();let input=parsed(options());
            within(&server.origin,Box::pin(async {
                let mut call=Box::pin(probe_console_target_with_request(&fixture.state,&target,Some(&input)));
                tokio::select! { _=&mut call=>panic!("held probe completed"),()=server.wait_calls(phase+1)=>{} }
                drop(call);let row=fixture.audit().await;assert_eq!(row.status,"cancelled");
                assert_eq!(row.requested_model.as_deref(),Some(MODEL));assert_eq!(row.resolved_model.as_deref(),Some(RESOLVED));
                assert_eq!(row.route_trace.as_ref().unwrap()["consoleMediaProbe"]["chargeConfirmed"],true);
                assert!(row.route_trace.as_ref().unwrap()["chataibotImage"].is_null());
                assert_eq!(fixture.balance().await.remaining_messages,Some(2));
                let states=fixture.state.local_runtime.as_ref().unwrap().list_model_states(Default::default()).await.unwrap();
                assert!(states.iter().all(|row|row.failure_count==0 && row.last_success_at.is_none()),"{states:?}");
                assert_eq!(server.calls().len(),phase+1);server.wait_closed().await;
            })).await;
            fixture.finish().await;server.stop().await;
        }
    }).await;
}
