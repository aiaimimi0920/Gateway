//! Token settlement uses reported usage or retains the observed reservation, never invented usage.
use super::*;

#[test]
fn grok_web_token_prepaid_buffered_success() {
    run(buffered);
}
async fn buffered() {
    for endpoint in ["chat/completions", "responses"] {
        for reported in [false] {
            let reply = wire::Reply::default();
            let peer = wire::Server::start(reply).await;
            let fixture = Prepaid::new(&peer).await;
            fixture.token_mode().await;
            // One text part "合成问题" is 12 UTF-8 bytes: the existing estimator reserves ceil(12/4).
            let reserved =
                (3.0 * fixture.state.config.quota_pre_deduct_estimate_ratio).ceil() as i64;
            assert!((1..10000).contains(&reserved));
            let response = fixture.request(&peer, endpoint, false).await;
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let value: Value = serde_json::from_slice(&bytes).unwrap();
            assert!(String::from_utf8_lossy(&bytes).contains("合成答案"));
            if reported {
                assert_eq!(value["usage"]["total_tokens"], 5);
            } else {
                assert!(value["usage"].is_null());
            }
            fixture.audit("completed").await;
            assert_eq!(
                fixture.tokens().await,
                if reported { 9995 } else { 10000 - reserved }
            );
            assert_wire(&peer);
            fixture.finish().await;
            peer.finish().await;
        }
    }
}
#[test]
fn grok_web_token_prepaid_insufficient_balance_never_dispatches() {
    run(insufficient);
}
async fn insufficient() {
    for endpoint in ["chat/completions", "responses"] {
        for stream in [false, true] {
            for remaining in [0, 1] {
                let peer = wire::Server::start(Default::default()).await;
                let fixture = Prepaid::new(&peer).await;
                fixture.token_remaining(remaining).await;
                let response = fixture.request(&peer, endpoint, stream).await;
                assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
                let bytes = response.into_body().collect().await.unwrap().to_bytes();
                let value: Value = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(value["error"]["code"], "token_balance_exhausted");
                assert!(
                    peer.calls().is_empty(),
                    "insufficient balance must not create upstream work"
                );
                assert_eq!(fixture.tokens().await, remaining);
                tokio::time::timeout(Duration::from_secs(3), async {
                    while Arc::strong_count(&fixture.state) > 1 {
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                let rows = fixture
                    .state
                    .local_runtime
                    .as_ref()
                    .unwrap()
                    .list_audits(&db::RequestAuditFilters::default())
                    .await
                    .unwrap();
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].status, "failed");
                assert_eq!(rows[0].access_key_id.as_deref(), Some(fixture.id.as_str()));
                assert_eq!(rows[0].total_tokens, None);
                fixture.finish().await;
                peer.finish().await;
            }
        }
    }
}

#[test]
fn grok_web_token_prepaid_stream_success_settles_usage_or_keeps_reservation() {
    run(success);
}
async fn success() {
    for endpoint in ["chat/completions", "responses"] {
        for reported in [false] {
            let mut reply = wire::Reply::default();
            // Grok has no upstream token usage in this contract.
            let _ = &mut reply;
            let peer = wire::Server::start(reply).await;
            let fixture = Prepaid::new(&peer).await;
            fixture.token_mode().await;
            assert_eq!(fixture.tokens().await, 10000);
            let response = fixture.request(&peer, endpoint, true).await;
            assert_eq!(response.status(), StatusCode::OK);
            // The pull-driven stream has not reached its terminal event yet.
            let reserved_balance = fixture.tokens().await;
            assert!((1..10000).contains(&reserved_balance));
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let text = String::from_utf8_lossy(&bytes);
            assert!(text.contains("合成答案"));
            assert!(text.contains(if endpoint == "responses" {
                "response.completed"
            } else {
                "[DONE]"
            }));
            fixture.audit("completed").await;
            assert_eq!(
                fixture.tokens().await,
                if reported { 9995 } else { reserved_balance }
            );
            assert_wire(&peer);
            fixture.finish().await;
            peer.finish().await;
        }
    }
}
#[test]
fn grok_web_token_prepaid_failed_terminal_refunds() {
    run(failed);
}
async fn failed() {
    for endpoint in ["chat/completions", "responses"] {
        for stream in [false, true] {
            let mut reply = wire::Reply::default();
            reply.frames[1] = "<close>".into();
            let peer = wire::Server::start(reply).await;
            let fixture = Prepaid::new(&peer).await;
            fixture.token_mode().await;
            let response = fixture.request(&peer, endpoint, stream).await;
            if !stream {
                assert_ne!(response.status(), StatusCode::OK);
            }
            if let Ok(bytes) = response.into_body().collect().await {
                let text = String::from_utf8_lossy(&bytes.to_bytes()).into_owned();
                assert!(!text.contains("[DONE]") && !text.contains("response.completed"));
            }
            fixture.audit("failed").await;
            assert_eq!(fixture.tokens().await, 10000);
            assert_wire(&peer);
            fixture.finish().await;
            peer.finish().await;
        }
    }
}
#[test]
fn grok_web_token_prepaid_cancel_after_delta_refunds() {
    run(cancelled);
}
async fn cancelled() {
    for endpoint in ["chat/completions", "responses"] {
        let mut reply = wire::Reply::default();
        reply.frames.pop();
        let peer = wire::Server::start(reply).await;
        let fixture = Prepaid::new(&peer).await;
        fixture.token_mode().await;
        let response = fixture.request(&peer, endpoint, true).await;
        assert_eq!(response.status(), StatusCode::OK);
        let mut body = response.into_body();
        tokio::time::timeout(Duration::from_secs(3), async {
            let mut text = String::new();
            while !text.contains("合成答案") {
                if let Ok(bytes) = body.frame().await.unwrap().unwrap().into_data() {
                    text.push_str(&String::from_utf8_lossy(&bytes));
                }
                assert!(text.len() < 16384);
            }
            assert!(!text.contains("[DONE]") && !text.contains("response.completed"));
        })
        .await
        .unwrap();
        assert!((1..10000).contains(&fixture.tokens().await));
        drop(body);
        peer.wait_closed().await;
        fixture.audit("cancelled").await;
        assert_eq!(fixture.tokens().await, 10000);
        assert_wire(&peer);
        fixture.finish().await;
        peer.finish().await;
    }
}
