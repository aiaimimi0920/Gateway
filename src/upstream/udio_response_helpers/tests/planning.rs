use super::*;

#[test]
fn prepare_udio_generation_plan_reads_music_contract() {
    let req = music_generation_request(serde_json::json!({
        "prompt": "cinematic synthwave anthem",
        "wait_audio": false,
        "wait_timeout_secs": 12,
        "poll_interval_ms": 750
    }));

    let plan = prepare_udio_generation_plan(&req, crate::protocol::udio::UDIO_DEFAULT_MODEL, true)
        .expect("generation plan");

    assert_eq!(
        plan.output_kind,
        crate::protocol::udio::UdioOutputKind::Music
    );
    assert_eq!(plan.prompt, "cinematic synthwave anthem");
    assert_eq!(plan.wait_audio, false);
    assert_eq!(plan.wait_timeout, Duration::from_secs(15));
    assert_eq!(plan.poll_interval, Duration::from_millis(1000));
    assert_eq!(
        plan.generate_request["gen_params"]["prompt"],
        "cinematic synthwave anthem"
    );
}

#[test]
fn prepare_udio_generation_plan_preserves_missing_browser_runtime_contract() {
    let req = image_generation_request(serde_json::json!({
        "prompt": "cover art"
    }));

    let error =
        prepare_udio_generation_plan(&req, crate::protocol::udio::UDIO_DEFAULT_MODEL, false)
            .expect_err("missing runtime should fail");

    assert_eq!(error.code.as_deref(), Some("missing_udio_browser_runtime"));
    assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
}

#[test]
fn prepare_udio_execution_context_reads_cookie_runtime_contract() {
    let mut payload = make_payload("https://www.udio.com/");
    payload
        .headers
        .insert("cookie".to_string(), "a=b".to_string());
    let req = music_generation_request(serde_json::json!({
        "prompt": "cinematic synthwave anthem",
        "wait_audio": false,
        "wait_timeout_secs": 480,
        "poll_secs": 2
    }));

    let prepared = prepare_udio_execution_context(
        &payload,
        &req,
        crate::protocol::udio::UDIO_DEFAULT_MODEL,
        None,
        Duration::from_secs(60),
    )
    .expect("udio execution context");

    assert_eq!(prepared.base_url, "https://www.udio.com");
    assert_eq!(prepared.runtime_state_object_key, None);
    assert_eq!(
        prepared
            .headers
            .get("cookie")
            .and_then(|value| value.to_str().ok()),
        Some("a=b")
    );
    assert_eq!(prepared.request_timeout, Duration::from_secs(840));
    assert_eq!(
        prepared.generation_plan.output_kind,
        crate::protocol::udio::UdioOutputKind::Music
    );
    assert_eq!(
        prepared.generation_plan.prompt,
        "cinematic synthwave anthem"
    );
    assert!(!prepared.generation_plan.wait_audio);
}

#[test]
fn prepare_udio_execution_context_reads_runtime_state_fallback_contract() {
    let mut payload = make_payload("https://www.udio.com/");
    payload.runtime_state_object_key = Some("runtime-123".to_string());
    let req = image_generation_request(serde_json::json!({
        "prompt": "cover art"
    }));

    let prepared = prepare_udio_execution_context(
        &payload,
        &req,
        crate::protocol::udio::UDIO_DEFAULT_MODEL,
        None,
        Duration::from_secs(45),
    )
    .expect("udio execution context");

    assert_eq!(prepared.base_url, "https://www.udio.com");
    assert_eq!(
        prepared.runtime_state_object_key.as_deref(),
        Some("runtime-123")
    );
    assert_eq!(prepared.request_timeout, Duration::from_secs(600));
    assert_eq!(
        prepared.generation_plan.output_kind,
        crate::protocol::udio::UdioOutputKind::Image
    );
    assert_eq!(prepared.generation_plan.prompt, "cover art");
}
