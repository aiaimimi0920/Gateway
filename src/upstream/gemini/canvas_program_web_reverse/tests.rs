use base64::Engine;
use serde_json::{json, Value};

use super::*;
use crate::protocol::gemini::shared::GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER;
use crate::routing::candidate::ProviderAccountPayload;
use crate::routing::candidate::ProviderExecutionMode;

fn make_payload() -> ProviderAccountPayload {
    ProviderAccountPayload {
        adapter: GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER.to_string(),
        base_url: "https://gemini.google.com".to_string(),
        api_key: "unused".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: Some(
            "credential-runtime/gemini-canvas/program/storage-state.json".to_string(),
        ),
        account_name: None,
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: None,
        headers: std::collections::HashMap::new(),
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
        extra_body: Some(std::collections::HashMap::from([
            ("shareId".to_string(), json!("canvas-share-789")),
            (
                "browserRuntimeStateObjectKey".to_string(),
                json!("credential-runtime/gemini-canvas/program"),
            ),
            ("canvasProgramHint".to_string(), json!("quota-lane-probe")),
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
                    "targetMimeType": "video/mp4",
                    "targetCandidates": [
                        {
                            "url": "https://contribution.usercontent.google.com/download?filename=sunrise_over_gold.mp4",
                            "mimeType": "video/mp4",
                            "kind": "video",
                            "source": "media_node_current_src"
                        }
                    ],
                    "wsUrl": "ws://127.0.0.1:9998",
                    "apiStyle": "google_generative_language",
                    "requestEnvelopeKind": "canvas_proxy_request",
                    "requestUrl": "https://gemini.google.com/_/BardChatUi/data/batchexecute?rpcids=hNvQHb&source-path=%2Fapp%2F4abc4e7577b6149f",
                    "requestBody": "f.req=%5Bnull%2C%22A+short+electronic+cue.%22%5D&at=OLD-TOKEN",
                    "requestRpcId": "hNvQHb",
                    "responseRpcId": "MUAZcd",
                    "sourcePath": "/app/4abc4e7577b6149f",
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

mod endpoint_contracts;
mod payload_contracts;
mod response_contracts;
mod runtime_contracts;
