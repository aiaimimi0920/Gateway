//! 决策接口必须通过真实公开路由和预付凭证，而不是伪装成聊天回复。
use super::*;

#[tokio::test]
async fn commandcode_systemone_public_answers_usage_and_billing() {
    let upstream = json!({"model":UPSTREAM_MODEL,"answers":{
        "urgent":{"type":"noul","noul":0.96},
        "department":{"type":"choice","choice":"billing","confidence":0.89,
            "probabilities":{"billing":0.91,"shipping":0.09}},
        "frustration":{"type":"score","score":0.7,"confidence":0.5,
            "legend":{"0":"calm","1":"angry"},"probabilities":{"0":0.3,"1":0.7}}
    },"usage":{"input_tokens":84,"output_tokens":3},"extension":{"preserved":true}});
    let peer = peer::Peer::raw(upstream.to_string().into_bytes()).await;
    let fixture = Box::pin(Fixture::systemone(&peer)).await;
    let (status, text) = super::super::test_support::origin(peer.origin.clone(), async {
        let response = fixture.open("systemone", json!({"model":MODEL,
            "state":{"message":"refund requested"},"questions":{
                "urgent":{"type":"noul","instructions":"urgent?"},
                "department":{"type":"choice","criteria":{"billing":null,"shipping":"delivery"}},
                "frustration":{"type":"score","criteria":["calm","angry"]}
            }})).await;
        (response.status(), drain(response.into_body()).await)
    })
    .await;
    // 即使前置断言失败也先结束本次隔离 runtime，避免失败测试留下 worker。
    let remaining = fixture.remaining().await;
    let audits = if status == StatusCode::OK {
        fixture.audits(1).await
    } else {
        vec![]
    };
    fixture.finish().await;
    let calls = peer.calls.lock().unwrap().clone();
    peer.stop().await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let body: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(body["answers"], upstream["answers"]);
    assert_eq!(body["extension"], upstream["extension"]);
    assert_eq!(body["model"], MODEL);
    assert!(body.get("choices").is_none());
    assert_eq!(remaining, 1);
    assert_identity(&audits[0]);
    assert_eq!(audits[0].status, "completed");
    assert_eq!(audits[0].total_tokens, Some(87));
    assert_eq!(calls.len(), 1);
    let (path, headers, request) = &calls[0];
    assert_eq!(path, "/provider/v1/systemone");
    assert_eq!(headers["authorization"], format!("Bearer {KEY}"));
    assert!(!headers.contains_key("cookie") && !headers.contains_key("originator"));
    assert_eq!(request["model"], UPSTREAM_MODEL);
    assert!(request.get("messages").is_none());
}
