use crate::protocol::gemini::canvas_program_web_reverse as surface;
use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::gemini_canvas;
use crate::protocol::gemini_web;
use crate::routing::candidate::ProviderAccountPayload;

pub fn gemini_canvas_program_bootstrap_incomplete_error(provider: &str) -> GatewayError {
    GatewayError::server_error(
        "Gemini Canvas program bootstrap did not produce a concrete canvasProgramUrl/appPath/conversationId handle.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_program_bootstrap_incomplete")
}

pub fn is_concrete_gemini_canvas_program_url(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    if let Ok(parsed) = url::Url::parse(trimmed) {
        return matches!(
            parsed.domain(),
            Some("gemini.google.com") | Some("www.gemini.google.com")
        ) && parsed.path().starts_with("/app/")
            && parsed
                .path_segments()
                .and_then(|segments| segments.last())
                .map(|segment| !segment.is_empty())
                .unwrap_or(false);
    }
    trimmed.starts_with("/app/") && trimmed.len() > "/app/".len()
}

pub fn is_concrete_gemini_canvas_app_path(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("/app/") && trimmed.len() > "/app/".len()
}

pub fn is_concrete_gemini_canvas_conversation_id(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("c_") && trimmed.len() > 2
}

pub fn gemini_canvas_program_payload_has_concrete_handle(payload: &ProviderAccountPayload) -> bool {
    surface::relay_config_from_payload(payload)
        .map(|config| config.has_concrete_handle())
        .unwrap_or(false)
}

pub fn normalize_gemini_canvas_program_bootstrap_operation(operation: &str) -> &str {
    match operation {
        "tts" => "text",
        "text" | "image" | "music" | "video" => operation,
        _ => "image",
    }
}

pub fn gemini_canvas_program_payload_operation(payload: &ProviderAccountPayload) -> Option<String> {
    let extra_body = payload.extra_body.as_ref()?;
    extra_body
        .get("canvasProgramOperation")
        .or_else(|| extra_body.get("canvas_program_operation"))
        .or_else(|| extra_body.get("bootstrapOperation"))
        .or_else(|| extra_body.get("bootstrap_operation"))
        .or_else(|| {
            extra_body
                .get("canvasProgramInvokeContract")
                .or_else(|| extra_body.get("canvas_program_invoke_contract"))
                .and_then(Value::as_object)
                .and_then(|contract| {
                    contract
                        .get("operation")
                        .or_else(|| contract.get("bootstrapOperation"))
                        .or_else(|| contract.get("bootstrap_operation"))
                })
        })
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_gemini_canvas_program_bootstrap_operation)
        .map(str::to_string)
}

pub fn gemini_canvas_program_payload_handle_matches_operation(
    payload: &ProviderAccountPayload,
    operation: &str,
) -> bool {
    if !gemini_canvas_program_payload_has_concrete_handle(payload) {
        return false;
    }
    let desired = normalize_gemini_canvas_program_bootstrap_operation(operation);
    gemini_canvas_program_payload_operation(payload).as_deref() == Some(desired)
}

pub fn gemini_canvas_program_payload_source_path(
    payload: &ProviderAccountPayload,
) -> Option<String> {
    let config = surface::relay_config_from_payload(payload).ok()?;
    if let Some(app_path) = config
        .app_endpoint
        .app_path
        .as_deref()
        .map(str::trim)
        .filter(|value| is_concrete_gemini_canvas_app_path(value))
    {
        return Some(app_path.to_string());
    }
    config
        .app_endpoint
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| is_concrete_gemini_canvas_conversation_id(value))
        .map(|value| format!("/app/{}", value.trim_start_matches("c_")))
}

pub fn gemini_canvas_program_payload_conversation_id(
    payload: &ProviderAccountPayload,
) -> Option<String> {
    surface::relay_config_from_payload(payload)
        .ok()?
        .app_endpoint
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| is_concrete_gemini_canvas_conversation_id(value))
        .map(str::to_string)
}

pub fn gemini_canvas_program_payload_response_id(
    payload: &ProviderAccountPayload,
) -> Option<String> {
    surface::relay_config_from_payload(payload)
        .ok()?
        .app_endpoint
        .response_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn gemini_canvas_program_payload_locator(
    payload: &ProviderAccountPayload,
    stream_body: Option<&str>,
) -> Option<gemini_canvas::GeminiCanvasStreamGenerateLocator> {
    let app_path = gemini_canvas_program_payload_source_path(payload)?;
    let conversation_id = gemini_canvas_program_payload_conversation_id(payload)?;
    let response_id = stream_body
        .and_then(|body| gemini_canvas::extract_stream_generate_response_id(body).ok())
        .or_else(|| gemini_canvas_program_payload_response_id(payload))?;
    Some(gemini_canvas::GeminiCanvasStreamGenerateLocator {
        response_id,
        conversation_id,
        app_path,
    })
}

pub fn gemini_canvas_program_payload_page_url(
    payload: &ProviderAccountPayload,
    base_url: &str,
) -> Option<String> {
    let config = surface::relay_config_from_payload(payload).ok()?;
    if let Some(url) = config
        .app_endpoint
        .canvas_program_url
        .as_deref()
        .map(str::trim)
        .filter(|value| is_concrete_gemini_canvas_program_url(value))
    {
        return Some(url.to_string());
    }
    if let Some(url) = config
        .app_endpoint
        .page_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(app_path) = gemini_web::extract_app_page_path_from_url(url) {
            if is_concrete_gemini_canvas_app_path(&app_path) {
                return Some(format!("{}{app_path}", base_url.trim_end_matches('/')));
            }
        }
    }
    gemini_canvas_program_payload_source_path(payload)
        .map(|app_path| format!("{}{app_path}", base_url.trim_end_matches('/')))
}

pub fn strip_gemini_canvas_program_handle_hints_from_payload(
    payload: &ProviderAccountPayload,
) -> ProviderAccountPayload {
    let mut cloned = payload.clone();
    let Some(extra_body) = cloned.extra_body.as_mut() else {
        return cloned;
    };
    for key in [
        "canvasProgramUrl",
        "canvas_program_url",
        "programUrl",
        "program_url",
        "pageUrl",
        "page_url",
        "appPath",
        "app_path",
        "invokeBaseUrl",
        "invoke_base_url",
        "appEndpointBaseUrl",
        "app_endpoint_base_url",
        "musicWsUrl",
        "music_ws_url",
        "appMusicWsUrl",
        "app_music_ws_url",
        "videoInvokePath",
        "video_invoke_path",
        "videoPath",
        "video_path",
        "canvasProgramAction",
        "canvas_program_action",
        "programAction",
        "program_action",
        "canvasProgramActionInput",
        "canvas_program_action_input",
        "programActionInput",
        "program_action_input",
        "canvasProgramInvokeContract",
        "canvas_program_invoke_contract",
        "programInvokeContract",
        "program_invoke_contract",
        "conversationId",
        "conversation_id",
        "responseId",
        "response_id",
        "lastSeenConversationId",
        "last_seen_conversation_id",
        "lastSeenResponseId",
        "last_seen_response_id",
        "candidatePairs",
        "stableProgramPair",
        "latestResponsePair",
        "aggregateHints",
        "programId",
        "program_id",
        "canvasProgramOperation",
        "canvas_program_operation",
        "bootstrapOperation",
        "bootstrap_operation",
        "capturedAt",
        "captured_at",
        "lastValidatedAt",
        "last_validated_at",
        "newChatClicked",
        "modeSelected",
        "googleApiKey",
        "google_api_key",
        "cloudApiKey",
        "cloud_api_key",
        "apiKeys",
        "api_keys",
        "authToken",
        "auth_token",
    ] {
        extra_body.remove(key);
    }
    cloned
}
