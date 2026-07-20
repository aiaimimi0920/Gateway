use std::collections::HashMap;

use base64::Engine;

use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalRelayRequest, CanonicalRelayResponse, CanonicalToolCall, MessageRole,
};
use crate::protocol::gemini_canvas;
use crate::protocol::tool_choice::{self, CanonicalToolChoice};

const AISTUDIO_FIXTURE_IMAGE_PNG_BASE64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO7Z6f0AAAAASUVORK5CYII=";
const AISTUDIO_FIXTURE_PCM_BYTES: &[u8] = b"\x00\x10\x00\x20\x00\x30\x00\x40";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AIStudioFixtureImage {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AIStudioFixtureAudio {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

/// Build a deterministic local fixture response for tool-required AI Studio cases.
///
/// The first rollout of `AIStudioWebReverse` uses fixture transport only to validate
/// caller-visible bridge semantics. For required/specific tool-choice cases we synthesize
/// a canonical tool call locally instead of depending on a generic text-only Gemini fixture.
pub fn build_fixture_canonical_response(
    req: &CanonicalRelayRequest,
    model: &str,
) -> Option<CanonicalRelayResponse> {
    if req.tools.is_empty() || fixture_request_contains_tool_history(req) {
        return None;
    }

    let tool_name = match tool_choice::parse_tool_choice(req.tool_choice.as_ref()) {
        Some(CanonicalToolChoice::Specific(name)) => Some(name),
        Some(CanonicalToolChoice::Required) => fixture_infer_requested_tool_name(req),
        _ => None,
    }?;

    let arguments = fixture_tool_arguments_json(req, &tool_name);
    Some(CanonicalRelayResponse {
        model: model.to_string(),
        text: String::new(),
        usage: None,
        tool_calls: vec![CanonicalToolCall {
            id: Some(format!("call_{tool_name}")),
            call_type: "function".to_string(),
            name: Some(tool_name),
            arguments: Some(arguments),
            raw: HashMap::new(),
        }],
        upstream_status: Some(200),
        finish_reason: Some("tool_calls".to_string()),
    })
}

pub fn fixture_image() -> Result<AIStudioFixtureImage, GatewayError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(AISTUDIO_FIXTURE_IMAGE_PNG_BASE64)
        .map_err(|error| {
            GatewayError::server_error(format!(
                "AIStudio fixture image payload is invalid base64: {error}"
            ))
            .with_code("invalid_aistudio_fixture_image")
        })?;
    Ok(AIStudioFixtureImage {
        mime_type: "image/png".to_string(),
        bytes,
    })
}

pub fn fixture_audio() -> AIStudioFixtureAudio {
    AIStudioFixtureAudio {
        mime_type: "audio/L16;codec=pcm;rate=24000".to_string(),
        bytes: AISTUDIO_FIXTURE_PCM_BYTES.to_vec(),
    }
}

pub fn build_fixture_audio_binary_response(
    req: &CanonicalRelayRequest,
) -> Result<(Vec<u8>, String), GatewayError> {
    let audio = fixture_audio();
    let gemini_audio = gemini_canvas::GeminiCanvasAudio {
        mime_type: audio.mime_type,
        bytes: audio.bytes,
    };
    gemini_canvas::build_audio_binary_response(req, &gemini_audio)
}

fn fixture_request_contains_tool_history(req: &CanonicalRelayRequest) -> bool {
    req.messages.iter().any(|message| {
        message.role == MessageRole::Tool
            || !message.tool_calls.is_empty()
            || message.tool_call_id.is_some()
    })
}

fn fixture_infer_requested_tool_name(req: &CanonicalRelayRequest) -> Option<String> {
    if req.tools.len() == 1 {
        return req.tools[0].name.clone();
    }

    let conversation = req.messages_text().to_lowercase();
    for tool in &req.tools {
        let Some(name) = tool.name.as_deref() else {
            continue;
        };
        if conversation.contains(&name.to_lowercase()) {
            return Some(name.to_string());
        }
    }

    req.tools.iter().find_map(|tool| tool.name.clone())
}

fn fixture_tool_arguments_json(req: &CanonicalRelayRequest, tool_name: &str) -> String {
    let conversation = req.messages_text();
    if tool_name.eq_ignore_ascii_case("weather") {
        if conversation.contains("Hangzhou") {
            return "{\"city\":\"Hangzhou\"}".to_string();
        }
        if conversation.contains("San Francisco") {
            return "{\"city\":\"San Francisco\"}".to_string();
        }
    }
    "{}".to_string()
}
