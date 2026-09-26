use super::*;

#[test]
fn build_text_mode_selection_preflight_request_targets_last_selected_mode() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: None,
        build_label: Some("bl-1".to_string()),
        session_id: Some("sid-1".to_string()),
        language: "zh-CN".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: None,
    };

    let request =
        build_text_mode_selection_preflight_request(&bootstrap, "/share/fe24c455a570").unwrap();
    assert!(request.query.iter().any(|(key, value)| {
        key == "rpcids" && value == GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID
    }));
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "source-path" && value == "/share/fe24c455a570"));

    let f_req = request
        .form
        .iter()
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value)
        .unwrap();
    let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
    let rpc = outer[0]
        .as_array()
        .and_then(|group| group.first())
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(
        rpc[0].as_str(),
        Some(GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID)
    );
    let payload = serde_json::from_str::<Vec<Value>>(rpc[1].as_str().unwrap()).unwrap();
    let state = payload[0].as_array().unwrap();
    assert_eq!(state.len(), 100);
    assert_eq!(
        state[99].as_str(),
        Some(GEMINI_CANVAS_TEXT_LAST_SELECTED_MODE_ID)
    );
    let keys = payload[1].as_array().unwrap();
    let key_group = keys[0].as_array().unwrap();
    assert_eq!(key_group[0].as_str(), Some("last_selected_mode_id_on_web"));
}

#[test]
fn build_mode_selection_preflight_request_allows_media_selected_id() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: None,
        build_label: Some("bl-1".to_string()),
        session_id: Some("sid-1".to_string()),
        language: "zh-CN".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: None,
    };

    let request = build_mode_selection_preflight_request(
        &bootstrap,
        "/app/abc123",
        GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID,
    )
    .unwrap();
    assert!(request.query.iter().any(|(key, value)| {
        key == "rpcids" && value == GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID
    }));
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "source-path" && value == "/app/abc123"));

    let f_req = request
        .form
        .iter()
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value)
        .unwrap();
    let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
    let rpc = outer[0]
        .as_array()
        .and_then(|group| group.first())
        .and_then(Value::as_array)
        .unwrap();
    let payload = serde_json::from_str::<Vec<Value>>(rpc[1].as_str().unwrap()).unwrap();
    let state = payload[0].as_array().unwrap();
    assert_eq!(state.len(), 100);
    assert_eq!(
        state[99].as_str(),
        Some(GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID)
    );
}

#[test]
fn build_media_operation_selection_preflight_request_matches_captured_shape() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: None,
        build_label: Some("bl-1".to_string()),
        session_id: Some("sid-1".to_string()),
        language: "zh-CN".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: None,
    };

    let request =
        build_media_operation_selection_preflight_request(&bootstrap, "/app/abc123", 11).unwrap();
    assert!(request.query.iter().any(|(key, value)| {
        key == "rpcids" && value == GEMINI_CANVAS_MEDIA_OPERATION_SELECTION_RPCID
    }));
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "source-path" && value == "/app/abc123"));

    let f_req = request
        .form
        .iter()
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value)
        .unwrap();
    let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
    let rpc = outer[0]
        .as_array()
        .and_then(|group| group.first())
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(
        rpc[0].as_str(),
        Some(GEMINI_CANVAS_MEDIA_OPERATION_SELECTION_RPCID)
    );
    let payload = serde_json::from_str::<Vec<Value>>(rpc[1].as_str().unwrap()).unwrap();
    assert_eq!(Value::Array(payload), json!([[[1, 11], [2, 11], [6, 11]]]));
}

#[test]
fn build_media_operation_selection_preflight_request_maps_image_mode_to_captured_index() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: None,
        build_label: Some("bl-1".to_string()),
        session_id: Some("sid-1".to_string()),
        language: "zh-CN".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: None,
    };

    let request = build_media_operation_selection_preflight_request(
        &bootstrap,
        "/app",
        GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
    )
    .unwrap();
    let f_req = request
        .form
        .iter()
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value)
        .unwrap();
    let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
    let rpc = outer[0]
        .as_array()
        .and_then(|group| group.first())
        .and_then(Value::as_array)
        .unwrap();
    let payload = serde_json::from_str::<Vec<Value>>(rpc[1].as_str().unwrap()).unwrap();
    assert_eq!(Value::Array(payload), json!([[[1, 11], [2, 11], [6, 11]]]));
}

#[test]
fn build_text_bootstrap_preflight_request_uses_empty_payload() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: Some("AT".to_string()),
        build_label: Some("bl-1".to_string()),
        session_id: Some("sid-1".to_string()),
        language: "zh-CN".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: None,
    };

    let request =
        build_text_bootstrap_preflight_request(&bootstrap, "/share/fe24c455a570").unwrap();
    assert!(request
        .query
        .iter()
        .any(|(key, value)| { key == "rpcids" && value == GEMINI_CANVAS_TEXT_BOOTSTRAP_RPCID }));
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "source-path" && value == "/share/fe24c455a570"));

    let f_req = request
        .form
        .iter()
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value)
        .unwrap();
    let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
    let rpc = outer[0]
        .as_array()
        .and_then(|group| group.first())
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(rpc[0].as_str(), Some(GEMINI_CANVAS_TEXT_BOOTSTRAP_RPCID));
    assert_eq!(rpc[1].as_str(), Some("[]"));
    assert_eq!(rpc[3].as_str(), Some("generic"));
    assert_eq!(
        request
            .form
            .iter()
            .find(|(key, _)| key == "at")
            .map(|(_, value)| value.as_str()),
        Some("AT")
    );
}

#[test]
fn build_text_state_preflight_request_uses_canvas_state_payload() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: None,
        build_label: Some("bl-1".to_string()),
        session_id: Some("sid-1".to_string()),
        language: "zh-CN".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: None,
    };

    let request = build_text_state_preflight_request(&bootstrap, "/app").unwrap();
    assert!(request.query.iter().any(|(key, value)| {
        key == "rpcids" && value == GEMINI_CANVAS_TEXT_STATE_PREFLIGHT_RPCID
    }));
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "source-path" && value == "/app"));

    let f_req = request
        .form
        .iter()
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value)
        .unwrap();
    let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
    let rpc = outer[0]
        .as_array()
        .and_then(|group| group.first())
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(
        rpc[0].as_str(),
        Some(GEMINI_CANVAS_TEXT_STATE_PREFLIGHT_RPCID)
    );
    assert_eq!(rpc[1].as_str(), Some("[[null,null,null,null,true]]"));
    assert_eq!(rpc[3].as_str(), Some("generic"));
}

#[test]
fn build_image_state_keys_preflight_request_matches_captured_shape() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: Some("AT".to_string()),
        build_label: Some("boq".to_string()),
        session_id: Some("123".to_string()),
        language: "zh-CN".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: None,
    };

    let request = build_image_state_keys_preflight_request(&bootstrap, "/app").unwrap();
    let f_req = request
        .form
        .iter()
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value)
        .unwrap();
    let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
    let rpc = outer[0]
        .as_array()
        .and_then(|group| group.first())
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(
        rpc[0].as_str(),
        Some(GEMINI_CANVAS_TEXT_STATE_PREFLIGHT_RPCID)
    );
    let payload = serde_json::from_str::<Value>(rpc[1].as_str().unwrap()).unwrap();
    assert_eq!(
        payload.pointer("/0/0/0").and_then(Value::as_str),
        Some("adaptive_device_responses_enabled")
    );
    assert_eq!(
        payload
            .pointer(
                format!(
                    "/0/0/{}",
                    GEMINI_CANVAS_IMAGE_STATE_KEYS_PREFLIGHT_KEYS.len() - 1
                )
                .as_str()
            )
            .and_then(Value::as_str),
        Some("zs_student_aip_banner_dismissal_count")
    );
}
