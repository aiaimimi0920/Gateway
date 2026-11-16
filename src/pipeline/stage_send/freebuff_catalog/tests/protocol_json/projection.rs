//! Projection is inside original FINISH ownership; invalid data cannot become a completed run.
use super::*;
async fn reply(path: &'static str, mode: &str) -> (u16, Value, Value) {
    let peer = peer::Peer::new().await;
    *peer.mode.lock().unwrap() = mode.into();
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let grant = host.grant().await;
    let task = host.public_at(path, input(path));
    let view = host.pending(&grant, &task).await;
    assert_eq!(host.decision(&grant, &view, "approve").await, 200);
    let (status, body) = task.await.unwrap();
    assert_eq!(peer.calls("/api/v1/freebuff/session/admission"), 1);
    assert_eq!(peer.calls("/api/v1/chat/completions"), 1);
    let finishes = peer.finishes();
    assert_eq!(
        finishes.len(),
        1,
        "known native costs must still be closed out"
    );
    assert_eq!(finishes[0]["totalCredits"], 13);
    assert_eq!(claims(&host).await, 1);
    if status == 200 {
        settled(&host, "completed", 9).await;
    } else {
        settled(&host, "failed", 10).await;
    }
    host.close().await;
    peer.server.close().await;
    (status, body, finishes[0].clone())
}
#[tokio::test]
async fn freebuff_public_json_tools_keep_arguments_extensions_and_literal_text() {
    for path in [RESPONSES, MESSAGES] {
        let (status, body, finish) = reply(path, "json_tools").await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(finish["status"], "completed");
        let parts = body[if path == RESPONSES {
            "output"
        } else {
            "content"
        }]
        .as_array()
        .unwrap();
        assert_eq!(
            parts.len(),
            2,
            "literal XML must not be parsed into another tool"
        );
        let text = if path == RESPONSES {
            &parts[0]["content"][0]["text"]
        } else {
            &parts[0]["text"]
        };
        assert_eq!(text, "literal <tool_calls>not executable</tool_calls>");
        assert_eq!(parts[1]["name"], "lookup");
        if path == RESPONSES {
            assert_eq!(parts[1]["call_id"], "out-1");
            assert_eq!(parts[1]["arguments"], "{ \"s\": \"&amp;\" }");
        } else {
            assert_eq!(parts[1]["input"], json!({"s":"&amp;"}));
            assert_eq!(
                body["gateway_freebuff"]["tool_calls"][0]["function"]["arguments"],
                "{ \"s\": \"&amp;\" }"
            );
        }
        assert_eq!(
            body["gateway_freebuff"]["tool_calls"][0]["extra_content"],
            json!({"opaque":"keep"})
        );
    }
}
#[tokio::test]
async fn freebuff_public_json_unrepresentable_native_output_finishes_failed_without_replay() {
    for (path, mode) in [
        (RESPONSES, "json_multiple"),
        (MESSAGES, "json_multiple"),
        (MESSAGES, "json_array_args"),
        (RESPONSES, "json_missing_id"),
        (MESSAGES, "json_missing_id"),
    ] {
        let (status, body, finish) = reply(path, mode).await;
        assert_eq!(status, 502, "{path} {mode}: {body}");
        assert_eq!(
            finish["status"], "failed",
            "projection failure must be known before FINISH"
        );
    }
}
#[tokio::test]
async fn freebuff_public_json_partial_tokens_stay_partial_while_reported_cost_finishes() {
    for path in [RESPONSES, MESSAGES] {
        let (status, body, finish) = reply(path, "json_partial").await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(finish["status"], "completed");
        assert_eq!(body["usage"]["input_tokens"], 3);
        assert!(body["usage"].get("output_tokens").is_none());
        assert!(body["usage"].get("input_tokens_details").is_none());
        assert_eq!(body["gateway_freebuff"]["completion"]["usage"]["cost"], 0.1);
    }
}
