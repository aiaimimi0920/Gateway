use super::*;

#[test]
fn build_tts_trigger_request_uses_response_id_payload() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: None,
        build_label: Some("bl-1".to_string()),
        session_id: Some("sid-1".to_string()),
        language: "zh-CN".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: None,
    };

    let request = build_tts_trigger_request("r_e0e4aa76ab2755e3", &bootstrap, "/app").unwrap();
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "rpcids" && value == GEMINI_CANVAS_TTS_TRIGGER_RPCID));
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
    assert_eq!(rpc[0].as_str(), Some(GEMINI_CANVAS_TTS_TRIGGER_RPCID));
    assert_eq!(rpc[1].as_str(), Some("[\"r_e0e4aa76ab2755e3\"]"));
    assert_eq!(rpc[3].as_str(), Some("generic"));
}

#[test]
fn build_music_trigger_request_uses_response_id_payload() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: None,
        build_label: Some("bl-1".to_string()),
        session_id: Some("sid-1".to_string()),
        language: "zh-CN".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: None,
    };

    let request = build_music_trigger_request("r_a7578d122722b7b5", &bootstrap, "/app").unwrap();
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "rpcids" && value == GEMINI_CANVAS_TTS_TRIGGER_RPCID));
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
    assert_eq!(rpc[0].as_str(), Some(GEMINI_CANVAS_TTS_TRIGGER_RPCID));
    assert_eq!(rpc[1].as_str(), Some("[\"r_a7578d122722b7b5\"]"));
    assert_eq!(rpc[3].as_str(), Some("generic"));
}

#[test]
fn infer_tts_export_locale_prefers_detected_english_language() {
    assert_eq!(
        infer_tts_export_locale("Hello from Gemini Canvas!", "zh-CN"),
        "en-CN"
    );
}

#[test]
fn build_tts_audio_export_request_uses_text_and_locale_payload() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: None,
        build_label: Some("bl-1".to_string()),
        session_id: Some("sid-1".to_string()),
        language: "zh-CN".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: None,
    };

    let request = build_tts_audio_export_request(
        "Hello from Gemini Canvas!",
        "en-CN",
        &bootstrap,
        "/app/85cba2bfe36a963e",
    )
    .unwrap();
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "rpcids" && value == GEMINI_CANVAS_TTS_EXPORT_RPCID));
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "source-path" && value == "/app/85cba2bfe36a963e"));

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
    assert_eq!(rpc[0].as_str(), Some(GEMINI_CANVAS_TTS_EXPORT_RPCID));
    let payload = serde_json::from_str::<Vec<Value>>(rpc[1].as_str().unwrap()).unwrap();
    assert!(payload[0].is_null());
    assert_eq!(payload[1].as_str(), Some("Hello from Gemini Canvas!"));
    assert_eq!(payload[2].as_str(), Some("en-CN"));
    assert_eq!(payload[4].as_i64(), Some(2));
    assert_eq!(rpc[3].as_str(), Some("generic"));
}

#[test]
fn extract_audio_from_tts_export_response_decodes_ogg_payload() {
    let frame = serde_json::to_string(&vec![json!([
        "wrb.fr",
        "XqA3Ic",
        "[\"T2dnUw==\"]",
        null,
        null,
        null,
        "generic"
    ])])
    .unwrap();
    let body = format!(")]}}'\n\n{}\n{}\n", frame.len(), frame);
    let audio = extract_audio_from_tts_export_response(&body).unwrap();
    assert_eq!(audio.mime_type, "audio/ogg");
    assert_eq!(audio.bytes, b"OggS");
}

#[test]
fn extract_audio_from_tts_export_response_scans_raw_body_when_frame_length_is_invalid() {
    let frame = serde_json::to_string(&vec![json!([
        "wrb.fr",
        "XqA3Ic",
        "[\"T2dnUw\"]",
        null,
        null,
        null,
        "generic"
    ])])
    .unwrap();
    let body = format!(")]}}'\n\n999999\n{}\n", frame);
    let audio = extract_audio_from_tts_export_response(&body).unwrap();
    assert_eq!(audio.mime_type, "audio/ogg");
    assert_eq!(audio.bytes, b"OggS");
}
