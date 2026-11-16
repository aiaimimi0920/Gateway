//! Actual buffered/SSE transport to synthetic loopback only, after product validation.
use super::*;
use crate::upstream::client::kiro_api_key_test_support as fixture;
fn payload() -> ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":"kiro_compatible","auth_mode":"kiro_oauth_runtime",
        "base_url":"https://runtime.eu-central-1.kiro.dev","responses_path":"/","api_key":"synthetic-oauth-access",
        "session_auth":{"transport":"bearer"},"extra_body":{"profileArn":"arn:aws:codewhisperer:eu-central-1:123456789012:profile/synthetic"}})).unwrap()
}
#[tokio::test]
async fn kiro_oauth_runtime_actual_sender_preserves_exact_model_profile_and_omits_key_type() {
    for stream in [false, true] {
        let server = fixture::Server::start(
            200,
            "application/vnd.amazon.eventstream",
            fixture::event_frame("OK"),
        )
        .await;
        let request = openai::normalize_chat_completions(json!({"model":"claude-sonnet-contract","stream":stream,"messages":[{"role":"user","content":"hello"}]})).unwrap();
        let mut client = UpstreamClient::new(3);
        client.http = rquest::Client::builder().no_proxy().build().unwrap();
        let headers = HashMap::from([
            ("tokentype".into(), "API_KEY".into()),
            ("cookie".into(), "foreign".into()),
        ]);
        fixture::within(&server.origin, async {
            if stream {
                let UpstreamStreamingResponse::Http(response) = client
                    .execute_stream(
                        &payload(),
                        &request,
                        "claude-sonnet-contract",
                        Some(&headers),
                    )
                    .await
                    .unwrap()
                else {
                    panic!("HTTP required");
                };
                let output = crate::upstream::kiro_oauth_runtime::translate(
                    response
                        .bytes_stream()
                        .map(|v| v.map_err(crate::protocol::stream_error::StreamError::Transport)),
                    "claude-sonnet-contract".into(),
                    request.clone(),
                )
                .map(|v| String::from_utf8(v.unwrap().to_vec()).unwrap())
                .collect::<Vec<_>>()
                .await
                .join("");
                assert!(output.contains("OK") && output.contains("claude-sonnet-contract"));
            } else {
                let result = client
                    .execute(
                        &payload(),
                        &request,
                        "claude-sonnet-contract",
                        Some(&headers),
                    )
                    .await
                    .unwrap();
                assert_eq!(result.text, "OK");
                assert_eq!(result.model, "claude-sonnet-contract");
            }
        })
        .await;
        let calls = server.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 1);
        let (headers, body) = &calls[0];
        assert_eq!(headers["authorization"], "Bearer synthetic-oauth-access");
        assert!(!headers.contains_key("tokentype") && !headers.contains_key("cookie"));
        assert_eq!(
            headers["x-amz-target"],
            crate::protocol::kiro_api_key::TARGET
        );
        assert_eq!(
            body["profileArn"],
            payload().extra_body.unwrap()["profileArn"]
        );
        assert_eq!(
            body["conversationState"]["currentMessage"]["userInputMessage"]["modelId"],
            "claude-sonnet-contract"
        );
        assert!(body["conversationState"]
            .get("agentContinuationId")
            .is_none());
        server.stop().await;
    }
}
#[tokio::test]
async fn kiro_oauth_runtime_http_failures_are_terminal_and_never_return_provider_body() {
    for (status, content_type) in [
        (200, "application/json"),
        (307, "application/json"),
        (401, "application/json"),
        (429, "application/json"),
    ] {
        let server =
            fixture::Server::start(status, content_type, b"private-provider-body".to_vec()).await;
        let request = openai::normalize_chat_completions(
            json!({"messages":[{"role":"user","content":"hello"}]}),
        )
        .unwrap();
        let mut client = UpstreamClient::new(3);
        client.http = rquest::Client::builder().no_proxy().build().unwrap();
        fixture::within(&server.origin, async {
            let error = client
                .execute(&payload(), &request, "exact", None)
                .await
                .unwrap_err();
            assert!(!error.retryable);
            assert!(matches!(
                error.fallback_hint,
                crate::error::FallbackHint::Abort { .. }
            ));
            assert!(!error.message.contains("private-provider-body"));
            assert_eq!(error.provider_name.as_deref(), Some("kiro-oauth"));
        })
        .await;
        assert_eq!(server.calls.lock().unwrap().len(), 1);
        server.stop().await;
    }
}
#[test]
fn kiro_oauth_runtime_final_guard_rejects_late_profile_destination_and_body_mutations() {
    let request =
        openai::normalize_chat_completions(json!({"messages":[{"role":"user","content":"hello"}]}))
            .unwrap();
    let payload = payload();
    let client = UpstreamClient::new(3);
    for kind in 0..4 {
        let mut plan =
            UpstreamClient::build_request_plan(&payload, &request, "exact", false).unwrap();
        match kind {
            0 => plan.url = "https://foreign.invalid/".into(),
            1 => plan.body.as_mut().unwrap()["profileArn"] = json!("foreign"),
            2 => plan.body.as_mut().unwrap()["extra"] = json!("foreign"),
            _ => plan.query.push(("private".into(), "value".into())),
        }
        assert!(client
            .send_profiled_plan(&payload, &plan, HeaderMap::new())
            .is_err());
    }
}
