//! Failed or cancelled resolution never starts inference or consumes the public prepaid balance.
use super::super::test_support::{Peer, Reply};
use super::*;
#[tokio::test]
async fn lobsterai_virtual_resolution_failures_do_not_infer_or_replay() {
    Box::pin(async {
        let mut replies = vec![
            Reply::raw(403, b"secret".to_vec()),
            Reply::raw(429, b"secret".to_vec()),
            Reply::raw(500, b"secret".to_vec()),
            Reply::raw(302, vec![]).header("location", "https://example.invalid"),
            Reply::raw(200, b"{\"code\":0,\"code\":0,\"data\":{}}".to_vec()),
            Reply::raw(200, vec![b'x'; 65_537]),
            Reply::json(json!({"code":40101,"message":KEY})),
        ];
        for model in [
            json!({"modelId":"yd-ultra","apiFormat":"openai"}),
            json!({"modelId":"model","apiFormat":"openai","accessible":false}),
            json!({"modelId":"model","apiFormat":"gemini"}),
            json!({"modelId":"model","apiFormat":"openai","runtimeProfile":"unknown"}),
            json!({"modelId":"kimi-k3","apiFormat":"openai"}),
            json!({"apiFormat":"openai"}),
        ] {
            replies.push(Reply::json(json!({"code":0,"data":model})));
        }
        for reply in replies {
            let peer = Peer::start(vec![reply]).await;
            let fixture = Box::pin(virtual_models::fixture(&peer, "yd-fast")).await;
            super::super::transport::TEST_ORIGIN
                .scope(peer.origin.clone(), async {
                    let response = fixture
                        .open("chat/completions", body("chat/completions", false))
                        .await;
                    assert_ne!(response.status(), StatusCode::OK);
                    assert!(!drain(response.into_body()).await.contains(KEY));
                })
                .await;
            assert_ne!(fixture.audits(1).await[0].status, "completed");
            assert_eq!(fixture.remaining().await, 2);
            fixture.finish().await;
            peer.inspect(|calls| assert_eq!(calls.len(), 1));
        }
    })
    .await;
}
#[tokio::test]
async fn lobsterai_virtual_401_wakes_worker_then_next_request_resolves_again() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            let peer = Peer::start(vec![
                Reply::raw(401, KEY.as_bytes().to_vec()),
                Reply::json(json!({"code":0,"data":{"accessToken":"new-access"}})),
                virtual_models::resolution("openai"),
                Reply::raw(200, native::reply("chat", false, false, true)),
            ])
            .await;
            let fixture = Box::pin(virtual_models::fixture(&peer, "yd-fast")).await;
            super::super::transport::TEST_ORIGIN
                .scope(peer.origin.clone(), async {
                    let response = fixture.open(endpoint, body(endpoint, false)).await;
                    assert_ne!(response.status(), StatusCode::OK);
                    drain(response.into_body()).await;
                    assert_ne!(fixture.audits(1).await[0].status, "completed");
                    assert_eq!(fixture.remaining().await, 2);
                    tokio::time::timeout(Duration::from_secs(5), async {
                        loop {
                            if fixture.state.route_config.snapshot().document().providers[0]
                                .credentials[0]
                                .api_key
                                .as_deref()
                                == Some("new-access")
                            {
                                break;
                            }
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .unwrap();
                    let response = fixture.open(endpoint, body(endpoint, false)).await;
                    let status = response.status();
                    let text = drain(response.into_body()).await;
                    assert_eq!(status, StatusCode::OK, "{text}");
                })
                .await;
            assert_eq!(
                fixture
                    .audits(2)
                    .await
                    .iter()
                    .filter(|a| a.status == "completed")
                    .count(),
                1
            );
            assert_eq!(fixture.remaining().await, 1);
            fixture.finish().await;
            peer.inspect(|calls| {
                assert_eq!(calls.len(), 4);
                assert_eq!(calls[1].path, "/api/auth/refresh");
                assert_eq!(calls[2].path, "/api/models/yd-fast/resolve");
                for call in &calls[2..] {
                    assert_eq!(call.headers["authorization"], "Bearer new-access");
                }
            });
        }
    })
    .await;
}
#[tokio::test]
async fn lobsterai_virtual_resolution_timeout_refunds_without_inference() {
    let peer = Peer::start(vec![Reply::stalled()]).await;
    let fixture = Box::pin(virtual_models::fixture(&peer, "yd-fast")).await;
    // The fixture worker holds AppState; production shares the existing five-second request budget.
    let response = super::super::transport::TEST_ORIGIN
        .scope(
            peer.origin.clone(),
            fixture.open("chat/completions", body("chat/completions", false)),
        )
        .await;
    assert_ne!(response.status(), StatusCode::OK);
    drain(response.into_body()).await;
    assert_ne!(fixture.audits(1).await[0].status, "completed");
    assert_eq!(fixture.remaining().await, 2);
    fixture.finish().await;
    peer.inspect(|calls| assert_eq!(calls.len(), 1));
}
