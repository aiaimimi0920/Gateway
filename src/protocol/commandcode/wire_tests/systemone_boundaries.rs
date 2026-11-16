//! 公开决策接口的拒绝、退款和隐私边界；使用现有 loopback/runtime 设施。
use super::*;

fn request() -> Value {
    json!({"model":MODEL,"state":"合成状态","questions":{"yes":{"type":"noul"}}})
}

#[tokio::test]
async fn commandcode_systemone_invalid_public_request_never_sends_or_charges() {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    let f = Box::pin(Fixture::systemone(&peer)).await;
    super::super::test_support::origin(peer.origin.clone(), async {
        for (key, value) in [
            ("stream", json!(true)),
            ("state", json!(null)),
            ("questions", json!({})),
        ] {
            let mut body = request();
            body[key] = value;
            let response = f.open("systemone", body).await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert!(drain(response.into_body())
                .await
                .contains("invalid_commandcode_systemone"));
            assert_eq!(f.remaining().await, 2);
            assert!(peer.calls.lock().unwrap().is_empty());
        }
    })
    .await;
    f.finish().await;
    peer.stop().await;
}

#[tokio::test]
async fn commandcode_systemone_bad_upstream_answers_refund_without_replay() {
    for body in [
        json!({"model":UPSTREAM_MODEL,"answers":{},"usage":{"input_tokens":1,"output_tokens":1}}),
        json!({"model":UPSTREAM_MODEL,"answers":{"yes":{"type":"noul","noul":0.5}},"usage":{"input_tokens":-1,"output_tokens":1}}),
        json!({"model":UPSTREAM_MODEL,"choices":[{"message":{"content":"not a decision"}}]}),
    ] {
        let peer = peer::Peer::raw(body.to_string().into_bytes()).await;
        let f = Box::pin(Fixture::systemone(&peer)).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let response = f.open("systemone", request()).await;
            assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
            assert!(!drain(response.into_body()).await.contains(KEY));
            assert_eq!(f.remaining().await, 2);
            assert_eq!(f.audits(1).await[0].status, "failed");
            assert_eq!(peer.calls.lock().unwrap().len(), 1);
        })
        .await;
        f.finish().await;
        peer.stop().await;
    }
}

#[tokio::test]
async fn commandcode_systemone_zdr_is_retained_and_422_refunds() {
    let peer = peer::Peer::new("chat", false, false, 422).await;
    let f = Box::pin(Fixture::systemone(&peer)).await;
    super::super::test_support::origin(peer.origin.clone(), async {
        let response = f.open_headers("systemone", request(), &["1"]).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert!(!drain(response.into_body())
            .await
            .contains("synthetic-private-error"));
        assert_eq!(f.remaining().await, 2);
        assert_eq!(f.audits(1).await[0].status, "failed");
        let calls = peer.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "/provider/v1/systemone");
        assert_eq!(calls[0].1["x-cmd-zdr"], "1");
    })
    .await;
    f.finish().await;
    peer.stop().await;
}

#[tokio::test]
async fn commandcode_systemone_does_not_infer_decisions_from_chat_catalog() {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    let f = Box::pin(Fixture::new(&peer)).await;
    super::super::test_support::origin(peer.origin.clone(), async {
        let response = f.open("systemone", request()).await;
        assert!(!response.status().is_success());
        drain(response.into_body()).await;
        assert_eq!(f.remaining().await, 2);
        assert!(peer.calls.lock().unwrap().is_empty());
    })
    .await;
    f.finish().await;
    peer.stop().await;
}
