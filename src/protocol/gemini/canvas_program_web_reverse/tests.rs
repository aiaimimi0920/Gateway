use std::collections::HashMap;

use serde_json::json;

use super::*;
use crate::routing::candidate::ProviderAccountPayload;

fn make_payload() -> ProviderAccountPayload {
    ProviderAccountPayload {
        discovered_protocols: Vec::new(),
        adapter: GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER.to_string(),
        base_url: "https://gemini.google.com".to_string(),
        api_key: "unused".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: Some(
            "credential-runtime/gemini-canvas/host-export/storage-state.json".to_string(),
        ),
        account_name: None,
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: None,
        headers: HashMap::new(),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        auth_token: None,
        responses_path: None,
        chat_completions_path: None,
        completions_path: None,
        embeddings_path: None,
        audio_transcriptions_path: None,
        audio_speech_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        extra_body: Some(HashMap::from([
            ("shareId".to_string(), json!("canvas-share-123")),
            (
                "browserRuntimeStateObjectKey".to_string(),
                json!("credential-runtime/gemini-canvas/host-export"),
            ),
            (
                "canvasRelayWsEndpoint".to_string(),
                json!("ws://127.0.0.1:42321/ws"),
            ),
            (
                "canvasProgramHint".to_string(),
                json!("browser-share-seeded-canvas"),
            ),
            (
                "canvasProgramUrl".to_string(),
                json!("https://gemini.google.com/app/program-123"),
            ),
            ("appPath".to_string(), json!("/app/program-123")),
            ("conversationId".to_string(), json!("c_program123")),
            ("responseId".to_string(), json!("r_program123")),
            (
                "invokeBaseUrl".to_string(),
                json!("https://canvas-endpoint.example"),
            ),
            (
                "musicWsUrl".to_string(),
                json!("wss://canvas-endpoint.example/ws/music"),
            ),
            (
                "videoInvokePath".to_string(),
                json!("/v1beta/models/gemini-video:predictLongRunning"),
            ),
            ("canvasProgramAction".to_string(), json!("music_generation")),
            (
                "canvasProgramActionInput".to_string(),
                json!("{'prompt': 'A short electronic cue.', 'duration_seconds': 30}"),
            ),
            (
                "canvasProgramInvokeContract".to_string(),
                json!({
                    "operation": "music",
                    "transportKind": "canvas_program_ws_candidate",
                    "target": "ws://127.0.0.1:9998",
                    "wsUrl": "ws://127.0.0.1:9998",
                    "apiStyle": "google_generative_language",
                    "requestEnvelopeKind": "canvas_proxy_request",
                    "requestUrl": "https://gemini.google.com/_/BardChatUi/data/batchexecute?rpcids=hNvQHb&source-path=%2Fapp%2Fprogram-123",
                    "requestBody": "f.req=%5Bnull%2C%22request-body%22%5D&at=old-token",
                    "requestRpcId": "hNvQHb",
                    "responseRpcId": "MUAZcd",
                    "sourcePath": "/app/program-123",
                    "modelHint": "models/veo-3.1-lite-generate-001;backend_beyond",
                    "actionName": "music_generation",
                    "actionInput": "{'prompt': 'A short electronic cue.', 'duration_seconds': 30}",
                    "prompt": "A short electronic cue.",
                    "durationSeconds": 30,
                    "uiState": "player_ready"
                }),
            ),
        ])),
        session_auth: None,
        keepalive: None,
    }
}

#[test]
fn relay_config_reads_program_fields() {
    let config = relay_config_from_payload(&make_payload()).expect("config");
    assert_eq!(config.bootstrap.share_id, "canvas-share-123");
    assert_eq!(
        config.bootstrap.runtime_state_object_key,
        "credential-runtime/gemini-canvas/host-export/storage-state.json"
    );
    assert_eq!(
        config.bootstrap.relay_ws_endpoint.as_deref(),
        Some("ws://127.0.0.1:42321/ws")
    );
    assert_eq!(
        config.bootstrap.canvas_program_hint.as_deref(),
        Some("browser-share-seeded-canvas")
    );
    assert_eq!(
        config.app_endpoint.canvas_program_url.as_deref(),
        Some("https://gemini.google.com/app/program-123")
    );
    assert_eq!(
        config.app_endpoint.app_path.as_deref(),
        Some("/app/program-123")
    );
    assert_eq!(
        config.app_endpoint.conversation_id.as_deref(),
        Some("c_program123")
    );
    assert_eq!(
        config.app_endpoint.response_id.as_deref(),
        Some("r_program123")
    );
    assert_eq!(
        config.app_endpoint.invoke_base_url.as_deref(),
        Some("https://canvas-endpoint.example")
    );
    assert_eq!(
        config.app_endpoint.music_ws_url.as_deref(),
        Some("wss://canvas-endpoint.example/ws/music")
    );
    assert_eq!(
        config.app_endpoint.video_invoke_path.as_deref(),
        Some("/v1beta/models/gemini-video:predictLongRunning")
    );
    assert_eq!(
        config.app_endpoint.canvas_program_action.as_deref(),
        Some("music_generation")
    );
    assert_eq!(
        config.app_endpoint.canvas_program_action_input.as_deref(),
        Some("{'prompt': 'A short electronic cue.', 'duration_seconds': 30}")
    );
    let invoke_contract = config
        .app_endpoint
        .canvas_program_invoke_contract
        .as_ref()
        .expect("invoke contract");
    assert_eq!(invoke_contract.operation.as_deref(), Some("music"));
    assert_eq!(
        invoke_contract.transport_kind.as_deref(),
        Some("canvas_program_ws_candidate")
    );
    assert_eq!(
        invoke_contract.target.as_deref(),
        Some("ws://127.0.0.1:9998")
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
        Some("https://gemini.google.com/_/BardChatUi/data/batchexecute?rpcids=hNvQHb&source-path=%2Fapp%2Fprogram-123")
    );
    assert_eq!(
        invoke_contract.request_body.as_deref(),
        Some("f.req=%5Bnull%2C%22request-body%22%5D&at=old-token")
    );
    assert_eq!(invoke_contract.request_rpc_id.as_deref(), Some("hNvQHb"));
    assert_eq!(invoke_contract.response_rpc_id.as_deref(), Some("MUAZcd"));
    assert_eq!(
        invoke_contract.source_path.as_deref(),
        Some("/app/program-123")
    );
    assert_eq!(
        invoke_contract.model_hint.as_deref(),
        Some("models/veo-3.1-lite-generate-001;backend_beyond")
    );
    assert_eq!(
        invoke_contract.action_name.as_deref(),
        Some("music_generation")
    );
    assert_eq!(
        invoke_contract.prompt.as_deref(),
        Some("A short electronic cue.")
    );
    assert_eq!(invoke_contract.duration_seconds, Some(30.0));
    assert_eq!(invoke_contract.ui_state.as_deref(), Some("player_ready"));
    assert!(config.has_concrete_handle());
}

#[test]
fn build_program_bootstrap_probe_prefers_explicit_program_url() {
    let probe = build_program_bootstrap_probe(&make_payload()).expect("probe");
    assert_eq!(
        probe.share_url,
        "https://gemini.google.com/share/canvas-share-123"
    );
    assert_eq!(probe.app_url, "https://gemini.google.com/app/program-123");
    assert!(probe.expect_canvas_program);
    assert_eq!(
        probe.metadata.get("canvasProgramHint").map(String::as_str),
        Some("browser-share-seeded-canvas")
    );
}
