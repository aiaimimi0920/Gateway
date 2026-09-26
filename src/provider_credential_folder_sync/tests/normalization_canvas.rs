//! Canvas runtime handles, share provenance and raw-source contracts.
use super::super::normalization::normalize_import_payload;
use super::build_test_provider_account;
use serde_json::Value;

#[test]
fn normalize_gemini_canvas_import_payload_converts_manual_export_output() {
    let mut provider = build_test_provider_account(
        "Gemini Canvas Images",
        "gemini_canvas_compatible",
        "gemini_canvas_images",
        "gemini_canvas",
        "https://gemini.google.com",
        Some("web_reverse_api"),
        Some("browser_challenge"),
    );
    provider.service_provider_key = "gemini_platform".to_string();
    provider.service_provider_label = "Gemini Platform".to_string();

    let payload = normalize_import_payload(
        "gemini-canvas-images",
        &provider,
        serde_json::json!({
            "runtimeStateObjectKey": "credential-runtime/gemini-canvas/demo/storage-state.json",
            "suggestedShareId": "share-123",
            "accountName": "Gemini Canvas Main",
            "baseUrl": "https://gemini.google.com",
            "credentialMaterialKey": "gemini-canvas-browser-main",
            "googleApiKey": "AIzaPrimaryKey123",
            "apiKeys": ["AIzaPrimaryKey123", "AIzaSecondaryKey456"]
        }),
        Some("browser_state"),
    )
    .expect("normalized gemini canvas payload");

    assert_eq!(
        payload.get("runtimeStateObjectKey").and_then(Value::as_str),
        Some("credential-runtime/gemini-canvas/demo/storage-state.json")
    );
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|extra| extra.get("shareId"))
            .and_then(Value::as_str),
        Some("share-123")
    );
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|extra| extra.get("googleApiKey"))
            .and_then(Value::as_str),
        Some("AIzaPrimaryKey123")
    );
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|extra| extra.get("apiKeys"))
            .and_then(Value::as_array)
            .map(|values| { values.iter().filter_map(Value::as_str).collect::<Vec<_>>() }),
        Some(vec!["AIzaPrimaryKey123", "AIzaSecondaryKey456"])
    );
    assert_eq!(
        payload
            .get("credentialMaterialKind")
            .and_then(Value::as_str),
        Some("browser_state")
    );
    assert_eq!(
        payload.get("credentialMaterialKey").and_then(Value::as_str),
        Some("gemini-canvas-browser-main")
    );
    assert_eq!(
        payload.get("providerSurfaceKey").and_then(Value::as_str),
        Some("gemini-canvas-images")
    );
    assert_eq!(
        payload.get("serviceProviderKey").and_then(Value::as_str),
        Some("gemini_platform")
    );
}

#[test]
fn normalize_gemini_canvas_import_payload_preserves_program_handle_provenance() {
    let mut provider = build_test_provider_account(
        "Gemini Canvas Program Relay",
        "gemini_canvas_program_web_reverse_compatible",
        "gemini_canvas_images",
        "gemini_canvas_program_web_reverse_modular",
        "https://gemini.google.com",
        Some("web_reverse_api"),
        Some("browser_backed"),
    );
    provider.service_provider_key = "gemini_platform".to_string();
    provider.service_provider_label = "Gemini Platform".to_string();

    let payload = normalize_import_payload(
        "gemini-canvas-program-relay",
        &provider,
        serde_json::json!({
            "runtimeStateObjectKey": "credential-runtime/gemini-canvas/program/storage-state.json",
            "shareUrl": "https://gemini.google.com/share/fe24c455a570",
            "shareId": "fe24c455a570",
            "canvasProgramUrl": "https://gemini.google.com/app/4abc4e7577b6149f",
            "beforeUrl": "https://gemini.google.com/canvas",
            "finalUrl": "https://gemini.google.com/app",
            "shareFollowKind": "same_page",
            "pageUrl": "https://gemini.google.com/app",
            "appPath": "/app/4abc4e7577b6149f",
            "programId": "4abc4e7577b6149f",
            "conversationId": "c_4abc4e7577b6149f",
            "responseId": "r_1d583cfb0fa7aee4",
            "invokeBaseUrl": "https://canvas-endpoint.example",
            "musicWsUrl": "wss://canvas-endpoint.example/ws/music",
            "videoInvokePath": "/v1beta/models/gemini-video:predictLongRunning",
            "canvasProgramAction": "music_generation",
            "canvasProgramActionInput": "{'prompt': 'A short electronic cue.', 'duration_seconds': 30}",
            "canvasProgramInvokeContract": {
                "operation": "music",
                "transportKind": "official_music_ws_candidate",
                "target": "wss://canvas-endpoint.example/ws/music",
                "actionName": "music_generation",
                "actionInput": "{'prompt': 'A short electronic cue.', 'duration_seconds': 30}",
                "prompt": "A short electronic cue.",
                "durationSeconds": 30,
                "uiState": "player_ready"
            },
            "googleApiKey": "AIzaProgramOwnedShouldDrop",
            "apiKeys": ["AIzaProgramOwnedShouldDrop"],
            "lastSeenConversationId": "c_4b6fc89f965c0d1e",
            "lastSeenResponseId": "r_7fa26ee4c86a2f0a",
            "candidatePairs": [
                {
                    "appPath": "/app/4abc4e7577b6149f",
                    "conversationId": "c_4abc4e7577b6149f",
                    "responseId": "r_1d583cfb0fa7aee4"
                }
            ],
            "aggregateHints": {
                "appPaths": ["/app/4abc4e7577b6149f"],
                "conversationIds": ["c_4abc4e7577b6149f"],
                "responseIds": ["r_1d583cfb0fa7aee4"]
            },
            "newChatClicked": true,
            "modeSelected": true,
            "capturedAt": "2026-05-05T02:40:36.361Z",
            "lastValidatedAt": "2026-05-05T02:40:36.361Z"
        }),
        Some("browser_state"),
    )
    .expect("normalized program relay payload");

    let extra = payload
        .get("extraBody")
        .and_then(Value::as_object)
        .expect("extraBody");
    assert_eq!(
        extra.get("canvasProgramUrl").and_then(Value::as_str),
        Some("https://gemini.google.com/app/4abc4e7577b6149f")
    );
    assert_eq!(
        extra.get("programId").and_then(Value::as_str),
        Some("4abc4e7577b6149f")
    );
    assert_eq!(
        extra.get("lastSeenResponseId").and_then(Value::as_str),
        Some("r_7fa26ee4c86a2f0a")
    );
    assert_eq!(
        extra.get("invokeBaseUrl").and_then(Value::as_str),
        Some("https://canvas-endpoint.example")
    );
    assert_eq!(
        extra.get("musicWsUrl").and_then(Value::as_str),
        Some("wss://canvas-endpoint.example/ws/music")
    );
    assert_eq!(
        extra.get("videoInvokePath").and_then(Value::as_str),
        Some("/v1beta/models/gemini-video:predictLongRunning")
    );
    assert_eq!(
        extra.get("canvasProgramAction").and_then(Value::as_str),
        Some("music_generation")
    );
    assert_eq!(
        extra
            .get("canvasProgramActionInput")
            .and_then(Value::as_str),
        Some("{'prompt': 'A short electronic cue.', 'duration_seconds': 30}")
    );
    assert_eq!(
        extra
            .get("canvasProgramInvokeContract")
            .and_then(Value::as_object)
            .and_then(|value| value.get("transportKind"))
            .and_then(Value::as_str),
        Some("official_music_ws_candidate")
    );
    assert!(!extra.contains_key("googleApiKey"));
    assert!(!extra.contains_key("apiKeys"));
    assert_eq!(payload.get("apiKey").and_then(Value::as_str), Some(""));
    assert_eq!(
        extra
            .get("candidatePairs")
            .and_then(Value::as_array)
            .map(|value| value.len()),
        Some(1)
    );
    assert_eq!(
        extra
            .get("aggregateHints")
            .and_then(Value::as_object)
            .and_then(|value| value.get("appPaths"))
            .and_then(Value::as_array)
            .map(|value| value.len()),
        Some(1)
    );
    assert_eq!(
        payload.get("providerSurfaceKey").and_then(Value::as_str),
        Some("gemini-canvas-program-relay")
    );
}

#[test]
fn normalize_gemini_canvas_import_payload_allows_source_credential_without_share_override() {
    let mut provider = build_test_provider_account(
        "Gemini Canvas Program Relay",
        "gemini_canvas_program_web_reverse_compatible",
        "gemini_canvas_images",
        "gemini_canvas_program_web_reverse_modular",
        "https://gemini.google.com",
        Some("web_reverse_api"),
        Some("browser_backed"),
    );
    provider.service_provider_key = "gemini_platform".to_string();
    provider.service_provider_label = "Gemini Platform".to_string();

    let payload = normalize_import_payload(
        "gemini-canvas-program-relay",
        &provider,
        serde_json::json!({
            "runtimeStateObjectKey": "credential-runtime/gemini-canvas/program/storage-state.json",
            "cookieHeader": "__Secure-1PSID=psid-main; __Secure-1PSIDTS=psidts-main",
            "credentialMaterialKey": "google-session-main-001"
        }),
        Some("browser_state"),
    )
    .expect("normalized source credential without share override");

    assert_eq!(
        payload.get("runtimeStateObjectKey").and_then(Value::as_str),
        Some("credential-runtime/gemini-canvas/program/storage-state.json")
    );
    let extra = payload
        .get("extraBody")
        .and_then(Value::as_object)
        .expect("extraBody");
    assert!(!extra.contains_key("shareId"));
    assert!(!extra.contains_key("shareUrl"));
    assert_eq!(
        payload
            .get("credentialMaterialKind")
            .and_then(Value::as_str),
        Some("browser_state")
    );
}

#[test]
fn normalize_gemini_canvas_import_payload_derives_share_id_from_share_url() {
    let mut provider = build_test_provider_account(
        "Gemini Canvas Program Relay",
        "gemini_canvas_program_web_reverse_compatible",
        "gemini_canvas_images",
        "gemini_canvas_program_web_reverse_modular",
        "https://gemini.google.com",
        Some("web_reverse_api"),
        Some("browser_backed"),
    );
    provider.service_provider_key = "gemini_platform".to_string();
    provider.service_provider_label = "Gemini Platform".to_string();

    let payload = normalize_import_payload(
        "gemini-canvas-program-relay",
        &provider,
        serde_json::json!({
            "runtimeStateObjectKey": "credential-runtime/gemini-canvas/program/storage-state.json",
            "shareUrl": "https://gemini.google.com/share/derived-share-456"
        }),
        Some("browser_state"),
    )
    .expect("normalized shareUrl override");

    let extra = payload
        .get("extraBody")
        .and_then(Value::as_object)
        .expect("extraBody");
    assert_eq!(
        extra.get("shareId").and_then(Value::as_str),
        Some("derived-share-456")
    );
    assert_eq!(
        extra.get("shareUrl").and_then(Value::as_str),
        Some("https://gemini.google.com/share/derived-share-456")
    );
}

#[test]
fn normalize_gemini_canvas_import_payload_unwraps_recursive_raw_source_chain() {
    let mut provider = build_test_provider_account(
        "Gemini Canvas Program Relay",
        "gemini_canvas_program_web_reverse_compatible",
        "gemini_canvas_images",
        "gemini_canvas_program_web_reverse_modular",
        "https://gemini.google.com",
        Some("web_reverse_api"),
        Some("browser_backed"),
    );
    provider.service_provider_key = "gemini_platform".to_string();
    provider.service_provider_label = "Gemini Platform".to_string();

    let payload = normalize_import_payload(
        "gemini-canvas-program-relay",
        &provider,
        serde_json::json!({
            "apiKey": "",
            "runtimeStateObjectKey": "credential-runtime/gemini-canvas/program/storage-state.json",
            "credentialMaterialKind": "browser_state",
            "providerSurfaceKey": "gemini-canvas-program-relay",
            "extraBody": {
                "shareId": "fe24c455a570"
            },
            "rawSource": {
                "runtimeStateObjectKey": "credential-runtime/gemini-canvas/program/storage-state.json",
                "shareId": "fe24c455a570",
                "canvasProgramHint": "matrix-live-bootstrap"
            }
        }),
        Some("browser_state"),
    )
    .expect("normalized recursive rawSource payload");

    let raw_source = payload
        .get("rawSource")
        .and_then(Value::as_object)
        .expect("rawSource object");
    assert!(!raw_source.contains_key("rawSource"));
    assert_eq!(
        raw_source
            .get("runtimeStateObjectKey")
            .and_then(Value::as_str),
        Some("credential-runtime/gemini-canvas/program/storage-state.json")
    );
    assert_eq!(
        raw_source.get("shareId").and_then(Value::as_str),
        Some("fe24c455a570")
    );
    assert_eq!(
        raw_source.get("canvasProgramHint").and_then(Value::as_str),
        Some("matrix-live-bootstrap")
    );
}
