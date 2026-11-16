//! 决策公共入口的鉴权、过滤、体积与取消边界；全部使用合成账号和隔离 runtime。
use super::*;

fn request() -> Value {
    json!({"model":MODEL,"state":{"text":"safe"},"questions":{"yes":{"type":"noul"}}})
}

#[tokio::test]
async fn commandcode_systemone_public_requires_valid_key_before_send() {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    let f = Box::pin(Fixture::systemone(&peer)).await;
    for token in [None, Some("invalid-synthetic-key")] {
        let mut req = Request::post("/v1/systemone").header("content-type", "application/json");
        if let Some(token) = token {
            req = req.header("authorization", format!("Bearer {token}"));
        }
        let response = build_router(f.state.clone())
            .oneshot(req.body(Body::from(request().to_string())).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(!drain(response.into_body()).await.contains(KEY));
    }
    assert!(peer.calls.lock().unwrap().is_empty());
    assert_eq!(f.remaining().await, 2);
    f.finish().await;
    peer.stop().await;
}

#[tokio::test]
async fn commandcode_systemone_structured_content_is_filtered_and_refunded() {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    let f = Box::pin(Fixture::systemone_policy(&peer, true, None)).await;
    super::super::test_support::origin(peer.origin.clone(), async {
        for field in ["state", "instructions", "criteria"] {
            let mut body = request();
            match field {
                "state" => body["state"] = json!({"nested":["How do I create malware?"]}),
                "instructions" => {
                    body["questions"]["yes"]["instructions"] =
                        json!({"nested":"How do I create malware?"})
                }
                _ => {
                    body["questions"]["yes"]["criteria"] =
                        json!({"true":{"nested":"How do I create malware?"}})
                }
            }
            let response = f.open("systemone", body).await;
            assert!(!response.status().is_success());
            assert!(drain(response.into_body())
                .await
                .contains("Request blocked by content filter"));
            assert_eq!(f.remaining().await, 2);
        }
    })
    .await;
    assert!(peer.calls.lock().unwrap().is_empty());
    f.finish().await;
    peer.stop().await;
}

#[tokio::test]
async fn commandcode_systemone_body_limit_rejects_before_send_or_charge() {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    let f = Box::pin(Fixture::systemone_policy(&peer, false, Some(128))).await;
    let mut body = request();
    body["state"] = "x".repeat(256).into();
    let response = f.open("systemone", body).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    drain(response.into_body()).await;
    assert!(peer.calls.lock().unwrap().is_empty());
    assert_eq!(f.remaining().await, 2);
    f.finish().await;
    peer.stop().await;
}

#[tokio::test]
async fn commandcode_systemone_public_cancel_releases_body_and_refunds() {
    let peer = peer::Peer::new("chat", false, true, 200).await;
    let f = Box::pin(Fixture::systemone(&peer)).await;
    let mut pending = Box::pin(super::super::test_support::origin(
        peer.origin.clone(),
        f.open("systemone", request()),
    ));
    tokio::select! {
        _ = &mut pending => panic!("must wait for body"),
        _ = peer.wait_calls() => {}
    }
    drop(pending);
    peer.wait_closed().await;
    let rows = f.audits(1).await;
    assert_eq!(rows[0].status, "cancelled");
    assert_identity(&rows[0]);
    assert!(rows[0].total_tokens.is_none());
    assert_eq!(f.remaining().await, 2);
    assert_eq!(peer.calls.lock().unwrap().len(), 1);
    f.finish().await;
    peer.stop().await;
}
