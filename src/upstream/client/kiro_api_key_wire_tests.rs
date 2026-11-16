use super::{kiro_api_key_test_support as fixture, *};

fn payload() -> ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":"kiro_compatible","auth_mode":"kiro_api_key",
        "base_url":"https://runtime.us-east-1.kiro.dev","responses_path":"/","api_key":"ksk_synthetic_only"})).unwrap()
}
#[tokio::test]
async fn kiro_api_key_real_buffered_and_stream_sender_use_static_key_and_cli_wire() {
    for stream in [false, true] {
        let server = fixture::Server::start(
            200,
            "application/vnd.amazon.eventstream",
            fixture::event_frame("OK"),
        )
        .await;
        let payload = payload();
        let model = "claude-sonnet-contract";
        let request=openai::normalize_chat_completions(json!({"model":model,"stream":stream,"messages":[{"role":"system","content":"rules"},{"role":"user","content":"hello"}]})).unwrap();
        let mut client = UpstreamClient::new(3);
        client.http = rquest::Client::builder().no_proxy().build().unwrap();
        let extras = std::collections::HashMap::from([
            ("tokentype".into(), "OAUTH".into()),
            ("host".into(), "foreign.invalid".into()),
            ("cookie".into(), "private".into()),
        ]);
        fixture::within(&server.origin, async {
            if stream {
                let UpstreamStreamingResponse::Http(response) = client
                    .execute_stream(&payload, &request, model, Some(&extras))
                    .await
                    .unwrap()
                else {
                    panic!("native HTTP stream required")
                };
                let output = crate::upstream::kiro_api_key::translate(
                    response
                        .bytes_stream()
                        .map(|v| v.map_err(crate::protocol::stream_error::StreamError::Transport)),
                    model.into(),
                    request.clone(),
                )
                .map(|v| String::from_utf8_lossy(&v.unwrap()).into_owned())
                .collect::<Vec<_>>()
                .await
                .join("");
                assert!(output.contains("OK") && output.contains(model));
                assert!(!output.contains("claude-sonnet-4.5"));
            } else {
                let response = client
                    .execute(&payload, &request, model, Some(&extras))
                    .await
                    .unwrap();
                assert_eq!(response.text, "OK");
                assert_eq!(response.model, model);
            }
        })
        .await;
        let calls = server.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 1);
        let (headers, body) = &calls[0];
        assert_eq!(headers["authorization"], "Bearer ksk_synthetic_only");
        assert_eq!(headers["tokentype"], "API_KEY");
        assert_eq!(headers["content-type"], "application/x-amz-json-1.0");
        assert_eq!(
            headers["x-amz-target"],
            crate::protocol::kiro_api_key::TARGET
        );
        assert_eq!(headers["amz-sdk-request"], "attempt=1; max=1");
        assert!(!headers.contains_key("cookie"));
        assert_ne!(headers["host"], "foreign.invalid");
        assert!(!headers["user-agent"].to_str().unwrap().contains("KiroIDE"));
        for history in body["conversationState"]["history"].as_array().unwrap() {
            if let Some(user) = history.get("userInputMessage") {
                assert_eq!(user["origin"], "KIRO_CLI");
                assert_eq!(user["modelId"], model);
            }
        }
        server.stop().await;
    }
}
#[tokio::test]
async fn kiro_api_key_real_sender_rejects_redirect_json_success_and_auth_errors_without_retry() {
    for (status, content_type) in [
        (307, "application/vnd.amazon.eventstream"),
        (200, "application/json"),
        (401, "application/json"),
        (403, "application/json"),
        (429, "application/json"),
    ] {
        for stream in [false, true] {
            let server = fixture::Server::start(status, content_type, b"{}".to_vec()).await;
            let request = openai::normalize_chat_completions(
                json!({"model":"exact","messages":[{"role":"user","content":"hello"}]}),
            )
            .unwrap();
            let mut client = UpstreamClient::new(3);
            client.http = rquest::Client::builder()
                .no_proxy()
                .redirect(rquest::redirect::Policy::limited(3))
                .build()
                .unwrap();
            fixture::within(&server.origin, async {
                let failed = if stream {
                    client
                        .execute_stream(&payload(), &request, "exact", None)
                        .await
                        .is_err()
                } else {
                    client
                        .execute(&payload(), &request, "exact", None)
                        .await
                        .is_err()
                };
                assert!(failed);
            })
            .await;
            assert_eq!(server.calls.lock().unwrap().len(), 1);
            server.stop().await;
        }
    }
}
#[test]
fn kiro_api_key_final_sender_refuses_late_plan_mutation_before_network() {
    let request =
        openai::normalize_chat_completions(json!({"messages":[{"role":"user","content":"hello"}]}))
            .unwrap();
    let payload = payload();
    let client = UpstreamClient::new(3);
    for kind in 0..4 {
        let mut bad =
            UpstreamClient::build_request_plan(&payload, &request, "exact", false).unwrap();
        match kind {
            0 => bad.url = "https://foreign.invalid/".into(),
            1 => bad.query.push(("leak".into(), "x".into())),
            2 => bad.body = Some(json!({"conversationState":{}})),
            _ => bad.body.as_mut().unwrap()["profileArn"] = json!("foreign-profile"),
        };
        assert!(client
            .send_profiled_plan(&payload, &bad, HeaderMap::new())
            .is_err());
    }
}
#[tokio::test]
async fn kiro_api_key_cli_decoder_preserves_exact_model_for_messages_and_responses() {
    for kind in [EndpointKind::Messages, EndpointKind::Responses] {
        let mut request = openai::normalize_chat_completions(
            json!({"messages":[{"role":"user","content":"hello"}]}),
        )
        .unwrap();
        request.endpoint_kind = kind;
        let source = futures::stream::iter([Ok::<
            _,
            crate::protocol::stream_error::StreamError<rquest::Error>,
        >(bytes::Bytes::from(fixture::event_frame(
            "OK",
        )))]);
        let output = crate::upstream::kiro_api_key::translate(
            source,
            "claude-opus-contract".into(),
            request,
        )
        .map(|v| String::from_utf8_lossy(&v.unwrap()).into_owned())
        .collect::<Vec<_>>()
        .await
        .join("");
        assert!(output.contains("OK") && output.contains("claude-opus-contract"));
        assert!(!output.contains("claude-opus-4.6"));
    }
}
