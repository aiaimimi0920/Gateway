use super::*;

#[test]
fn preferred_app_endpoint_invoke_helpers_prefer_explicit_contract_fields() {
    let mut payload = make_payload();
    payload.extra_body.as_mut().expect("extra").insert(
        "invokeBaseUrl".to_string(),
        json!("https://canvas-endpoint.example"),
    );
    payload.extra_body.as_mut().expect("extra").insert(
        "musicWsUrl".to_string(),
        json!("wss://canvas-endpoint.example/ws/music"),
    );
    payload.extra_body.as_mut().expect("extra").insert(
        "videoInvokePath".to_string(),
        json!("/v1beta/models/gemini-video:predictLongRunning"),
    );
    let config = relay_config_from_payload(&payload).expect("config");

    assert_eq!(
        preferred_app_endpoint_invoke_base_url(
            "https://generativelanguage.googleapis.com/v1beta",
            &config,
        ),
        "https://canvas-endpoint.example"
    );
    assert_eq!(
        preferred_app_endpoint_music_ws_url("wss://fallback.example/ws", &config),
        "wss://canvas-endpoint.example/ws/music"
    );
    assert_eq!(
        preferred_app_endpoint_video_request_url(
            "https://generativelanguage.googleapis.com/v1beta",
            "gemini-video",
            &config,
        ),
        "https://canvas-endpoint.example/v1beta/models/gemini-video:predictLongRunning"
    );
}

#[test]
fn missing_gemini_canvas_program_app_endpoint_handle_error_matches_contract() {
    let error = missing_gemini_canvas_program_app_endpoint_handle_error(
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
    );
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.provider_name.as_deref(),
        Some(GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER)
    );
    assert_eq!(
        error.code.as_deref(),
        Some("missing_gemini_canvas_program_app_endpoint_handle")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas program-owned app-endpoint lane requires a concrete app handle before invocation."
    );
}

#[test]
fn gemini_canvas_program_direct_http_exhausted_error_matches_contract() {
    let error = gemini_canvas_program_direct_http_exhausted_error(
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
    );
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some(GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER)
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_direct_http_exhausted")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas program direct HTTP fetch exhausted all API key transport attempts."
    );
}

#[test]
fn gemini_canvas_program_direct_http_invalid_json_error_matches_contract() {
    let error = gemini_canvas_program_direct_http_invalid_json_error(
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
        "expected ident",
    );
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some(GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER)
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_direct_http_invalid_json")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas program direct HTTP did not return valid JSON: expected ident"
    );
}

#[test]
fn gemini_canvas_program_direct_http_get_invalid_json_error_matches_contract() {
    let error = gemini_canvas_program_direct_http_get_invalid_json_error(
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
        "expected ident",
    );
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some(GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER)
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_direct_http_invalid_json")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas program direct HTTP GET did not return valid JSON: expected ident"
    );
}

#[test]
fn preferred_app_endpoint_action_prompt_uses_matching_action_contract() {
    let config = relay_config_from_payload(&make_payload()).expect("config");
    let hints = preferred_app_endpoint_action_hints(
        crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Music,
        &config,
    )
    .expect("hints");
    assert_eq!(hints.prompt.as_deref(), Some("A short electronic cue."));
    assert_eq!(hints.duration_seconds, Some(30.0));
    assert_eq!(hints.aspect_ratio, None);
    assert_eq!(
        preferred_app_endpoint_action_prompt(
            crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Music,
            &config,
        )
        .as_deref(),
        Some("A short electronic cue.")
    );
    assert_eq!(
        preferred_app_endpoint_action_prompt(
            crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Video,
            &config,
        ),
        None
    );
    assert_eq!(
        preferred_app_endpoint_action_aspect_ratio(
            crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Video,
            &config,
        ),
        None
    );

    let invoke_contract = preferred_app_endpoint_invoke_contract(
        crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Music,
        "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateMusic",
        None,
        &config,
    );
    assert_eq!(
        invoke_contract.prompt.as_deref(),
        Some("A short electronic cue.")
    );
    assert_eq!(invoke_contract.duration_seconds, Some(30.0));
    assert_eq!(invoke_contract.aspect_ratio, None);
    assert_eq!(
        invoke_contract.music_ws_url.as_deref(),
        Some("wss://canvas-endpoint.example/ws/music")
    );
    assert_eq!(invoke_contract.video_request_url, None);
    assert_eq!(
        invoke_contract.transport_kind.as_deref(),
        Some("canvas_program_ws_candidate")
    );
    assert_eq!(
        invoke_contract.ws_url.as_deref(),
        Some("ws://127.0.0.1:9998")
    );
    assert_eq!(
        invoke_contract.api_style.as_deref(),
        Some("google_generative_language")
    );
    assert_eq!(
        invoke_contract.request_envelope_kind.as_deref(),
        Some("canvas_proxy_request")
    );
    assert_eq!(
        invoke_contract.request_url.as_deref(),
        Some("https://gemini.google.com/_/BardChatUi/data/batchexecute?rpcids=hNvQHb&source-path=%2Fapp%2F4abc4e7577b6149f")
    );
    assert_eq!(
        invoke_contract.request_body.as_deref(),
        Some("f.req=%5Bnull%2C%22A+short+electronic+cue.%22%5D&at=OLD-TOKEN")
    );
    assert_eq!(invoke_contract.request_rpc_id.as_deref(), Some("hNvQHb"));
    assert_eq!(invoke_contract.response_rpc_id.as_deref(), Some("MUAZcd"));
    assert_eq!(
        invoke_contract.source_path.as_deref(),
        Some("/app/4abc4e7577b6149f")
    );
    assert_eq!(
        invoke_contract.model_hint.as_deref(),
        Some("models/veo-3.1-lite-generate-001;backend_beyond")
    );
}

#[test]
fn preferred_app_endpoint_video_request_url_requires_explicit_app_contract() {
    let mut payload = make_payload();
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .remove("invokeBaseUrl");
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .remove("videoInvokePath");
    let config = relay_config_from_payload(&payload).expect("config");
    let invoke_contract = preferred_app_endpoint_invoke_contract(
        crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Video,
        "https://generativelanguage.googleapis.com/v1beta",
        Some("gemini-video"),
        &config,
    );
    assert_eq!(invoke_contract.video_request_url, None);
}

#[test]
fn preferred_app_endpoint_invoke_contract_reads_music_download_target_candidate() {
    let config = relay_config_from_payload(&make_payload()).expect("config");
    let invoke_contract = preferred_app_endpoint_invoke_contract(
        crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Music,
        "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateMusic",
        None,
        &config,
    );
    assert_eq!(
        invoke_contract.asset_url.as_deref(),
        Some("https://contribution.usercontent.google.com/download?filename=sunrise_over_gold.mp4")
    );
    assert_eq!(
        invoke_contract.asset_mime_type.as_deref(),
        Some("video/mp4")
    );
    assert_eq!(invoke_contract.asset_kind.as_deref(), Some("video"));
}

#[test]
fn preferred_app_endpoint_invoke_contract_ignores_video_asset_download_target() {
    let mut payload = make_payload();
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .remove("invokeBaseUrl");
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .remove("videoInvokePath");
    payload.extra_body.as_mut().expect("extra").insert(
        "canvasProgramInvokeContract".to_string(),
        json!({
            "operation": "video",
            "transportKind": "official_video_http_candidate",
            "target": "https://contribution.usercontent.google.com/download?filename=video.mp4",
            "prompt": "A short clip of a glowing cube."
        }),
    );
    let config = relay_config_from_payload(&payload).expect("config");
    let invoke_contract = preferred_app_endpoint_invoke_contract(
        crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Video,
        "https://generativelanguage.googleapis.com/v1beta",
        Some("gemini-video"),
        &config,
    );
    assert_eq!(invoke_contract.video_request_url, None);
}

#[test]
fn build_program_batchexecute_request_from_invoke_contract_refreshes_bootstrap_and_prompt() {
    let config = relay_config_from_payload(&make_payload()).expect("config");
    let invoke_contract = preferred_app_endpoint_invoke_contract(
        crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Music,
        "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateMusic",
        None,
        &config,
    );
    let bootstrap = crate::protocol::gemini::web_reverse::GeminiWebBootstrap {
        access_token: Some("NEW-TOKEN".to_string()),
        build_label: Some("boq-updated".to_string()),
        session_id: Some("1234567890".to_string()),
        language: "zh-CN".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: Some("/app/4abc4e7577b6149f".to_string()),
    };

    let request = build_program_batchexecute_request_from_invoke_contract(
        &invoke_contract,
        &bootstrap,
        "zh-CN",
        Some("A replacement prompt."),
    )
    .expect("request build")
    .expect("request");

    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "rpcids" && value == "hNvQHb"));
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "source-path" && value == "/app/4abc4e7577b6149f"));
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "bl" && value == "boq-updated"));
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "f.sid" && value == "1234567890"));
    assert!(request.query.iter().any(|(key, _)| key == "_reqid"));
    assert!(request
        .form
        .iter()
        .any(|(key, value)| key == "at" && value == "NEW-TOKEN"));
    assert!(request
        .form
        .iter()
        .any(|(_, value)| value.contains("A replacement prompt.")));
}

#[test]
fn build_program_stream_generate_request_from_invoke_contract_preserves_query_and_replaces_prompt()
{
    let mut payload = make_payload();
    payload.extra_body.as_mut().expect("extra").insert(
        "canvasProgramInvokeContract".to_string(),
        json!({
            "operation": "video",
            "transportKind": "program_video_streamgenerate_candidate",
            "requestEnvelopeKind": "page_stream_generate_form",
            "requestUrl": "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate?bl=boq_assistant-bard-web-server_20260507.06_p3&f.sid=-300856583778447260&hl=zh-CN&_reqid=2405763&rt=c",
            "requestBody": "f.req=%5Bnull%2C%22%5B%5B%5C%22ORIGINAL+PROMPT%5C%22%2C0%5D%5D%22%5D&",
            "sourcePath": "/app/656b216d6cd92774",
            "prompt": "ORIGINAL PROMPT"
        }),
    );
    let config = relay_config_from_payload(&payload).expect("config");
    let invoke_contract = preferred_app_endpoint_invoke_contract(
        crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Video,
        "https://generativelanguage.googleapis.com/v1beta",
        Some("veo-3.1-generate-preview"),
        &config,
    );
    let request = build_program_stream_generate_request_from_invoke_contract(
        &invoke_contract,
        Some("REPLACED PROMPT"),
    )
    .expect("request build")
    .expect("request");
    assert_eq!(
        request.url,
        "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate"
    );
    assert!(
        request
            .query
            .iter()
            .any(|(key, value)| key == "bl"
                && value == "boq_assistant-bard-web-server_20260507.06_p3")
    );
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "f.sid" && value == "-300856583778447260"));
    assert!(request.raw_post_data.contains("REPLACED+PROMPT"));
    assert_eq!(
        request.headers.get("content-type").map(String::as_str),
        Some("application/x-www-form-urlencoded;charset=UTF-8")
    );
}

#[test]
fn connected_fetch_mode_helpers_recognize_canvas_page_no_key_variants() {
    assert!(connected_fetch_mode_is_canvas_page_no_key(
        "canvas_page_no_key"
    ));
    assert!(connected_fetch_mode_is_canvas_page_music_no_key(
        "canvas_page_music_no_key"
    ));
    assert!(!connected_fetch_mode_is_canvas_page_no_key("canvas_proxy"));
    assert!(!connected_fetch_mode_is_canvas_page_music_no_key(
        "canvas_preview_music_no_key"
    ));
}

#[test]
fn build_program_stream_generate_request_replaces_only_first_prompt_occurrence() {
    let mut payload = make_payload();
    payload.extra_body = Some(std::collections::HashMap::from([(
        "canvasProgramInvokeContract".to_string(),
        json!({
            "operation": "music",
            "transportKind": "program_music_streamgenerate_candidate",
            "requestEnvelopeKind": "page_stream_generate_form",
            "requestUrl": "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate?bl=boq_assistant-bard-web-server_20260507.06_p3&f.sid=-300856583778447260&hl=zh-CN&_reqid=2405763&rt=c",
            "requestBody": "f.req=%5Bnull%2C%22%5B%5B%5C%22ORIGINAL+PROMPT%5C%22%2C0%5D%2C%5B%5C%22history%5C%22%2C%5C%22ORIGINAL+PROMPT%5C%22%5D%5D%22%5D&",
            "sourcePath": "/app/656b216d6cd92774",
            "prompt": "ORIGINAL PROMPT"
        }),
    )]));
    let config = relay_config_from_payload(&payload).expect("config");
    let invoke_contract = preferred_app_endpoint_invoke_contract(
        crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Music,
        "https://generativelanguage.googleapis.com/v1beta",
        Some("lyria-3-preview"),
        &config,
    );
    let request = build_program_stream_generate_request_from_invoke_contract(
        &invoke_contract,
        Some("REPLACED PROMPT"),
    )
    .expect("request build")
    .expect("request");
    let decoded_form = url::form_urlencoded::parse(request.raw_post_data.as_bytes())
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value.into_owned())
        .expect("f.req");
    assert!(decoded_form.contains("REPLACED PROMPT"));
    assert!(decoded_form.contains("history"));
    assert!(decoded_form.contains("ORIGINAL PROMPT"));
}
