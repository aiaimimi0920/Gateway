//! Public header -> request privacy latch -> fixed sender, including billing failure paths.
use super::*;

#[tokio::test]
async fn commandcode_zdr_public_three_protocol_json_sse_wire() {
    for endpoint in ENDPOINTS {
        for stream in [false, true] {
            let peer = peer::Peer::new(
                if endpoint == "chat/completions" {
                    "chat"
                } else {
                    endpoint
                },
                false,
                false,
                200,
            )
            .await;
            let f = match endpoint {
                "responses" => Box::pin(Fixture::native_responses(&peer)).await,
                "messages" => Box::pin(Fixture::native_messages(&peer)).await,
                _ => Box::pin(Fixture::new(&peer)).await,
            };
            super::super::test_support::origin(peer.origin.clone(), async {
                let response = f
                    .open_headers(endpoint, body(endpoint, stream), &["1"])
                    .await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{text}");
                assert!(text.contains("合成答案"), "{text}");
                assert_eq!(f.audits(1).await[0].status, "completed");
                assert_eq!(f.remaining().await, 1);
                let calls = peer.calls.lock().unwrap();
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].0, format!("/provider/v1/{endpoint}"));
                assert_eq!(calls[0].1["x-cmd-zdr"], "1");
                assert_eq!(calls[0].1["authorization"], format!("Bearer {KEY}"));
                assert!(!calls[0].1.contains_key("cookie"));
            })
            .await;
            f.finish().await;
            peer.stop().await;
        }
    }
}

#[tokio::test]
async fn commandcode_zdr_invalid_public_controls_never_send_or_charge() {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    let f = Box::pin(Fixture::new(&peer)).await;
    super::super::test_support::origin(peer.origin.clone(), async {
        for endpoint in ENDPOINTS {
            for values in [vec!["0"], vec![""], vec![" 1"], vec!["1,1"], vec!["1", "1"]] {
                let response = f
                    .open_headers(endpoint, body(endpoint, false), &values)
                    .await;
                assert_eq!(response.status(), StatusCode::BAD_REQUEST);
                assert!(drain(response.into_body())
                    .await
                    .contains("invalid_commandcode_zdr"));
                assert!(peer.calls.lock().unwrap().is_empty());
                assert_eq!(f.remaining().await, 2);
            }
        }
    })
    .await;
    f.finish().await;
    peer.stop().await;
}

#[tokio::test]
async fn commandcode_zdr_422_aborts_without_replay_and_refunds() {
    let peer = peer::Peer::new("chat", false, false, 422).await;
    let f = Box::pin(Fixture::with_fallback(&peer, true)).await;
    super::super::test_support::origin(peer.origin.clone(), async {
        let response = f
            .open_headers("chat/completions", body("chat/completions", false), &["1"])
            .await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let text = drain(response.into_body()).await;
        assert!(!text.contains("synthetic-private-error"));
        assert_eq!(f.audits(1).await[0].status, "failed");
        assert_eq!(f.remaining().await, 2);
        assert_eq!(peer.calls.lock().unwrap().len(), 1);
        assert_eq!(peer.calls.lock().unwrap()[0].1["x-cmd-zdr"], "1");
    })
    .await;
    f.finish().await;
    peer.stop().await;
}
