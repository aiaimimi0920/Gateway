//! HTTP 200 不代表推理成功；含第二可调用候选时也不重放已收到的无效生成。
use super::*;
#[tokio::test]
async fn luma_inference_invalid_success_body_refunds_without_replay() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            for raw in [json!({"error":{"code":16,"message":KEY}}), json!({"choices":[]}),
                json!({"choices":[{"message":{"role":"assistant","content":"unfinished"},"finish_reason":null}]}),
                json!({"choices":[{"message":{"role":"assistant","content":null},"finish_reason":"tool_calls"}]})] {
                let peer = peer::Peer::raw(raw.to_string().into_bytes()).await;
                let fixture = Box::pin(Fixture::with_fallback(&peer,true)).await;
                super::super::test_support::origin(peer.origin.clone(),async {
                    let response = fixture.open(endpoint,body(endpoint,false)).await;
                    assert_eq!(response.status(),StatusCode::BAD_GATEWAY);
                    let text = drain(response.into_body()).await;
                    assert!(text.contains("luma_inference_response_failed") && !text.contains(KEY),"{text}");
                    assert_ne!(fixture.audits(1).await[0].status,"completed");
                    assert_eq!(fixture.remaining().await,2); assert_wire(&peer,1,endpoint);
                }).await;
                fixture.finish().await; peer.stop().await;
            }
        }
    }).await;
}
