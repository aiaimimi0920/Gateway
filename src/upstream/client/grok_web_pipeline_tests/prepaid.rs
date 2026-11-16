//! Message credit follows terminal outcome, not availability of upstream token usage.
use super::*;
#[path = "prepaid/fixture.rs"]
mod fixture;
use fixture::Prepaid;
#[path = "prepaid/admission.rs"]
mod admission;
#[path = "prepaid/public_tcp.rs"]
mod public_tcp;
#[path = "prepaid/tokens.rs"]
mod tokens;

#[test]
fn grok_web_prepaid_success_charges_one_unknown_usage() {
    run(success);
}
async fn success() {
    for endpoint in ["chat/completions", "responses"] {
        for stream in [false, true] {
            let peer = wire::Server::start(Default::default()).await;
            let fixture = Prepaid::new(&peer).await;
            fixture.balance(2).await;
            let response = fixture.request(&peer, endpoint, stream).await;
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            assert!(String::from_utf8_lossy(&bytes).contains("合成答案"));
            fixture.audit("completed").await;
            fixture.balance(1).await;
            assert_wire(&peer);
            fixture.finish().await;
            peer.finish().await;
        }
    }
}
#[test]
fn grok_web_prepaid_failed_terminal_refunds() {
    run(failed);
}
async fn failed() {
    for endpoint in ["chat/completions", "responses"] {
        for stream in [false, true] {
            let mut reply = wire::Reply::default();
            reply.frames[1] = "<close>".into();
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
            fixture.audit("failed").await;
            fixture.balance(2).await;
            assert_wire(&peer);
            fixture.finish().await;
            peer.finish().await;
        }
    }
}
#[test]
fn grok_web_prepaid_cancel_after_delta_refunds() {
    run(cancelled);
}
async fn cancelled() {
    for endpoint in ["chat/completions", "responses"] {
        let mut reply = wire::Reply::default();
        reply.frames.pop();
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
        })
        .await
        .unwrap();
        fixture.balance(1).await;
        drop(body);
        peer.wait_closed().await;
        fixture.audit("cancelled").await;
        fixture.balance(2).await;
        assert_wire(&peer);
        fixture.finish().await;
        peer.finish().await;
    }
}
