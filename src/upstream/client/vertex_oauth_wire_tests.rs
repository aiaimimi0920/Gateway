//! Final production validation precedes the loopback seam for buffered and aggregated-SSE calls.
use super::{vertex_oauth_test_support as fixture, *};
use crate::protocol::vertex_oauth::{business, VertexTarget};
fn payload(location: &str) -> ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":"gemini_api_compatible","auth_mode":business::AUTH_MODE,
        "base_url":VertexTarget::new("project-a",location,Some("quota-a")).unwrap().base_url().unwrap(),
        "api_key":"current-vertex-access", "auth_header_name":"authorization",
        "headers":{"x-goog-user-project":"quota-a"}})).unwrap()
}
fn request() -> CanonicalRelayRequest {
    openai::normalize_chat_completions(json!({"messages":[{"role":"user","content":"hello"}]}))
        .unwrap()
}
#[tokio::test]
async fn vertex_oauth_actual_buffered_and_sse_bind_current_bearer_and_quota_for_all_target_classes()
{
    let response = json!({"candidates":[{"content":{"role":"model","parts":[{"text":"OK"}]},"finishReason":"STOP"}],
        "usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":1,"totalTokenCount":3}});
    for location in ["global", "us-central1", "us", "eu"] {
        for stream in [false, true] {
            let server =
                fixture::Server::start(200, format!("data: {response}\n\n").into_bytes()).await;
            let mut client = UpstreamClient::new(5);
            client.http = server.client();
            let extras = std::collections::HashMap::from([
                ("authorization".into(), "Bearer stale-caller".into()),
                ("cookie".into(), "caller-cookie".into()),
                ("x-goog-api-key".into(), "wrong-key".into()),
                ("x-goog-user-project".into(), "wrong-project".into()),
            ]);
            fixture::within(&server.origin, async {
                if stream {
                    let UpstreamStreamingResponse::Bytes(output) = client
                        .execute_stream(
                            &payload(location),
                            &request(),
                            "gemini-contract",
                            Some(&extras),
                        )
                        .await
                        .unwrap()
                    else {
                        panic!("aggregated SSE")
                    };
                    let chunks = output
                        .map(|v| String::from_utf8_lossy(&v.unwrap()).into_owned())
                        .collect::<Vec<_>>()
                        .await
                        .join("");
                    assert!(chunks.contains("OK"));
                } else {
                    assert_eq!(
                        client
                            .execute(
                                &payload(location),
                                &request(),
                                "gemini-contract",
                                Some(&extras)
                            )
                            .await
                            .unwrap()
                            .text,
                        "OK"
                    );
                }
            })
            .await;
            {
                let calls = server.calls.lock().unwrap();
                assert_eq!(calls.len(), 1);
                let (uri, headers, body) = &calls[0];
                assert_eq!(uri, &format!("/v1/projects/project-a/locations/{location}/publishers/google/models/gemini-contract:streamGenerateContent?alt=sse"));
                assert_eq!(headers["authorization"], "Bearer current-vertex-access");
                assert_eq!(headers["x-goog-user-project"], "quota-a");
                assert!(!headers.contains_key("cookie") && !headers.contains_key("x-goog-api-key"));
                assert_eq!(body["contents"][0]["parts"][0]["text"], "hello");
            }
            server.stop().await;
        }
    }
}
#[tokio::test]
async fn vertex_oauth_legacy_auth_token_and_protocol_switch_are_rejected_before_network() {
    let server = fixture::Server::start(200, b"unexpected".to_vec()).await;
    let mut client = UpstreamClient::new(5);
    client.http = server.client();
    for (field, value) in [
        ("auth_token", json!("stale-access")),
        ("base_url", json!("https://evil.invalid")),
        (
            "call_flow",
            json!({"call_adapter":"openai","special_adapter":"none","fingerprint_adapter":"auto"}),
        ),
    ] {
        let mut raw = serde_json::to_value(payload("global")).unwrap();
        raw[field] = value;
        let changed = serde_json::from_value(raw).unwrap();
        fixture::within(&server.origin, async {
            assert!(client
                .execute(&changed, &request(), "gemini-contract", None)
                .await
                .is_err());
        })
        .await;
    }
    assert!(server.calls.lock().unwrap().is_empty());
    server.stop().await;
}
