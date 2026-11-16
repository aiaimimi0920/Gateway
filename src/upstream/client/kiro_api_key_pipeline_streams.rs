//! Truncated frames, late provider errors and caller drop retain the one finalize owner.
use super::*;

#[test]
fn kiro_api_key_pipeline_sse_late_frame_errors_do_not_complete_or_publish_usage() {
    run(late_errors);
}
async fn late_errors() {
    let good = wire::event_frame("VISIBLE");
    let mut bad_crc = wire::event_frame("BAD");
    *bad_crc.last_mut().unwrap() ^= 1;
    for endpoint in ["chat/completions", "messages", "responses"] {
        for tail in [
            good[..good.len() - 1].to_vec(),
            [good.clone(), bad_crc.clone()].concat(),
            [
                good.clone(),
                wire::error_frame("private-upstream-detail synthetic-exact-key"),
            ]
            .concat(),
        ] {
            let server = wire::Server::stream(tail, false).await;
            let fixture = Fixture::new(&server).await;
            wire::within(&server.origin, async {
                let response = fixture.open(endpoint, body(endpoint, true), true).await;
                let (text, failed) = drain(response.into_body()).await;
                assert!(failed || text.contains("error"), "{text}");
                assert!(
                    !text.contains("private-upstream-detail")
                        && !text.contains("synthetic-exact-key"),
                    "{text}"
                );
                assert!(
                    !text.contains("response.completed")
                        && !text.contains("message_stop")
                        && !text.contains("[DONE]"),
                    "{text}"
                );
                let row = fixture.audit().await;
                assert_eq!(row.status, "failed");
                assert!(row.total_tokens.is_none());
                let saved = serde_json::to_string(&row).unwrap();
                assert!(
                    !saved.contains("private-upstream-detail")
                        && !saved.contains("synthetic-exact-key"),
                    "{saved}"
                );
                assert_identity(&row);
                assert_wire(&server);
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    }
}
#[test]
fn kiro_api_key_pipeline_sse_drop_releases_connection_and_cancels_without_health_failure() {
    run(cancel);
}
async fn cancel() {
    for endpoint in ["chat/completions", "messages", "responses"] {
        let server = wire::Server::stream(wire::event_frame("VISIBLE"), true).await;
        let fixture = Fixture::new(&server).await;
        wire::within(&server.origin, async {
            let response = fixture.open(endpoint, body(endpoint, true), true).await;
            assert_eq!(response.status(), StatusCode::OK);
            let mut body = response.into_body();
            let mut seen = String::new();
            while !seen.contains("VISIBLE") {
                let frame = body.frame().await.unwrap().unwrap().into_data().unwrap();
                seen.push_str(std::str::from_utf8(&frame).unwrap());
            }
            let ended = tokio::time::timeout(Duration::from_millis(100), async {
                while let Some(frame) = body.frame().await {
                    if let Ok(frame) = frame {
                        if let Ok(bytes) = frame.into_data() {
                            seen.push_str(std::str::from_utf8(&bytes).unwrap());
                        }
                    }
                }
            })
            .await;
            assert!(ended.is_err());
            assert!(
                !seen.contains("response.completed")
                    && !seen.contains("message_stop")
                    && !seen.contains("[DONE]")
            );
            drop(body);
            let row = fixture.audit().await;
            assert_eq!(row.status, "cancelled");
            assert!(row.total_tokens.is_none());
            assert_identity(&row);
            assert_wire(&server);
            let states = fixture
                .local
                .list_model_states(db::CredentialModelStateFilters::default())
                .await
                .unwrap();
            assert!(states.iter().all(|s| s.failure_count == 0));
            tokio::time::timeout(Duration::from_secs(3), async {
                while server.closed() == 0 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
        })
        .await;
        fixture.finish().await;
        server.stop().await;
    }
}
