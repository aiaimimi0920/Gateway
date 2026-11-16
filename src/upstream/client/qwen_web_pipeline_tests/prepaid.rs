//! Message credit follows terminal outcome independently of optional upstream token usage.
use super::*;
#[path = "prepaid/fixture.rs"]
mod fixture;
use fixture::Prepaid;
#[path = "prepaid/admission.rs"]
mod admission;
#[path = "prepaid/concurrent_usage.rs"]
mod concurrent_usage;
#[path = "prepaid/mixed_outcome.rs"]
mod mixed_outcome;
#[path = "prepaid/overage.rs"]
mod overage;
#[path = "prepaid/public_tcp.rs"]
mod public_tcp;
#[path = "prepaid/recovery.rs"]
mod recovery;
#[path = "prepaid/tokens.rs"]
mod tokens;

#[test]
fn qwen_web_prepaid_success_charges_one_with_or_without_usage() {
    run(success);
}
async fn success() {
    for endpoint in ["chat/completions", "responses"] {
        for stream in [false, true] {
            for reported in [false, true] {
                let mut reply = wire::Reply::default();
                if !reported {
                    reply.chunks[1] = bytes::Bytes::from_static(b"data: {\"done\":true}\n\n");
                }
                let peer = wire::Server::start(reply).await;
                let fixture = Prepaid::new(&peer).await;
                fixture.balance(2).await;
                let response = fixture.request(&peer, endpoint, stream).await;
                assert_eq!(response.status(), StatusCode::OK);
                let bytes = response.into_body().collect().await.unwrap().to_bytes();
                let text = String::from_utf8_lossy(&bytes);
                assert!(text.contains("合成答案"));
                if stream {
                    assert!(text.contains(if endpoint == "responses" {
                        "response.completed"
                    } else {
                        "[DONE]"
                    }));
                } else {
                    let value: Value = serde_json::from_slice(&bytes).unwrap();
                    if reported {
                        assert_eq!(value["usage"]["total_tokens"], 5);
                    } else {
                        assert!(value["usage"].is_null());
                    }
                }
                fixture.audit("completed", reported).await;
                fixture.balance(1).await;
                assert_wire(&peer);
                fixture.finish().await;
                peer.finish().await;
            }
        }
    }
}
#[test]
fn qwen_web_prepaid_truncated_terminal_refunds() {
    run(failed);
}
async fn failed() {
    for endpoint in ["chat/completions", "responses"] {
        for stream in [false, true] {
            let mut reply = wire::Reply::default();
            reply.chunks.pop();
            let peer = wire::Server::start(reply).await;
            let fixture = Prepaid::new(&peer).await;
            let response = fixture.request(&peer, endpoint, stream).await;
            if !stream {
                assert_ne!(response.status(), StatusCode::OK);
            }
            if let Ok(bytes) = response.into_body().collect().await {
                let text = String::from_utf8_lossy(&bytes.to_bytes()).into_owned();
                assert!(!text.contains("[DONE]") && !text.contains("response.completed"));
            }
            fixture.audit("failed", false).await;
            fixture.balance(2).await;
            assert_wire(&peer);
            fixture.finish().await;
            peer.finish().await;
        }
    }
}
#[test]
fn qwen_web_prepaid_cancel_after_delta_refunds() {
    run(cancelled);
}
async fn cancelled() {
    for endpoint in ["chat/completions", "responses"] {
        let mut reply = wire::Reply::default();
        reply.chunks.pop();
        reply.hold = true;
        let peer = wire::Server::start(reply).await;
        let fixture = Prepaid::new(&peer).await;
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
        fixture.balance(1).await;
        drop(body);
        peer.wait_closed().await;
        fixture.audit("cancelled", false).await;
        fixture.balance(2).await;
        assert_wire(&peer);
        fixture.finish().await;
        peer.finish().await;
    }
}
