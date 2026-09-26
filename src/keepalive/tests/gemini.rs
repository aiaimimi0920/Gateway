use super::super::*;

fn make_gemini_canvas_keepalive_request(adapter: &str) -> GatewayKeepaliveEnsureRequest {
    GatewayKeepaliveEnsureRequest {
        project_id: None,
        session_key: None,
        previous_response_id: None,
        credential_id: Some("cred-gemini".to_string()),
        account_name: Some("gemini-main".to_string()),
        provider_account_id: "provider-gemini".to_string(),
        adapter: adapter.to_string(),
        base_url: "https://gemini.google.com".to_string(),
        model: "gemini-2.5-pro".to_string(),
        api_key: Some(String::new()),
        headers: HashMap::new(),
        extra_body: None,
        session_auth: None,
        expires_at: None,
        runtime_state_object_key: None,
    }
}

#[test]
fn infer_keepalive_protocol_family_maps_gemini_modular_surfaces() {
    assert_eq!(
        infer_keepalive_protocol_family(GEMINI_API_MODULAR_ADAPTER),
        GEMINI_GENERATE_CONTENT_FAMILY
    );
    assert_eq!(
        infer_keepalive_protocol_family(GEMINI_WEB_REVERSE_MODULAR_ADAPTER),
        GEMINI_WEB_CHAT_FAMILY
    );
    assert_eq!(
        infer_keepalive_protocol_family(GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER),
        "gemini_canvas"
    );
    assert_eq!(
        infer_keepalive_protocol_family(GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER),
        "gemini_canvas"
    );
}

#[test]
fn build_gemini_canvas_runtime_material_response_accepts_modular_canvas_lines() {
    let future_expiry = Some("2100-01-01T00:00:00Z".to_string());

    let browser_response = build_gemini_canvas_runtime_material_response(
        &make_gemini_canvas_keepalive_request(GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER),
        None,
        future_expiry.clone(),
        Some("credential-runtime/gemini-canvas/browser/storage-state.json".to_string()),
    );
    assert!(browser_response.ready);
    assert_eq!(
        browser_response.runtime_state_object_key.as_deref(),
        Some("credential-runtime/gemini-canvas/browser/storage-state.json")
    );

    let program_response = build_gemini_canvas_runtime_material_response(
        &make_gemini_canvas_keepalive_request(GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER),
        None,
        future_expiry,
        Some("credential-runtime/gemini-canvas/program/storage-state.json".to_string()),
    );
    assert!(program_response.ready);
    assert_eq!(
        program_response.runtime_state_object_key.as_deref(),
        Some("credential-runtime/gemini-canvas/program/storage-state.json")
    );
}

#[test]
fn build_gemini_canvas_runtime_material_response_requires_runtime_state_for_modular_program_line() {
    let response = build_gemini_canvas_runtime_material_response(
        &make_gemini_canvas_keepalive_request(GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER),
        None,
        Some("2100-01-01T00:00:00Z".to_string()),
        None,
    );
    assert!(!response.ready);
    assert_eq!(response.runtime_state_object_key, None);
    assert!(response
        .message
        .as_deref()
        .is_some_and(|message| message.contains("runtimeStateObjectKey")));
}
