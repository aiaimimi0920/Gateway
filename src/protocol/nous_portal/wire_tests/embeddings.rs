//! Embedding arrays and encodings must survive the public passthrough, not a Chat response adapter.
use super::*;

#[tokio::test]
async fn nous_public_embeddings_both_targets_preserve_input_data_and_usage() {
    Box::pin(async {
        for target in TARGETS {
            for input in [json!("合成问题"), json!(["甲", "乙"]), json!([12, 34]), json!([[12], [34]])] {
                for format in ["float", "base64"] {
                    let vector = if format == "float" { json!([0.5, -0.5]) } else { json!("AAAAPwAAAL8=") };
                    let upstream = json!({"object":"list","model":UPSTREAM_MODEL,"data":[
                        {"object":"embedding","index":0,"embedding":vector},
                        {"object":"embedding","index":1,"embedding":vector}],
                        "usage":{"prompt_tokens":2,"total_tokens":2}});
                    let peer = peer::Peer::raw(serde_json::to_vec(&upstream).unwrap()).await;
                    let fixture = Box::pin(Fixture::with_target(&peer, target)).await;
                    super::super::test_support::origin(peer.origin.clone(), async {
                        let response = fixture.open("embeddings", json!({"model":MODEL,
                            "input":input,"encoding_format":format,"dimensions":2,"user":"embedding-caller"})).await;
                        let status = response.status();
                        let text = drain(response.into_body()).await;
                        assert_eq!(status, StatusCode::OK, "{target:?}/{format}: {text}");
                        let reply: Value = serde_json::from_str(&text).unwrap();
                        assert_eq!(reply["data"], upstream["data"]);
                        assert_eq!(reply["usage"], upstream["usage"]);
                        assert_eq!(reply["model"], UPSTREAM_MODEL);
                        assert_eq!(reply, upstream, "embedding responses use the native passthrough contract");
                        let rows = fixture.audits(1).await;
                        assert_identity(&rows[0]);
                        assert_eq!(rows[0].status, "completed");
                        assert_eq!(rows[0].prompt_tokens, Some(2));
                        assert_eq!(rows[0].total_tokens, Some(2));
                        assert_eq!(fixture.remaining().await, 1);
                        let calls = peer.calls.lock().unwrap();
                        assert_eq!(calls.len(), 1);
                        let (path, headers, raw) = &calls[0];
                        assert_eq!(path, "/v1/embeddings");
                        assert_eq!(headers["authorization"], format!("Bearer {}", fixture::access("exact")));
                        assert_eq!(headers["user-agent"], "NeuroGateway/NousPortal");
                        for name in ["cookie", "x-api-key", "x-goog-api-key", "x-management-token", "originator", "chatgpt-account-id"] {
                            assert!(!headers.contains_key(name));
                        }
                        assert_eq!(raw["model"], UPSTREAM_MODEL);
                        assert_eq!(raw["input"], input);
                        assert_eq!(raw["encoding_format"], format);
                        assert_eq!(raw["dimensions"], 2);
                        assert_eq!(raw["user"], "embedding-caller");
                        assert!(raw.get("messages").is_none() && raw.get("stream").is_none());
                    }).await;
                    fixture.finish().await;
                    peer.stop().await;
                }
            }
        }
    }).await;
}
