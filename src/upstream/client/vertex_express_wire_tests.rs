//! Actual loopback HTTP bytes through the existing buffered and aggregated-SSE sender.
use super::{vertex_express_test_support as fixture, *};
use crate::protocol::vertex_express as key;

fn payload() -> ProviderAccountPayload {
    serde_json::from_value(
        json!({"adapter":key::ADAPTER,"auth_mode":key::AUTH_MODE,"base_url":key::BASE_URL,
        "api_key":"synthetic-express-key","auth_header_name":"x-goog-api-key"}),
    )
    .unwrap()
}
fn request() -> CanonicalRelayRequest {
    openai::normalize_chat_completions(
        json!({"messages":[{"role":"user","content":"hello"}],"temperature":0}),
    )
    .unwrap()
}
#[tokio::test]
async fn vertex_express_real_buffered_and_aggregated_sse_share_bound_key_path_and_body() {
    let response = json!({"candidates":[{"content":{"role":"model","parts":[{"text":"OK"}]},"finishReason":"STOP"}],
        "usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":1,"totalTokenCount":3}});
    for stream in [false, true] {
        let server =
            fixture::Server::start(200, format!("data: {response}\n\n").into_bytes()).await;
        let mut client = UpstreamClient::new(5);
        client.http = server.client();
        let extras = std::collections::HashMap::from([
            ("authorization".into(), "Bearer caller".into()),
            ("cookie".into(), "caller-cookie".into()),
            ("host".into(), "foreign.invalid".into()),
            ("x-goog-api-key".into(), "caller-key".into()),
            ("x-goog-user-project".into(), "foreign-project".into()),
        ]);
        fixture::within(&server.origin, async {
            if stream {
                let response = client
                    .execute_stream(&payload(), &request(), "gemini-contract", Some(&extras))
                    .await
                    .unwrap();
                let UpstreamStreamingResponse::Bytes(stream) = response else {
                    panic!("expected aggregated Gemini SSE")
                };
                let output = stream
                    .map(|chunk| String::from_utf8_lossy(&chunk.unwrap()).into_owned())
                    .collect::<Vec<_>>()
                    .await
                    .join("");
                assert!(output.contains("OK"));
            } else {
                let response = client
                    .execute(&payload(), &request(), "gemini-contract", Some(&extras))
                    .await
                    .unwrap();
                assert_eq!(response.text, "OK");
                let usage = response.usage.unwrap();
                assert_eq!(usage.prompt_tokens, 2);
                assert_eq!(usage.completion_tokens, 1);
            }
        })
        .await;
        let calls = server.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 1);
        let (uri, headers, body) = &calls[0];
        assert_eq!(
            uri,
            "/v1/publishers/google/models/gemini-contract:streamGenerateContent?alt=sse"
        );
        assert_eq!(headers["x-goog-api-key"], "synthetic-express-key");
        for name in ["authorization", "cookie", "x-goog-user-project"] {
            assert!(!headers.contains_key(name));
        }
        assert_ne!(headers["host"], "foreign.invalid");
        assert_eq!(body["contents"][0]["parts"][0]["text"], "hello");
        assert_eq!(body["generationConfig"]["temperature"], 0);
        assert!(body.get("model").is_none());
        server.stop().await;
    }
}
#[tokio::test]
async fn vertex_express_real_sender_does_not_follow_redirects_or_retry_itself() {
    for status in [307, 401, 403, 429] {
        for stream in [false, true] {
            let server = fixture::Server::start(status, b"private error".to_vec()).await;
            let mut client = UpstreamClient::new(5);
            client.http = server.client();
            fixture::within(&server.origin, async {
                if stream {
                    assert!(client
                        .execute_stream(&payload(), &request(), "gemini-contract", None)
                        .await
                        .is_err());
                } else {
                    assert!(client
                        .execute(&payload(), &request(), "gemini-contract", None)
                        .await
                        .is_err());
                }
            })
            .await;
            assert_eq!(server.calls.lock().unwrap().len(), 1);
            server.stop().await;
        }
    }
}

#[tokio::test]
async fn vertex_express_media_json_binary_and_invalid_model_are_rejected_before_network() {
    let server = fixture::Server::start(200, b"unused".to_vec()).await;
    let mut client = UpstreamClient::new(5);
    client.http = server.client();
    for kind in [
        EndpointKind::ImagesGenerations,
        EndpointKind::AudioSpeech,
        EndpointKind::Embeddings,
    ] {
        let mut req = request();
        req.endpoint_kind = kind;
        let error = client
            .execute_json_passthrough(
                "provider",
                ProviderExecutionMode::DirectHttp,
                &payload(),
                &req,
                "gemini-contract",
                None,
            )
            .await
            .unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("invalid_vertex_express_api_key")
        );
        assert!(client
            .execute_binary_passthrough(&payload(), &req, "gemini-contract", None)
            .await
            .is_err());
        assert!(client
            .execute(&payload(), &req, "gemini-contract", None)
            .await
            .is_err());
        assert!(client
            .execute_stream(&payload(), &req, "gemini-contract", None)
            .await
            .is_err());
    }
    assert!(client
        .execute(&payload(), &request(), "x?key=other", None)
        .await
        .is_err());
    assert!(server.calls.lock().unwrap().is_empty());
    server.stop().await;
}

#[tokio::test]
async fn vertex_express_multiple_requested_candidates_are_rejected_before_spending_quota() {
    let server = fixture::Server::start(200, b"unused".to_vec()).await;
    let mut client = UpstreamClient::new(5);
    client.http = server.client();
    for count in [json!(2), json!(0), json!("2")] {
        let mut req = request();
        req.extra.insert("n".into(), count);
        fixture::within(&server.origin, async {
            assert!(client
                .execute(&payload(), &req, "gemini-contract", None)
                .await
                .is_err());
            assert!(client
                .execute_stream(&payload(), &req, "gemini-contract", None)
                .await
                .is_err());
        })
        .await;
    }
    assert!(server.calls.lock().unwrap().is_empty());
    server.stop().await;
}
