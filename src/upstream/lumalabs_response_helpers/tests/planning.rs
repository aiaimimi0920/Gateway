use super::*;

#[test]
fn prepare_lumalabs_media_plan_builds_image_contract() {
    let payload = make_payload("lumalabs_compatible", "https://app.lumalabs.ai/");
    let req = image_generation_request(serde_json::json!({
        "prompt": "surreal glass flower"
    }));

    let plan = prepare_lumalabs_media_plan(
        &payload,
        &req,
        crate::protocol::lumalabs::LUMALABS_DEFAULT_IMAGE_MODEL,
        Duration::from_secs(30),
    )
    .expect("lumalabs media plan");

    assert_eq!(
        plan.operation,
        crate::protocol::lumalabs::LumalabsMediaOperation::Image
    );
    assert_eq!(plan.prompt, "surreal glass flower");
    assert_eq!(plan.base_url, "https://app.lumalabs.ai");
    assert_eq!(plan.realm_id, "realm-123");
    assert_eq!(plan.media_operation, "image");
    assert_eq!(plan.artifact_field, "image");
    assert_eq!(plan.request_timeout, Duration::from_secs(90));
    assert!(plan.auto_discover_action_type);
    assert_eq!(plan.locale, "zh-CN");
    assert_eq!(plan.action_body["type"], "create_image_uni_1");
}

#[test]
fn prepare_lumalabs_media_plan_preserves_unsupported_image_inputs_contract() {
    let payload = make_payload("lumalabs_compatible", "https://app.lumalabs.ai");
    let req = image_generation_request(serde_json::json!({
        "prompt": "edit this",
        "image": "data:image/png;base64,AAAA"
    }));

    let error = prepare_lumalabs_media_plan(
        &payload,
        &req,
        crate::protocol::lumalabs::LUMALABS_DEFAULT_IMAGE_MODEL,
        Duration::from_secs(30),
    )
    .expect_err("image inputs should fail");

    assert_eq!(
        error.code.as_deref(),
        Some("unsupported_lumalabs_image_inputs")
    );
    assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
}

#[test]
fn prepare_lumalabs_browser_execution_input_reads_contract() {
    let payload = make_payload("lumalabs_compatible", "https://app.lumalabs.ai/");
    let req = image_generation_request(serde_json::json!({
        "prompt": "surreal glass flower"
    }));

    let plan = prepare_lumalabs_media_plan(
        &payload,
        &req,
        crate::protocol::lumalabs::LUMALABS_DEFAULT_IMAGE_MODEL,
        Duration::from_secs(30),
    )
    .expect("lumalabs media plan");
    let prepared = prepare_lumalabs_browser_execution_input(&plan, "sk-test");

    assert_eq!(prepared.base_url, "https://app.lumalabs.ai");
    assert_eq!(prepared.realm_id, "realm-123");
    assert_eq!(prepared.media_operation.as_deref(), Some("image"));
    assert_eq!(prepared.artifact_field, "image");
    assert_eq!(prepared.session_token, "sk-test");
    assert_eq!(prepared.action_body["type"], "create_image_uni_1");
    assert!(prepared.auto_discover_action_type);
    assert_eq!(prepared.locale, "zh-CN");
    assert_eq!(prepared.timeout, Duration::from_secs(90));
}

#[test]
fn prepare_lumalabs_execution_context_reads_session_contract() {
    let payload = make_payload("lumalabs_compatible", "https://app.lumalabs.ai/");
    let req = image_generation_request(serde_json::json!({
        "prompt": "surreal glass flower"
    }));

    let prepared = prepare_lumalabs_execution_context(
        &payload,
        &req,
        crate::protocol::lumalabs::LUMALABS_DEFAULT_IMAGE_MODEL,
        Duration::from_secs(30),
    )
    .expect("lumalabs execution context");

    assert_eq!(prepared.browser_input.session_token, "sk-test");
    assert_eq!(prepared.media_plan.prompt, "surreal glass flower");
    assert_eq!(prepared.browser_input.base_url, "https://app.lumalabs.ai");
    assert_eq!(prepared.browser_input.realm_id, "realm-123");
    assert_eq!(prepared.browser_input.timeout, Duration::from_secs(90));
}

#[test]
fn prepare_lumalabs_execution_context_preserves_image_input_rejection_contract() {
    let payload = make_payload("lumalabs_compatible", "https://app.lumalabs.ai");
    let req = image_generation_request(serde_json::json!({
        "prompt": "edit this",
        "image": "data:image/png;base64,AAAA"
    }));

    let error = prepare_lumalabs_execution_context(
        &payload,
        &req,
        crate::protocol::lumalabs::LUMALABS_DEFAULT_IMAGE_MODEL,
        Duration::from_secs(30),
    )
    .expect_err("image inputs should fail");

    assert_eq!(
        error.code.as_deref(),
        Some("unsupported_lumalabs_image_inputs")
    );
    assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
}
