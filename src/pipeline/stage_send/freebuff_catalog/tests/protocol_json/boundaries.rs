//! Unsupported raw fields/modalities and pending cancellation must cause zero paid traffic.
use super::*;
#[tokio::test]
async fn freebuff_public_json_rejects_lossy_fields_before_catalog_quote_or_purchase() {
    let peer = peer::Peer::new().await;
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let before = peer.hits.lock().unwrap().len();
    for path in [RESPONSES, MESSAGES] {
        for (key, value) in [
            ("stream", json!("true")),
            ("unknown", json!(true)),
            ("codebuff_metadata", json!({"runId":"forged"})),
        ] {
            let mut body = input(path);
            body[key] = value;
            let (status, error) = host.public_at(path, body).await.unwrap();
            assert_eq!(status, 400, "{path} {key}: {error}");
        }
    }
    for (path, key, value) in [
        (RESPONSES, "previous_response_id", json!("foreign")),
        (RESPONSES, "store", json!(true)),
        (
            RESPONSES,
            "reasoning",
            json!({"effort":"high","summary":"auto"}),
        ),
        (
            RESPONSES,
            "input",
            json!([{"role":"user","content":[{"type":"input_text","text":"x","cache_control":{}}]}]),
        ),
        (
            RESPONSES,
            "input",
            json!([{"role":"user","content":[{"type":"input_image","image_url":"https://invalid.test"}]}]),
        ),
        (
            RESPONSES,
            "input",
            json!([{"type":null,"role":"user","content":"x"}]),
        ),
        (RESPONSES, "tools", json!([{"type":"web_search"}])),
        (
            MESSAGES,
            "thinking",
            json!({"type":"enabled","budget_tokens":100}),
        ),
        (
            MESSAGES,
            "system",
            json!([{"type":"text","text":"x","cache_control":{"type":"ephemeral"}}]),
        ),
        (
            MESSAGES,
            "messages",
            json!([{"role":"user","content":[{"type":"tool_result","tool_use_id":"c1","content":"error","is_error":true}]}]),
        ),
        (
            MESSAGES,
            "messages",
            json!([{"role":"assistant","content":[{"type":"thinking","thinking":"private","signature":"foreign"}]}]),
        ),
        (
            MESSAGES,
            "tools",
            json!([{"name":"lookup","input_schema":{},"cache_control":{}}]),
        ),
    ] {
        let mut body = input(path);
        body[key] = value;
        let (status, error) = host.public_at(path, body).await.unwrap();
        assert_eq!(status, 400, "{path} {key}: {error}");
    }
    assert_eq!(peer.hits.lock().unwrap().len(), before);
    assert_eq!(claims(&host).await, 0);
    assert_eq!(host.balance().await, 10);
    host.close().await;
    peer.server.close().await;
}
#[tokio::test]
async fn freebuff_public_json_pending_cancel_never_buys_or_runs() {
    for path in [RESPONSES, MESSAGES] {
        let peer = peer::Peer::new().await;
        let host = Box::pin(Host::new(&peer)).await;
        host.seed_device(&peer).await;
        let grant = host.grant().await;
        let task = host.public_at(path, input(path));
        let view = host.pending(&grant, &task).await;
        assert_eq!(host.decision(&grant, &view, "cancel").await, 200);
        assert_eq!(task.await.unwrap().0, 409);
        assert_eq!(peer.calls("/api/v1/freebuff/session/admission"), 0);
        assert_eq!(peer.calls("/api/v1/chat/completions"), 0);
        assert!(peer.finishes().is_empty());
        assert_eq!(claims(&host).await, 0);
        settled(&host, "failed", 10).await;
        host.close().await;
        peer.server.close().await;
    }
}
