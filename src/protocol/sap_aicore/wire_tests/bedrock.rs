//! SAP endpoint, scope, native frame conversion and settlement through the public prepaid key.
use super::*;
pub(super) mod media;
mod tools;
use crate::upstream::client::kiro_api_key_test_support::frame;

async fn fixture(peer: &peer::Peer, fallback: bool) -> Fixture {
    Fixture::configured(
        peer,
        fallback,
        super::super::test_support::BASE,
        KEY,
        json!({"sap_bedrock_converse": true}),
    )
    .await
}

pub(super) fn reply() -> Vec<u8> {
    json!({"model":"tenant-private-model", "output":{"message":{"role":"assistant","content":[{"text":"合成回复"}]}},
        "stopReason":"end_turn", "usage":{"inputTokens":2,"outputTokens":3,"totalTokens":5}})
        .to_string().into_bytes()
}

fn event(kind: &str, data: Value) -> Vec<u8> {
    frame(
        &[
            (":message-type", "event"),
            (":event-type", kind),
            (":content-type", "application/json"),
        ],
        data.to_string().as_bytes(),
    )
}

pub(super) fn events() -> Vec<u8> {
    [
        event("messageStart", json!({"role":"assistant"})),
        event(
            "contentBlockDelta",
            json!({"contentBlockIndex":0,"delta":{"text":"合成"}}),
        ),
        event(
            "contentBlockDelta",
            json!({"contentBlockIndex":0,"delta":{"text":"回复"}}),
        ),
        event("contentBlockStop", json!({"contentBlockIndex":0})),
        event("messageStop", json!({"stopReason":"end_turn"})),
        event(
            "metadata",
            json!({"usage":{"inputTokens":2,"outputTokens":3,"totalTokens":5},
            "metrics":{"privateExtension":"internal-metadata"}}),
        ),
    ]
    .concat()
}

#[tokio::test]
async fn sap_aicore_bedrock_chat_uses_native_wire_and_single_settlement() {
    Box::pin(async {
        let peer = peer::Peer::raw_sequence(vec![reply(), events()]).await;
        let f = fixture(&peer, false).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            for streaming in [false, true] {
                let mut request = json!({"model":MODEL,"stream":streaming,"n":1,"max_completion_tokens":32,
                    "temperature":0.25,"top_p":0.9,"stop":"end","user":"local-only",
                    "messages":[{"role":"system","content":"规则"},{"role":"user","content":"问题"}]});
                if streaming { request["stream_options"] = json!({"include_usage":true}); }
                let response = f.open("chat/completions", request).await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{text}");
                assert!(!text.contains("tenant-private-model") && !text.contains("internal-metadata") && !text.contains("gateway_bedrock"), "{text}");
                if streaming {
                    assert!(text.contains("data: [DONE]"), "{text}");
                    let frames: Vec<Value> = text.lines().filter_map(|line| line.strip_prefix("data: "))
                        .filter(|data| *data != "[DONE]").map(|data| serde_json::from_str(data).unwrap()).collect();
                    assert!(frames.iter().all(|frame| frame["model"] == MODEL));
                    let content: String = frames.iter().filter_map(|f| f.pointer("/choices/0/delta/content").and_then(Value::as_str)).collect();
                    assert_eq!(content, "合成回复");
                    assert_eq!(frames.iter().filter(|f| f["usage"]["total_tokens"] == 5).count(), 1);
                } else {
                    let json: Value = serde_json::from_str(&text).unwrap();
                    assert_eq!(json["model"], MODEL);
                    assert_eq!(json["choices"][0]["message"]["content"], "合成回复");
                    assert_eq!(json["usage"]["total_tokens"], 5);
                }
            }
            for audit in f.audits(2).await {
                assert_identity(&audit);
                assert_eq!(audit.status, "completed");
                assert_eq!(audit.total_tokens, Some(5));
            }
            assert_eq!(f.remaining().await, 0);
        }).await;
        assert_eq!(peer.oauth.lock().unwrap().len(), 1);
        {
            let sent = peer.calls.lock().unwrap();
            assert_eq!(sent.len(), 2);
            for ((path, headers, body), (op, accept)) in sent.iter().zip([
                ("converse", "application/json"), ("converse-stream", "application/vnd.amazon.eventstream")]) {
                assert_eq!(*path, format!("/v2/inference/deployments/deployment1/{op}"));
                assert_eq!(headers["authorization"], "Bearer synthetic-sap-access");
                assert_eq!(headers["AI-Resource-Group"], "group-a");
                assert_eq!(headers["accept"], accept);
                assert!(!headers.contains_key("cookie") && !headers.contains_key("x-amz-date"));
                assert_eq!(*body, json!({"system":[{"text":"规则"}],"messages":[{"role":"user","content":[{"text":"问题"}]}],
                    "inferenceConfig":{"maxTokens":32,"temperature":0.25,"topP":0.9,"stopSequences":["end"]}}));
            }
        }
        f.finish().await;
        peer.stop().await;
    }).await;
}

#[tokio::test]
async fn sap_aicore_bedrock_rejects_out_of_scope_before_oauth() {
    Box::pin(async {
        let peer = peer::Peer::raw(reply()).await;
        let f = fixture(&peer, false).await;
        let mut cases = vec![("messages", body("messages",false)), ("responses", body("responses", false))];
        for (key, value) in [("tools", json!([])), ("additionalModelRequestFields", json!({})),
            ("max_tokens", json!(0)), ("n",json!(2)),
            ("messages",json!([{"role":"user","content":[{"type":"image_url","image_url":{"url":"https://never-fetch.invalid/remote"}}]}]))] {
            let mut request = body("chat/completions", false);
            request[key] = value;
            cases.push(("chat/completions", request));
        }
        super::super::test_support::origin(peer.origin.clone(), async {
            for (endpoint, request) in cases {
                let response = f.open(endpoint, request).await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::BAD_REQUEST, "{endpoint}: {text}");
            }
            assert_eq!(f.remaining().await, 2);
        }).await;
        assert!(peer.oauth.lock().unwrap().is_empty() && peer.calls.lock().unwrap().is_empty());
        f.finish().await;
        peer.stop().await;
    }).await;
}

#[tokio::test]
async fn sap_aicore_bedrock_late_crc_failure_refunds_and_does_not_replay() {
    Box::pin(async {
        let mut bytes = events();
        *bytes.last_mut().unwrap() ^= 1;
        let peer = peer::Peer::raw(bytes).await;
        let f = fixture(&peer, true).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let response = f
                .open("chat/completions", body("chat/completions", true))
                .await;
            let mut body = response.into_body();
            let mut received = Vec::new();
            tokio::time::timeout(Duration::from_secs(5), async {
                while let Some(Ok(frame)) = body.frame().await {
                    if let Ok(bytes) = frame.into_data() {
                        received.extend_from_slice(&bytes);
                    }
                }
            })
            .await
            .unwrap();
            drop(body);
            let text = String::from_utf8(received).unwrap();
            assert!(
                !text.contains("[DONE]") && !text.contains("total_tokens"),
                "{text}"
            );
            assert_ne!(f.audits(1).await[0].status, "completed");
            assert_eq!(f.remaining().await, 2);
        })
        .await;
        assert_eq!(peer.oauth.lock().unwrap().len(), 1);
        assert_eq!(peer.calls.lock().unwrap().len(), 1);
        f.finish().await;
        peer.stop().await;
    })
    .await;
}
