use super::*;

#[cfg(feature = "line-suno-web-reverse-api")]
#[tokio::test]
async fn execute_suno_image_inputs_rejected_before_send() {
    let payload = make_payload("suno_compatible", "https://studio-api-prod.suno.com");
    let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
    req.requested_model = Some("chirp-v3-5".to_string());
    req.raw_body = json!({
        "prompt": "cover art",
        "image": "data:image/png;base64,aGVsbG8=",
    });
    let client = UpstreamClient::new(5);
    let err = client
        .execute_suno_media(&payload, &req, "chirp-v3-5", None)
        .await
        .expect_err("uploaded image inputs should be rejected before send");
    assert_eq!(err.http_status, Some(400));
    assert_eq!(err.code.as_deref(), Some("unsupported_suno_image_inputs"));
}

#[cfg(feature = "line-suno-web-reverse-api")]
#[tokio::test]
async fn execute_suno_video_multiple_outputs_rejected_before_send() {
    let payload = make_payload("suno_compatible", "https://studio-api-prod.suno.com");
    let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
    req.requested_model = Some("chirp-v3-5".to_string());
    req.raw_body = json!({
        "prompt": "cinematic stage clip",
        "n": 2,
    });
    let client = UpstreamClient::new(5);
    let err = client
        .execute_suno_media(&payload, &req, "chirp-v3-5", None)
        .await
        .expect_err("multi-video requests should be rejected before send");
    assert_eq!(err.http_status, Some(400));
    assert_eq!(err.code.as_deref(), Some("unsupported_suno_video_count"));
}

// ── resolve_model ─────────────────────────────────────────────────────

// ── network tests (ignored) ───────────────────────────────────────────

#[tokio::test]
#[ignore = "requires network access"]
async fn execute_real_openai_request() {
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
    let mut payload = make_payload("openai_compatible", "https://api.openai.com");
    payload.api_key = api_key;

    let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
    let client = UpstreamClient::new(30);
    let result = client.execute(&payload, &req, "gpt-4o-mini", None).await;
    assert!(result.is_ok(), "{result:?}");
}
