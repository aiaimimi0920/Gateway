//! 取消退还预付次数；未知拒绝不重放；未实现入口不外发。
use super::*;
#[tokio::test]
async fn commandcode_sse_cancel_refunds_and_releases_source() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            let peer = peer::Peer::new(family(endpoint), false, true, 200).await;
            let fixture = Box::pin(Fixture::new(&peer)).await;
            super::super::test_support::origin(peer.origin.clone(), async {
                let response = fixture.open(endpoint, body(endpoint, true)).await;
                assert_eq!(response.status(), StatusCode::OK);
                let mut body = response.into_body();
                let mut seen = Vec::new();
                tokio::time::timeout(Duration::from_secs(3), async {
                    while !String::from_utf8_lossy(&seen).contains("合成答案") {
                        seen.extend_from_slice(
                            &body.frame().await.unwrap().unwrap().into_data().unwrap(),
                        );
                    }
                })
                .await
                .unwrap();
                drop(body);
                let rows = fixture.audits(1).await;
                assert_eq!(rows[0].status, "cancelled");
                assert_identity(&rows[0]);
                assert!(rows[0].total_tokens.is_none());
                assert_eq!(fixture.remaining().await, 2);
                assert_wire(&peer, 1, endpoint);
                peer.wait_closed().await;
                let states = fixture
                    .state
                    .local_runtime
                    .as_ref()
                    .unwrap()
                    .list_model_states(db::CredentialModelStateFilters::default())
                    .await
                    .unwrap();
                assert!(states.iter().all(|s| s.failure_count == 0));
            })
            .await;
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}
#[tokio::test]
async fn commandcode_unknown_rejection_is_not_replayed_or_charged() {
    Box::pin(async {
        for status in [400, 401, 402, 403, 429, 500, 503] {
            for stream in [false, true] {
                let peer = peer::Peer::new("chat", false, false, status).await;
                let fixture = Box::pin(Fixture::with_fallback(&peer, true)).await;
                super::super::test_support::origin(peer.origin.clone(), async {
                    let response = fixture
                        .open("chat/completions", body("chat/completions", stream))
                        .await;
                    assert_eq!(response.status().as_u16(), status);
                    let text = drain(response.into_body()).await;
                    assert!(
                        text.contains("commandcode_upstream_rejected") && !text.contains(KEY),
                        "{text}"
                    );
                    let rows = fixture.audits(1).await;
                    assert_identity_among(&rows[0], &["exact", "unrelated"]);
                    assert_ne!(rows[0].status, "completed");
                    assert_eq!(fixture.remaining().await, 2);
                    assert_wire(&peer, 1, "chat");
                })
                .await;
                fixture.finish().await;
                peer.stop().await;
            }
        }
    })
    .await;
}
#[tokio::test]
async fn commandcode_unimplemented_endpoints_fail_before_network() {
    Box::pin(async {
        for (endpoint, body) in [
            ("embeddings", json!({"model":MODEL,"input":"hello"})),
            ("completions", json!({"model":MODEL,"prompt":"hello"})),
            (
                "responses",
                json!({"model":MODEL,"input":"hello","store":true}),
            ),
            (
                "responses",
                json!({"model":MODEL,"input":"hello","previous_response_id":"resp_old"}),
            ),
        ] {
            let peer = peer::Peer::new("chat", false, false, 200).await;
            let fixture = Box::pin(Fixture::new(&peer)).await;
            super::super::test_support::origin(peer.origin.clone(), async {
                let response = fixture.open(endpoint, body).await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert!(!status.is_success(), "{text}");
                assert!(peer.calls.lock().unwrap().is_empty());
                assert_ne!(fixture.audits(1).await[0].status, "completed");
                assert_eq!(fixture.remaining().await, 2);
            })
            .await;
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}
