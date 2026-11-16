//! Provisional protocol events cannot turn a bad native tail, unknown cost or rejected FINISH green.
use super::*;
async fn drain(
    mut stream: impl futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Unpin,
) -> (String, bool) {
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(chunk) => bytes.extend_from_slice(&chunk),
            Err(_) => return (String::from_utf8(bytes).unwrap(), true),
        }
    }
    (String::from_utf8(bytes).unwrap(), false)
}
#[tokio::test]
async fn freebuff_public_protocol_sse_bad_tail_unknown_cost_and_rejected_finish_never_replay() {
    for path in PATHS {
        for mode in ["public_sse_bad", "public_sse_unknown", "finish_rejected"] {
            let peer = peer::Peer::new().await;
            *peer.mode.lock().unwrap() = if mode == "finish_rejected" {
                "public_sse_known"
            } else {
                mode
            }
            .into();
            let host = Box::pin(Host::new(&peer)).await;
            host.seed_device(&peer).await;
            let task = request(&host, path, input(path));
            public_sse::approve(&host, &task).await;
            let response = task.await.unwrap();
            assert_eq!(response.status(), 200);
            let mut stream = response.bytes_stream();
            assert!(std::str::from_utf8(&stream.next().await.unwrap().unwrap())
                .unwrap()
                .contains("partial"));
            if mode == "finish_rejected" {
                *peer.mode.lock().unwrap() = mode.into();
            }
            peer.live.eof.notify_one();
            let (rest, failed) = tokio::time::timeout(Duration::from_secs(4), drain(stream))
                .await
                .unwrap();
            assert!(failed, "{path} {mode}: {rest}");
            assert!(!rest.contains(terminal(path)));
            assert_eq!(peer.calls("/api/v1/chat/completions"), 1);
            assert_eq!(peer.calls("/api/v1/freebuff/session/admission"), 1);
            assert_eq!(peer.calls("/api/v1/freebuff/session/attempt"), 0);
            let finishes = peer.finishes();
            if mode == "public_sse_unknown" {
                assert!(finishes.is_empty());
            } else {
                assert_eq!(finishes.len(), 1);
                assert_eq!(finishes[0]["totalCredits"], 13);
                assert_eq!(
                    finishes[0]["status"],
                    if mode == "public_sse_bad" {
                        "failed"
                    } else {
                        "completed"
                    }
                );
            }
            settled(&host, "failed", 10).await;
            host.close().await;
            peer.server.close().await;
        }
    }
}
#[tokio::test]
async fn freebuff_public_protocol_sse_tools_are_incremental_and_keep_the_original_call_id() {
    for path in PATHS {
        let peer = peer::Peer::new().await;
        *peer.mode.lock().unwrap() = "public_sse_tools".into();
        let host = Box::pin(Host::new(&peer)).await;
        host.seed_device(&peer).await;
        let mut body = input(path);
        body["tools"] = if path == PATHS[0] {
            json!([{"type":"function","name":"caller_only","parameters":{"type":"object"}}])
        } else {
            json!([{"name":"caller_only","input_schema":{"type":"object"}}])
        };
        let task = request(&host, path, body);
        public_sse::approve(&host, &task).await;
        let response = task.await.unwrap();
        assert_eq!(response.status(), 200);
        let mut stream = response.bytes_stream();
        let mut before = String::new();
        while !before.contains("1}") {
            before.push_str(std::str::from_utf8(&stream.next().await.unwrap().unwrap()).unwrap());
        }
        assert!(before.contains("caller_only"));
        assert!(before.contains("tool-1"));
        assert!(before.contains("extra_content"));
        assert!(!before.contains(terminal(path)));
        assert!(peer.finishes().is_empty());
        peer.live.eof.notify_one();
        let (rest, failed) = drain(stream).await;
        assert!(!failed);
        assert!(rest.contains(terminal(path)));
        assert_eq!(peer.finishes()[0]["totalCredits"], 26);
        assert_eq!(send_fences::attempts(&host, 1).await, [1]);
        settled(&host, "completed", 9).await;
        host.close().await;
        peer.server.close().await;
    }
}
#[tokio::test]
async fn freebuff_public_protocol_sse_json_fallback_and_cost_only_never_fabricate_token_counts() {
    for path in PATHS {
        for mode in ["json_tools", "public_sse_cost_only"] {
            let peer = peer::Peer::new().await;
            *peer.mode.lock().unwrap() = mode.into();
            let host = Box::pin(Host::new(&peer)).await;
            host.seed_device(&peer).await;
            let task = request(&host, path, input(path));
            public_sse::approve(&host, &task).await;
            let response = task.await.unwrap();
            assert_eq!(response.status(), 200);
            peer.live.eof.notify_one();
            let (body, failed) = drain(response.bytes_stream()).await;
            assert!(!failed, "{body}");
            assert!(body.contains(terminal(path)));
            assert!(!body.contains("thinking_delta"));
            assert!(!body.contains("signature_delta"));
            if mode == "json_tools" {
                assert!(body.contains("lookup"));
                assert!(body.contains("out-1"));
                assert!(body.contains("native reason"));
                assert!(body.contains("input_tokens"));
            } else {
                assert!(!body.contains("input_tokens"));
            }
            assert_eq!(peer.calls("/api/v1/chat/completions"), 1);
            assert_eq!(peer.finishes()[0]["totalCredits"], 13);
            settled(&host, "completed", 9).await;
            let rows = host
                .state
                .local_runtime
                .as_ref()
                .unwrap()
                .list_audits(&crate::db::RequestAuditFilters::default())
                .await
                .unwrap();
            assert_eq!(
                rows[0].total_tokens,
                if mode == "json_tools" { Some(7) } else { None }
            );
            host.close().await;
            peer.server.close().await;
        }
    }
}
#[tokio::test]
async fn freebuff_public_messages_sse_unrepresentable_array_tool_input_fails_before_paid_finish() {
    let peer = peer::Peer::new().await;
    *peer.mode.lock().unwrap() = "json_array_args".into();
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let task = request(&host, PATHS[1], input(PATHS[1]));
    public_sse::approve(&host, &task).await;
    let response = task.await.unwrap();
    assert_eq!(response.status(), 502);
    assert_eq!(peer.calls("/api/v1/chat/completions"), 1);
    assert_eq!(peer.finishes()[0]["status"], "failed");
    assert_eq!(peer.finishes()[0]["totalCredits"], 13);
    settled(&host, "failed", 10).await;
    host.close().await;
    peer.server.close().await;
}
