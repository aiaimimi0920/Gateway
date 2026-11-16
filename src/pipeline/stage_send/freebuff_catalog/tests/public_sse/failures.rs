//! Real public SSE failures cannot turn provisional text/finish markers into settled success.
use super::*;
async fn drain(
    mut stream: impl futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Unpin,
) -> (String, bool) {
    let mut bytes = Vec::new();
    let mut failed = false;
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(chunk) => bytes.extend_from_slice(&chunk),
            Err(_) => {
                failed = true;
                break;
            }
        }
    }
    (String::from_utf8(bytes).unwrap(), failed)
}
#[tokio::test]
async fn freebuff_public_chat_sse_bad_tail_unknown_cost_and_finish_rejection_never_succeed_or_replay(
) {
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
        let task = request_stream(&host);
        approve(&host, &task).await;
        let response = task.await.unwrap();
        assert_eq!(response.status(), 200);
        let mut stream = response.bytes_stream();
        let first = stream.next().await.unwrap().unwrap();
        assert!(String::from_utf8(first.to_vec())
            .unwrap()
            .contains("partial"));
        if mode == "finish_rejected" {
            *peer.mode.lock().unwrap() = mode.into();
        }
        peer.live.eof.notify_one();
        let (rest, failed) = tokio::time::timeout(Duration::from_secs(4), drain(stream))
            .await
            .unwrap();
        assert!(
            failed,
            "{mode}: an invalid run must terminate with an error"
        );
        assert!(!rest.contains("[DONE]"));
        assert!(!rest.contains("\"finish_reason\":\"stop\""));
        let finishes = peer.finishes();
        if mode == "public_sse_unknown" {
            assert!(finishes.is_empty(), "unknown is not zero");
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
        assert_eq!(peer.calls("/api/v1/chat/completions"), 1);
        assert_eq!(peer.calls("/api/v1/freebuff/session/admission"), 1);
        assert_eq!(peer.calls("/api/v1/freebuff/session/attempt"), 0);
        assert_eq!(claims(&host).await, 1);
        settled(&host, "failed", 10).await;
        host.close().await;
        peer.server.close().await;
    }
}
#[tokio::test]
async fn freebuff_public_chat_sse_json_fallback_preserves_complete_native_output_and_usage() {
    let peer = peer::Peer::new().await;
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let task = request_stream(&host);
    approve(&host, &task).await;
    let response = task.await.unwrap();
    assert_eq!(response.status(), 200);
    let (body, failed) = drain(response.bytes_stream()).await;
    assert!(!failed);
    assert_eq!(body.matches("native answer").count(), 1);
    assert!(body.contains("native reason"));
    assert!(body.contains("reasoning_details"));
    assert_eq!(body.matches("[DONE]").count(), 1);
    assert!(body.contains("\"finish_reason\":\"stop\""));
    assert_eq!(peer.finishes().len(), 1);
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
    assert_eq!(rows[0].total_tokens, Some(7));
    host.close().await;
    peer.server.close().await;
}
#[tokio::test]
async fn freebuff_public_chat_sse_native_tool_fragments_remain_incremental_under_single_attempt() {
    let peer = peer::Peer::new().await;
    *peer.mode.lock().unwrap() = "public_sse_tools".into();
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let mut body = request();
    body["stream"] = json!(true);
    body["tools"] = json!([{"type":"function","function":{"name":"caller_only","parameters":{"type":"object"}}}]);
    let client = host.client.clone();
    let token = host.token.clone();
    let origin = host.server.origin.clone();
    let task = tokio::spawn(async move {
        client
            .post(format!("{origin}/v1/chat/completions"))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .unwrap()
    });
    approve(&host, &task).await;
    let response = task.await.unwrap();
    assert_eq!(response.status(), 200);
    let mut stream = response.bytes_stream();
    let mut before = String::new();
    while !before.contains("1}") {
        let bytes = stream.next().await.unwrap().unwrap();
        before.push_str(std::str::from_utf8(&bytes).unwrap());
    }
    assert!(before.contains("caller_only"));
    assert!(before.contains("extra_content"));
    assert!(before.contains("native thought"));
    assert!(before.contains("reasoning_details"));
    assert!(!before.contains("[DONE]"));
    assert!(!before.contains("\"finish_reason\":\"tool_calls\""));
    peer.live.eof.notify_one();
    let (rest, failed) = drain(stream).await;
    assert!(!failed);
    assert!(rest.contains("\"finish_reason\":\"tool_calls\""));
    assert!(rest.contains("[DONE]"));
    // Native SDK round(0.25 * (1 + 0.055) * 100), not a chunk sum or token estimate.
    assert_eq!(peer.finishes()[0]["totalCredits"], 26);
    assert_eq!(send_fences::attempts(&host, 1).await, [1]);
    settled(&host, "completed", 9).await;
    host.close().await;
    peer.server.close().await;
}

#[tokio::test]
async fn freebuff_public_chat_sse_known_cost_without_token_counts_keeps_usage_unknown() {
    let peer = peer::Peer::new().await;
    *peer.mode.lock().unwrap() = "public_sse_cost_only".into();
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let task = request_stream(&host);
    approve(&host, &task).await;
    let response = task.await.unwrap();
    let mut stream = response.bytes_stream();
    assert!(stream.next().await.unwrap().is_ok());
    peer.live.eof.notify_one();
    let (rest, failed) = drain(stream).await;
    assert!(!failed);
    assert!(rest.contains("[DONE]"));
    assert!(!rest.contains("prompt_tokens"));
    assert!(!rest.contains("completion_tokens"));
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
    assert!(rows[0].total_tokens.is_none());
    host.close().await;
    peer.server.close().await;
}
