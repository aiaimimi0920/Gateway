//! 官方 Responses 的两套用量字段、未完成详情和 HTTP 200 错误结算。
use super::*;

#[tokio::test]
async fn agnes_responses_usage_aliases_reach_audit_for_json_and_sse() {
    Box::pin(async {
        for stream in [false, true] {
            let bytes = String::from_utf8(native::reply("responses", false, stream, true))
                .unwrap()
                .replace("\"input_tokens\"", "\"prompt_tokens\"")
                .replace("\"output_tokens\"", "\"completion_tokens\"")
                .into_bytes();
            let peer = peer::Peer::raw(bytes).await;
            let fixture = Box::pin(Fixture::new(&peer)).await;
            super::super::test_support::origin(peer.origin.clone(), async {
                let response = fixture.open("responses", body("responses", stream)).await;
                assert_eq!(response.status(), StatusCode::OK);
                assert!(drain(response.into_body()).await.contains("合成答案"));
                let rows = fixture.audits(1).await;
                assert_identity(&rows[0]);
                assert_eq!(
                    (
                        rows[0].prompt_tokens,
                        rows[0].completion_tokens,
                        rows[0].total_tokens
                    ),
                    (Some(2), Some(3), Some(5))
                );
                assert_eq!(fixture.remaining().await, 1);
                assert_wire(&peer, 1, "responses");
            })
            .await;
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn agnes_responses_json_preserves_incomplete_details() {
    Box::pin(async {
        let expected = json!({"id":"agnes-native", "object":"response", "model":UPSTREAM_MODEL,
            "status":"incomplete", "incomplete_details":{"reason":"max_output_tokens"},
            "error":null, "output":[{"type":"message", "role":"assistant",
                "content":[{"type":"output_text", "text":"合成答案"}]}],
            "usage":{"input_tokens":2,"output_tokens":3,"total_tokens":5}});
        let peer = peer::Peer::raw(serde_json::to_vec(&expected).unwrap()).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let response = fixture.open("responses", body("responses", false)).await;
            assert_eq!(response.status(), StatusCode::OK);
            let actual: Value = serde_json::from_str(&drain(response.into_body()).await).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(fixture.audits(1).await[0].total_tokens, Some(5));
            assert_wire(&peer, 1, "responses");
        })
        .await;
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}

#[tokio::test]
async fn agnes_responses_body_error_refunds_without_replay() {
    Box::pin(async {
        for reply in [
            json!({"status":"failed","error":{"code":"synthetic_failure","message":KEY},"output":[]}),
            json!({"status":"cancelled","error":null,"output":[]}),
            json!({"status":"completed","error":{"message":KEY},"output":[]}),
            json!({"status":"in_progress","error":null,"output":[]}),
            json!({"status":"completed"}),
        ] {
            let peer = peer::Peer::raw(serde_json::to_vec(&reply).unwrap()).await;
            let fixture = Box::pin(Fixture::new(&peer)).await;
            super::super::test_support::origin(peer.origin.clone(), async {
                let response = fixture.open("responses", body("responses", false)).await;
                assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
                let text = drain(response.into_body()).await;
                assert!(!text.contains(KEY), "{text}");
                assert!(text.contains("agnes_response_failed"), "{text}");
                let rows = fixture.audits(1).await;
                assert_identity(&rows[0]);
                assert_ne!(rows[0].status, "completed");
                assert_eq!(fixture.remaining().await, 2);
                assert_wire(&peer, 1, "responses");
            }).await;
            fixture.finish().await;
            peer.stop().await;
        }
    }).await;
}
