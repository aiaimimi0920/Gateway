//! Lost submit/poll/asset results never authorize replay or fallback to another account.
use super::{pipeline_fixture::*, test_support::*};
use serde_json::json;

fn once(server: &Server, calls: usize) {
    let rows = server.calls();
    assert_eq!(rows.len(), calls);
    assert_eq!(rows.iter().filter(|c| c.method == "POST").count(), 1);
}
#[tokio::test]
async fn chataibot_pipeline_refusals_and_invalid_results_never_fallback_and_refund() {
    Box::pin(async {
        for scenario in 0..10 {
            let mut replies = Vec::new();
            match scenario {
                0..=4 => replies.push(Reply {
                    status: [307, 401, 403, 429, 503][scenario],
                    ..Reply::json(json!({"message":format!("private-{TOKEN}")}))
                }),
                5 => replies.push(Reply::json(
                    json!({"status":"pending","jobId":"../foreign"}),
                )),
                6 => {
                    replies.push(Reply::json(
                        json!({"status":"pending","jobId":"synthetic-job"}),
                    ));
                    replies.push(Reply::json(
                        json!({"status":"error","message":format!("private-{TOKEN}")}),
                    ));
                }
                7 => replies.push(Reply::json(json!({"imageUrl":"https://127.0.0.1/private"}))),
                8 => {
                    replies.push(Reply::json(json!({"imageUrl":ASSET})));
                    replies.push(Reply {
                        bytes: b"\x89PNG\r\n\x1a\n".to_vec(),
                        ..Reply::png()
                    });
                }
                9 => {
                    replies.push(Reply::json(json!({"imageUrl":ASSET})));
                    replies.push(Reply {
                        status: 307,
                        ..Reply::json(json!({"private":TOKEN}))
                    });
                }
                _ => unreachable!(),
            }
            let count = replies.len();
            let server = Server::start(replies).await;
            let fixture = Box::pin(Fixture::new(5)).await;
            within(&server.origin, async {
                let response = fixture.open("images/generations", body()).await;
                assert!(!response.status().is_success());
                let text = drain(response.into_body()).await;
                assert!(
                    !text.contains(TOKEN) && !text.contains("synthetic-private"),
                    "{text}"
                );
                let row = fixture.audit().await;
                assert_eq!(row.status, "failed");
                identity(&row);
                let trace = row.route_trace.as_ref().unwrap();
                assert_eq!(trace["candidateCount"], 2);
                assert_eq!(trace["fallbackEligible"], false);
                assert!(row.total_tokens.is_none());
                assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                once(&server, count);
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}
fn held(phase: usize) -> Vec<Reply> {
    let mut replies = vec![Reply::json(
        json!({"status":"pending","jobId":"synthetic-job"}),
    )];
    if phase > 0 {
        replies.push(Reply::json(
            json!({"status":"completed","data":{"imageUrl":ASSET}}),
        ));
    }
    if phase > 1 {
        replies.push(Reply::png());
    }
    replies.last_mut().unwrap().hold = true;
    replies
}
#[tokio::test]
async fn chataibot_pipeline_total_deadline_at_submit_poll_or_asset_releases_source_and_refunds() {
    Box::pin(async {
        for phase in 0..3 {
            let server = Server::start(held(phase)).await;
            let fixture = Box::pin(Fixture::new(1)).await;
            within(&server.origin, async {
                let response = fixture.open("images/generations", body()).await;
                assert!(!response.status().is_success());
                let text = drain(response.into_body()).await;
                assert!(text.contains("chataibot_timeout"), "{text}");
                let row = fixture.audit().await;
                assert_eq!(row.status, "failed");
                identity(&row);
                assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                once(&server, phase + 1);
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
async fn chataibot_pipeline_caller_drop_at_each_phase_is_neutral_and_refunds() {
    Box::pin(async {
        for phase in 0..3 {
            let server = Server::start(held(phase)).await; let fixture = Box::pin(Fixture::new(5)).await;
            within(&server.origin, async {
                let controller = fixture.state.concurrency_registry.get_or_create("chataibot-pipeline");
                let initial = controller.snapshot().current_limit;
                let mut call = Box::pin(fixture.open("images/generations", body()));
                tokio::select! { result = &mut call => panic!("held response finished: {}", result.status()),
                    () = server.wait_calls(phase + 1) => {} }
                assert_eq!(fixture.balance().await.remaining_messages, Some(1)); drop(call);
                let row = fixture.audit().await; assert_eq!(row.status, "cancelled"); identity(&row);
                assert!(row.total_tokens.is_none()); assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                let states = fixture.state.local_runtime.as_ref().unwrap().list_model_states(crate::db::CredentialModelStateFilters::default()).await.unwrap();
                assert!(states.iter().all(|s| s.failure_count == 0), "{states:?}");
                assert_eq!(controller.snapshot().active_count, 0); assert_eq!(controller.snapshot().current_limit, initial);
                once(&server, phase + 1); server.wait_closed().await;
            }).await;
            fixture.finish().await; server.stop().await;
        }
    }).await;
}
#[tokio::test]
async fn chataibot_pipeline_admission_rejects_auth_models_and_unimplemented_options_without_network(
) {
    Box::pin(async {
        for scenario in 0..6 {
            let server = Server::start(vec![]).await;
            let mut fixture = Box::pin(Fixture::new(5)).await;
            let mut input = body();
            match scenario {
                0 => fixture.token = "invalid-key".into(),
                1 => input["model"] = json!("unauthorized-model"),
                2 => input["n"] = json!(2),
                3 => input["size"] = json!("1024x1024"),
                4 => input["quality"] = json!("high"),
                5 => input["response_format"] = json!("unknown"),
                _ => unreachable!(),
            }
            within(&server.origin, async {
                let response = fixture.open("images/generations", input).await;
                assert!(!response.status().is_success());
                let _ = drain(response.into_body()).await;
                assert!(server.calls().is_empty());
                assert_eq!(fixture.balance().await.remaining_messages, Some(2));
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}
#[tokio::test]
async fn chataibot_pipeline_unknown_usage_keeps_token_reservation_without_fake_image_tokens() {
    Box::pin(async {
        let server =
            Server::start(vec![Reply::json(json!({"imageUrl":ASSET})), Reply::png()]).await;
        let fixture = Box::pin(Fixture::new(5)).await;
        fixture
            .adjust(json!({"balanceMode":"token_prepaid","totalTokens":100,"remainingTokens":100}))
            .await;
        within(&server.origin, async {
            let response = fixture.open("images/generations", body()).await;
            assert!(response.status().is_success());
            let text = drain(response.into_body()).await;
            assert!(!text.contains("usage"));
            let row = fixture.audit().await;
            assert!(row.total_tokens.is_none());
            identity(&row);
            let estimate =
                (crate::redis::usage_tracking::estimate_token_count("合成图片测试") as f64 * 1.2)
                    .ceil() as i64;
            assert_eq!(
                fixture.balance().await.remaining_tokens,
                Some(100 - estimate)
            );
            once(&server, 2);
        })
        .await;
        fixture.finish().await;
        server.stop().await;
    })
    .await;
}
