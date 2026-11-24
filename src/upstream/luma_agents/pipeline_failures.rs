//! Public failures and local cancellation must not replay a potentially charged generation.
use super::{pipeline_fixture::*, test_support::*};
use serde_json::json;
use std::time::Duration;

fn single_submit(server: &Server, expected_calls: usize) {
    let calls = server.calls();
    assert_eq!(calls.len(), expected_calls);
    assert_eq!(calls.iter().filter(|c| c.method == "POST").count(), 1);
    for call in calls.iter().filter(|c| c.path.starts_with("/v1/")) {
        assert_eq!(call.headers["authorization"], format!("Bearer {KEY}"));
    }
}
#[tokio::test]
async fn luma_agents_pipeline_http_refusals_never_retry_or_fall_back_and_refund() {
    Box::pin(async {
        for status in [307, 401, 403, 429, 503] {
            let server = Server::start(vec![Reply::json(
                status,
                json!({"private":format!("upstream-private {KEY}")}),
            )])
            .await;
            let fixture = Box::pin(Fixture::new(&server, 5)).await;
            within(&server.origin, async {
                let response = fixture.open("images/generations", body(IMAGE)).await;
                assert!(!response.status().is_success());
                let text = drain(response.into_body()).await;
                assert!(
                    !text.contains(KEY) && !text.contains("upstream-private"),
                    "{text}"
                );
                let row = fixture.audit().await;
                assert_eq!(row.status, "failed");
                identity(&row, IMAGE, "uni-1");
                assert!(row.total_tokens.is_none());
                assert_eq!(row.route_trace.as_ref().unwrap()["candidateCount"], 2);
                assert_eq!(row.route_trace.as_ref().unwrap()["fallbackEligible"], false);
                assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                single_submit(&server, 1);
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}
#[tokio::test]
async fn luma_agents_pipeline_failed_mismatched_or_unverified_output_is_not_billable_success() {
    Box::pin(async {
        for scenario in 0..5 {
            let mut value = job(if scenario == 0 { "failed" } else { "completed" });
            let mut replies = Vec::new();
            match scenario {
                0 => {
                    value["failure_code"] = json!("content_moderated");
                    value["failure_reason"] = json!(format!("upstream-private {KEY}"));
                }
                1 => value["id"] = json!("not-a-generation-id"),
                2 => value["output"][0]["url"] = json!("https://127.0.0.1/private"),
                _ => {}
            }
            replies.push(Reply::json(201, value));
            if scenario == 3 {
                let mut asset = Reply::png();
                asset.bytes = b"<html>upstream-private</html>".to_vec();
                replies.push(asset);
            } else if scenario == 4 {
                replies.push(Reply::json(307, json!({"private":"upstream-private"})));
            }
            let expected = replies.len();
            let server = Server::start(replies).await;
            let fixture = Box::pin(Fixture::new(&server, 5)).await;
            within(&server.origin, async {
                let response = fixture.open("images/generations", body(IMAGE)).await;
                assert!(!response.status().is_success());
                let text = drain(response.into_body()).await;
                assert!(
                    !text.contains(KEY)
                        && !text.contains("upstream-private")
                        && !text.contains("signature=private"),
                    "{text}"
                );
                let row = fixture.audit().await;
                assert_eq!(row.status, "failed");
                identity(&row, IMAGE, "uni-1");
                assert!(row.total_tokens.is_none());
                let facts = &row.route_trace.as_ref().unwrap()["lumaAgentsGeneration"];
                if scenario == 0 {
                    assert_eq!(facts["state"], "failed");
                    assert_eq!(row.response_id, ID);
                    assert!(facts["usage"]["credits_charged"].is_null());
                } else if scenario == 1 {
                    assert!(facts.is_null());
                } else {
                    assert_eq!(facts["state"], "completed");
                    assert_eq!(facts["mediaVerified"], false);
                    assert_eq!(row.response_id, ID);
                }
                assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                single_submit(&server, expected);
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}
fn held(phase: usize) -> Vec<Reply> {
    let mut replies = match phase {
        0 => vec![Reply::json(201, job("queued"))],
        1 => vec![
            Reply::json(201, job("queued")),
            Reply::json(200, job("completed")),
        ],
        _ => vec![
            Reply::json(201, job("queued")),
            Reply::json(200, job("completed")),
            Reply::png(),
        ],
    };
    replies.last_mut().unwrap().hold = true;
    replies
}
#[tokio::test]
async fn luma_agents_pipeline_submit_poll_and_asset_deadlines_release_source_and_refund() {
    Box::pin(async {
        for phase in 0..3 {
            let server = Server::start(held(phase)).await;
            let fixture = Box::pin(Fixture::new(&server, 1)).await;
            within(&server.origin, async {
                let response = fixture.open("images/generations", body(IMAGE)).await;
                assert!(!response.status().is_success());
                let text = drain(response.into_body()).await;
                assert!(!text.contains(ID) && !text.contains("credits_charged"));
                let row = fixture.audit().await;
                assert_ne!(row.status, "completed");
                super::cancellation_audit_tests::assert_phase(&row, phase);
                assert!(row.total_tokens.is_none());
                assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                single_submit(&server, phase + 1);
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
async fn luma_agents_pipeline_caller_drop_at_submit_poll_or_asset_is_neutral_and_refunds() {
    Box::pin(async {
        for phase in 0..3 {
            let server = Server::start(held(phase)).await;
            let fixture = Box::pin(Fixture::new(&server, 5)).await;
            within(&server.origin, async {
                let controller = fixture
                    .state
                    .concurrency_registry
                    .get_or_create("luma-pipeline");
                let initial = controller.snapshot().current_limit;
                let mut call = Box::pin(fixture.open("images/generations", body(IMAGE)));
                // Cancel at the observed stage, not after an assumed scheduler/network latency.
                tokio::time::timeout(Duration::from_secs(3), async {
                    tokio::select! {
                        _ = call.as_mut() => panic!("held upstream stage must remain pending"),
                        _ = async {
                            while server.calls().len() < phase + 1 {
                                tokio::task::yield_now().await;
                            }
                        } => {}
                    }
                })
                .await
                .expect("expected submit, poll or asset stage must be reached");
                single_submit(&server, phase + 1);
                assert_eq!(fixture.balance().await.remaining_messages, Some(1));
                drop(call);
                let row = fixture.audit().await;
                assert_eq!(row.status, "cancelled");
                super::cancellation_audit_tests::assert_phase(&row, phase);
                identity(&row, IMAGE, "uni-1");
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
                assert_eq!(controller.snapshot().active_count, 0);
                assert_eq!(controller.snapshot().current_limit, initial);
                server.wait_closed().await;
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}
