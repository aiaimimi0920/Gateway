//! A cancelled or refused call must release its socket and reservation without false success.
use super::*;
#[tokio::test]
async fn nous_public_live_sse_drop_cancels_refunds_and_preserves_account_health() {
    Box::pin(async {
        for target in TARGETS {
            for endpoint in ENDPOINTS.into_iter().chain(["completions"]) {
                let peer = peer::Peer::new("chat", false, true, 200).await;
                let fixture = Box::pin(Fixture::with_target(&peer, target)).await;
                super::super::test_support::origin(peer.origin.clone(), async {
                    let response = fixture.open(endpoint, body(endpoint, true)).await;
                    assert_eq!(response.status(), StatusCode::OK);
                    let mut body = response.into_body();
                    let mut seen = Vec::new();
                    tokio::time::timeout(Duration::from_secs(3), async {
                        while !String::from_utf8_lossy(&seen).contains("合成答案") {
                            let frame = body.frame().await.unwrap().unwrap().into_data().unwrap();
                            seen.extend_from_slice(&frame);
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
                    assert_wire(&peer, 1);
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
        }
    })
    .await;
}
#[tokio::test]
async fn nous_public_unknown_rejection_is_not_replayed_or_charged() {
    Box::pin(async {
        for target in TARGETS {
            for endpoint in ENDPOINTS.into_iter().chain(["completions", "embeddings"]) {
                let peer = peer::Peer::new("chat", false, false, 400).await;
                let fixture = Box::pin(Fixture::with_target(&peer, target)).await;
                super::super::test_support::origin(peer.origin.clone(), async {
                    let request = if endpoint == "embeddings" {
                        json!({"model":MODEL,"input":"合成问题"})
                    } else {
                        body(endpoint, false)
                    };
                    let response = fixture.open(endpoint, request).await;
                    let status = response.status();
                    let text = drain(response.into_body()).await;
                    assert!(!status.is_success(), "{text}");
                    assert!(!text.contains(&fixture::access("exact")));
                    let rows = fixture.audits(1).await;
                    assert_identity(&rows[0]);
                    assert_ne!(rows[0].status, "completed");
                    assert_eq!(fixture.remaining().await, 2);
                    if endpoint == "embeddings" {
                        assert_eq!(peer.calls.lock().unwrap().len(), 1);
                        assert_eq!(peer.calls.lock().unwrap()[0].0, "/v1/embeddings");
                    } else {
                        assert_wire(&peer, 1);
                    }
                })
                .await;
                fixture.finish().await;
                peer.stop().await;
            }
        }
    })
    .await;
}
