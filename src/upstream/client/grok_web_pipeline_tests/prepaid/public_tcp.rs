//! Normal balance-bearing Key over downstream TCP, including socket cancellation after a delta.
use super::super::tcp::Tcp;
use super::*;
use futures::StreamExt;

#[test]
fn grok_web_prepaid_tcp_success_settles_message_or_tokens() {
    run(success);
}
async fn success() {
    for endpoint in ["chat/completions", "responses"] {
        for stream in [false, true] {
            for tokens in [false, true] {
                let peer = wire::Server::start(Default::default()).await;
                let fixture = Prepaid::new(&peer).await;
                if tokens {
                    fixture.token_mode().await;
                }
                let tcp = Tcp::start_state(fixture.state.clone(), &peer).await;
                let response = tcp.request_token(endpoint, stream, &fixture.token).await;
                assert_eq!(response.status().as_u16(), 200);
                let text = read(response, false).await;
                assert!(text.contains("合成答案"));
                if stream {
                    assert!(text.contains(if endpoint == "responses" {
                        "response.completed"
                    } else {
                        "[DONE]"
                    }));
                } else {
                    let value: Value = serde_json::from_str(&text).unwrap();
                    assert!(value["usage"].is_null());
                }
                tcp.finish().await;
                fixture.audit("completed").await;
                if tokens {
                    // Unknown usage retains the existing single-text reservation, not invented tokens.
                    let reserved =
                        (3.0 * fixture.state.config.quota_pre_deduct_estimate_ratio).ceil() as i64;
                    assert!(reserved > 0 && reserved < 10000);
                    assert_eq!(fixture.tokens().await, 10000 - reserved);
                } else {
                    fixture.balance(1).await;
                }
                assert_wire(&peer);
                fixture.finish().await;
                peer.finish().await;
            }
        }
    }
}
#[test]
fn grok_web_prepaid_tcp_truncation_refunds_message_or_tokens() {
    run(failed);
}
async fn failed() {
    for endpoint in ["chat/completions", "responses"] {
        for stream in [false, true] {
            for tokens in [false, true] {
                let mut reply = wire::Reply::default();
                reply.frames[1] = "<close>".into();
                let peer = wire::Server::start(reply).await;
                let fixture = Prepaid::new(&peer).await;
                if tokens {
                    fixture.token_mode().await;
                }
                let tcp = Tcp::start_state(fixture.state.clone(), &peer).await;
                let response = tcp.request_token(endpoint, stream, &fixture.token).await;
                if !stream {
                    assert_ne!(response.status().as_u16(), 200);
                }
                let text = read(response, stream).await;
                assert!(!text.contains("[DONE]") && !text.contains("response.completed"));
                tcp.finish().await;
                fixture.audit("failed").await;
                if tokens {
                    assert_eq!(fixture.tokens().await, 10000);
                } else {
                    fixture.balance(2).await;
                }
                assert_wire(&peer);
                fixture.finish().await;
                peer.finish().await;
            }
        }
    }
}
#[test]
fn grok_web_prepaid_tcp_disconnect_refunds_message_or_tokens() {
    run(cancelled);
}
async fn cancelled() {
    for endpoint in ["chat/completions", "responses"] {
        for tokens in [false, true] {
            let mut reply = wire::Reply::default();
            reply.frames.pop();
            let peer = wire::Server::start(reply).await;
            let fixture = Prepaid::new(&peer).await;
            if tokens {
                fixture.token_mode().await;
            }
            let tcp = Tcp::start_state(fixture.state.clone(), &peer).await;
            let response = tcp.request_token(endpoint, true, &fixture.token).await;
            assert_eq!(response.status().as_u16(), 200);
            let mut chunks = response.bytes_stream();
            tokio::time::timeout(Duration::from_secs(3), async {
                let mut bytes = Vec::new();
                while !String::from_utf8_lossy(&bytes).contains("合成答案") {
                    bytes.extend_from_slice(&chunks.next().await.unwrap().unwrap());
                    assert!(bytes.len() < 16384);
                }
                let text = String::from_utf8_lossy(&bytes);
                assert!(!text.contains("[DONE]") && !text.contains("response.completed"));
            })
            .await
            .unwrap();
            if tokens {
                assert!((1..10000).contains(&fixture.tokens().await));
            } else {
                fixture.balance(1).await;
            }
            drop(chunks);
            peer.wait_closed().await;
            tcp.finish().await;
            fixture.audit("cancelled").await;
            if tokens {
                assert_eq!(fixture.tokens().await, 10000);
            } else {
                fixture.balance(2).await;
            }
            assert_wire(&peer);
            fixture.finish().await;
            peer.finish().await;
        }
    }
}
async fn read(response: rquest::Response, allow_stream_error: bool) -> String {
    let mut chunks = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = chunks.next().await {
        match chunk {
            Ok(chunk) => bytes.extend_from_slice(&chunk),
            Err(_) if allow_stream_error => break,
            Err(error) => panic!("unexpected body error: {error}"),
        }
        assert!(bytes.len() < 16384);
    }
    String::from_utf8(bytes).unwrap()
}
