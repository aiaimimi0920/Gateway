use base64::Engine;
use bytes::Bytes;
use std::collections::HashMap;

use serde_json::Value;

use crate::error::{classify_upstream_error, GatewayError};

use super::normalize_gemini_canvas_program_bootstrap_operation;

pub fn gemini_canvas_program_bootstrap_missing_handle_patch_error(provider: &str) -> GatewayError {
    GatewayError::server_error(
        "Gemini Canvas program bootstrap completed without emitting runtime handle fields.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_program_bootstrap_missing_handle_patch")
}

#[derive(Debug, serde::Deserialize)]
pub struct GeminiCanvasBrowserPoolResult {
    pub ok: bool,
    pub status: Option<u16>,
    pub result: Option<Value>,
    pub error: Option<GeminiCanvasBrowserPoolError>,
}

#[derive(Debug, serde::Deserialize)]
pub struct GeminiCanvasBrowserPoolError {
    pub code: Option<String>,
    pub message: Option<String>,
    pub status: Option<u16>,
    pub body: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct GeminiCanvasBrowserInvocationResult {
    pub operation: String,
    #[serde(rename = "bootstrapOperation")]
    pub bootstrap_operation: Option<String>,
    #[serde(rename = "shareUrl")]
    pub share_url: Option<String>,
    #[serde(rename = "shareId")]
    pub share_id: Option<String>,
    #[serde(rename = "shareFollowKind")]
    pub share_follow_kind: Option<String>,
    #[serde(rename = "beforeUrl")]
    pub before_url: Option<String>,
    #[serde(rename = "finalUrl")]
    pub final_url: Option<String>,
    #[serde(rename = "pageUrl")]
    pub page_url: Option<String>,
    #[serde(rename = "canvasProgramUrl")]
    pub canvas_program_url: Option<String>,
    #[serde(rename = "appPath")]
    pub app_path: Option<String>,
    #[serde(rename = "conversationId")]
    pub conversation_id: Option<String>,
    #[serde(rename = "responseId")]
    pub response_id: Option<String>,
    #[serde(rename = "invokeBaseUrl")]
    pub invoke_base_url: Option<String>,
    #[serde(rename = "musicWsUrl")]
    pub music_ws_url: Option<String>,
    #[serde(rename = "videoInvokePath")]
    pub video_invoke_path: Option<String>,
    #[serde(rename = "canvasProgramAction")]
    pub canvas_program_action: Option<String>,
    #[serde(rename = "canvasProgramActionInput")]
    pub canvas_program_action_input: Option<String>,
    #[serde(rename = "canvasProgramInvokeContract")]
    pub canvas_program_invoke_contract: Option<Value>,
    #[serde(rename = "lastSeenConversationId")]
    pub last_seen_conversation_id: Option<String>,
    #[serde(rename = "lastSeenResponseId")]
    pub last_seen_response_id: Option<String>,
    #[serde(rename = "candidatePairs", default)]
    pub candidate_pairs: Vec<Value>,
    #[serde(rename = "stableProgramPair")]
    pub stable_program_pair: Option<Value>,
    #[serde(rename = "latestResponsePair")]
    pub latest_response_pair: Option<Value>,
    #[serde(rename = "aggregateHints")]
    pub aggregate_hints: Option<Value>,
    #[serde(rename = "capturedAt")]
    pub captured_at: Option<String>,
    #[serde(rename = "lastValidatedAt")]
    pub last_validated_at: Option<String>,
    #[serde(rename = "newChatClicked")]
    pub new_chat_clicked: Option<bool>,
    #[serde(rename = "modeSelected")]
    pub mode_selected: Option<bool>,
    #[serde(rename = "bodyText")]
    pub body_text: Option<String>,
    #[serde(rename = "bodyBase64")]
    pub body_base64: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    pub text: Option<String>,
    #[serde(default)]
    pub media: Vec<GeminiCanvasBrowserMediaAsset>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GeminiCanvasBrowserMediaAsset {
    pub kind: String,
    pub url: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    #[serde(rename = "bodyBase64")]
    pub body_base64: Option<String>,
    pub alt: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[serde(rename = "durationSeconds")]
    pub duration_seconds: Option<f64>,
}

#[derive(Debug, Clone)]
struct GeminiCanvasCanonicalPair {
    canvas_program_url: Option<String>,
    app_path: Option<String>,
    conversation_id: Option<String>,
    response_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct GeminiCanvasBrowserFetchInvocationResult {
    pub operation: String,
    pub status: u16,
    #[serde(rename = "finalUrl")]
    pub final_url: Option<String>,
    #[serde(rename = "pageUrl")]
    pub page_url: Option<String>,
    #[serde(rename = "canvasProgramUrl")]
    pub canvas_program_url: Option<String>,
    #[serde(rename = "appPath")]
    pub app_path: Option<String>,
    #[serde(rename = "conversationId")]
    pub conversation_id: Option<String>,
    #[serde(rename = "responseId")]
    pub response_id: Option<String>,
    #[serde(rename = "invokeBaseUrl")]
    pub invoke_base_url: Option<String>,
    #[serde(rename = "musicWsUrl")]
    pub music_ws_url: Option<String>,
    #[serde(rename = "videoInvokePath")]
    pub video_invoke_path: Option<String>,
    #[serde(rename = "lastSeenConversationId")]
    pub last_seen_conversation_id: Option<String>,
    #[serde(rename = "lastSeenResponseId")]
    pub last_seen_response_id: Option<String>,
    #[serde(rename = "candidatePairs", default)]
    pub candidate_pairs: Vec<Value>,
    #[serde(rename = "capturedAt")]
    pub captured_at: Option<String>,
    #[serde(rename = "lastValidatedAt")]
    pub last_validated_at: Option<String>,
    #[serde(rename = "contentType")]
    pub content_type: Option<String>,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(rename = "bodyText")]
    pub body_text: Option<String>,
    #[serde(rename = "bodyBase64")]
    pub body_base64: Option<String>,
}

fn parse_browser_pool_result(
    provider: &str,
    body_text: &str,
    parse_error_code: &'static str,
    context_label: &str,
) -> Result<GeminiCanvasBrowserPoolResult, GatewayError> {
    serde_json::from_str(body_text).map_err(|error| {
        GatewayError::server_error(format!(
            "Failed to parse {context_label}: {error}. body: {body_text}"
        ))
        .with_provider(provider)
        .with_code(parse_error_code)
    })
}

fn classify_browser_pool_failure(
    provider: &str,
    http_status: u16,
    result: GeminiCanvasBrowserPoolResult,
) -> GatewayError {
    let mut gateway_error = classify_upstream_error(
        result
            .status
            .or_else(|| result.error.as_ref().and_then(|entry| entry.status))
            .unwrap_or(http_status),
        result
            .error
            .as_ref()
            .and_then(|entry| entry.body.as_deref())
            .unwrap_or(""),
        Some(provider),
    );
    if let Some(error) = result.error {
        if let Some(code) = error.code {
            gateway_error.code = Some(code);
        }
        if let Some(message) = error.message {
            gateway_error.message = message;
        }
    }
    gateway_error
}

pub fn parse_program_browser_invocation_response(
    provider: &str,
    http_status: u16,
    body_text: &str,
    expected_operation: &str,
) -> Result<GeminiCanvasBrowserInvocationResult, GatewayError> {
    let result = parse_browser_pool_result(
        provider,
        body_text,
        "gemini_canvas_browser_pool_output_parse_failed",
        "Gemini Canvas browser pool response",
    )?;
    if !result.ok {
        return Err(classify_browser_pool_failure(provider, http_status, result));
    }
    let result_body = result.result.ok_or_else(|| {
        GatewayError::server_error(
            "Gemini Canvas browser pool reported success without a result payload.",
        )
        .with_provider(provider)
        .with_code("gemini_canvas_browser_pool_missing_result")
    })?;
    let invocation = serde_json::from_value::<GeminiCanvasBrowserInvocationResult>(result_body)
        .map_err(|error| {
            GatewayError::server_error(format!(
                "Failed to decode Gemini Canvas browser pool media payload: {error}"
            ))
            .with_provider(provider)
            .with_code("gemini_canvas_browser_pool_result_decode_failed")
        })?;
    if invocation.operation != expected_operation {
        return Err(GatewayError::server_error(format!(
            "Gemini Canvas browser pool returned '{}' while '{}' was requested.",
            invocation.operation, expected_operation
        ))
        .with_provider(provider)
        .with_code("gemini_canvas_browser_pool_operation_mismatch"));
    }
    Ok(invocation)
}

pub fn parse_program_bootstrap_invocation_response(
    provider: &str,
    http_status: u16,
    body_text: &str,
) -> Result<GeminiCanvasBrowserInvocationResult, GatewayError> {
    let result = parse_browser_pool_result(
        provider,
        body_text,
        "gemini_canvas_program_bootstrap_output_parse_failed",
        "Gemini Canvas program bootstrap response",
    )?;
    if !result.ok {
        return Err(classify_browser_pool_failure(provider, http_status, result));
    }
    let result_body = result.result.ok_or_else(|| {
        GatewayError::server_error(
            "Gemini Canvas program bootstrap reported success without a result payload.",
        )
        .with_provider(provider)
        .with_code("gemini_canvas_program_bootstrap_missing_result")
    })?;
    let invocation = serde_json::from_value::<GeminiCanvasBrowserInvocationResult>(result_body)
        .map_err(|error| {
            GatewayError::server_error(format!(
                "Failed to decode Gemini Canvas program bootstrap payload: {error}"
            ))
            .with_provider(provider)
            .with_code("gemini_canvas_program_bootstrap_decode_failed")
        })?;
    if invocation.operation != "bootstrap_program" {
        return Err(GatewayError::server_error(format!(
            "Gemini Canvas program bootstrap returned '{}' instead of 'bootstrap_program'.",
            invocation.operation
        ))
        .with_provider(provider)
        .with_code("gemini_canvas_program_bootstrap_operation_mismatch"));
    }
    Ok(invocation)
}

pub fn parse_connected_fetch_invocation_response(
    provider: &str,
    http_status: u16,
    body_text: &str,
) -> Result<GeminiCanvasBrowserFetchInvocationResult, GatewayError> {
    let result = parse_browser_pool_result(
        provider,
        body_text,
        "gemini_canvas_connected_fetch_output_parse_failed",
        "Gemini Canvas connected fetch response",
    )?;
    if !result.ok {
        return Err(classify_browser_pool_failure(provider, http_status, result));
    }
    let result_body = result.result.ok_or_else(|| {
        GatewayError::server_error(
            "Gemini Canvas connected fetch reported success without a result payload.",
        )
        .with_provider(provider)
        .with_code("gemini_canvas_connected_fetch_missing_result")
    })?;
    let invocation = serde_json::from_value::<GeminiCanvasBrowserFetchInvocationResult>(
        result_body,
    )
    .map_err(|error| {
        GatewayError::server_error(format!(
            "Failed to decode Gemini Canvas connected fetch payload: {error}"
        ))
        .with_provider(provider)
        .with_code("gemini_canvas_connected_fetch_decode_failed")
    })?;
    if !(200..300).contains(&invocation.status) {
        return Err(classify_upstream_error(
            invocation.status,
            invocation.body_text.as_deref().unwrap_or(""),
            Some(provider),
        ));
    }
    Ok(invocation)
}

pub fn parse_connected_fetch_json_body(
    provider: &str,
    body_text: Option<&str>,
) -> Result<Value, GatewayError> {
    let body_text = body_text.ok_or_else(|| {
        GatewayError::server_error("Gemini Canvas connected fetch completed without a JSON body.")
            .with_provider(provider)
            .with_code("gemini_canvas_connected_fetch_missing_body")
    })?;
    serde_json::from_str::<Value>(body_text).map_err(|error| {
        GatewayError::server_error(format!(
            "Gemini Canvas connected fetch did not return valid JSON: {error}"
        ))
        .with_provider(provider)
        .with_code("gemini_canvas_connected_fetch_invalid_json")
    })
}

pub fn parse_connected_fetch_get_json_body(
    provider: &str,
    body_text: Option<&str>,
) -> Result<Value, GatewayError> {
    let body_text = body_text.ok_or_else(|| {
        GatewayError::server_error(
            "Gemini Canvas connected fetch GET completed without a JSON body.",
        )
        .with_provider(provider)
        .with_code("gemini_canvas_connected_fetch_missing_body")
    })?;
    serde_json::from_str::<Value>(body_text).map_err(|error| {
        GatewayError::server_error(format!(
            "Gemini Canvas connected fetch GET did not return valid JSON: {error}"
        ))
        .with_provider(provider)
        .with_code("gemini_canvas_connected_fetch_invalid_json")
    })
}

pub fn decode_connected_fetch_body_bytes(
    provider: &str,
    invocation: &GeminiCanvasBrowserFetchInvocationResult,
) -> Result<(Bytes, Option<String>), GatewayError> {
    let bytes = if let Some(body_base64) = invocation.body_base64.as_deref() {
        Bytes::from(
            base64::engine::general_purpose::STANDARD
                .decode(body_base64)
                .map_err(|error| {
                    GatewayError::server_error(format!(
                        "Gemini Canvas connected fetch returned invalid base64 bytes: {error}"
                    ))
                    .with_provider(provider)
                    .with_code("gemini_canvas_connected_fetch_invalid_base64")
                })?,
        )
    } else if let Some(body_text) = invocation.body_text.as_ref() {
        Bytes::from(body_text.clone().into_bytes())
    } else {
        return Err(GatewayError::server_error(
            "Gemini Canvas connected fetch GET completed without a body payload.",
        )
        .with_provider(provider)
        .with_code("gemini_canvas_connected_fetch_missing_body"));
    };
    Ok((bytes, invocation.content_type.clone()))
}

pub fn runtime_patch_from_browser_invocation(
    result: &GeminiCanvasBrowserInvocationResult,
) -> Option<HashMap<String, Value>> {
    let mut patch = HashMap::new();
    let effective_operation = result
        .bootstrap_operation
        .as_deref()
        .or({
            let operation = result.operation.as_str();
            matches!(operation, "text" | "tts" | "image" | "music" | "video").then_some(operation)
        })
        .map(normalize_gemini_canvas_program_bootstrap_operation)
        .map(str::to_string);
    let canonical_pair =
        canonical_pair_from_browser_invocation(result, effective_operation.as_deref());
    if let Some(value) = effective_operation.as_ref() {
        patch.insert(
            "canvasProgramOperation".to_string(),
            Value::String(value.clone()),
        );
        patch.insert(
            "bootstrapOperation".to_string(),
            Value::String(value.clone()),
        );
    }
    if let Some(value) = result.share_url.as_ref() {
        patch.insert("shareUrl".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = result.share_id.as_ref() {
        patch.insert("shareId".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = result.share_follow_kind.as_ref() {
        patch.insert("shareFollowKind".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = result.before_url.as_ref() {
        patch.insert("beforeUrl".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = result.final_url.as_ref() {
        patch.insert("finalUrl".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = canonical_pair
        .as_ref()
        .and_then(|pair| pair.canvas_program_url.as_ref())
        .or(result.canvas_program_url.as_ref())
    {
        patch.insert("canvasProgramUrl".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = result.page_url.as_ref() {
        patch.insert("pageUrl".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = canonical_pair
        .as_ref()
        .and_then(|pair| pair.app_path.as_ref())
        .or(result.app_path.as_ref())
    {
        patch.insert("appPath".to_string(), Value::String(value.clone()));
        if let Some(program_id) = value.rsplit('/').next().filter(|entry| !entry.is_empty()) {
            patch.insert(
                "programId".to_string(),
                Value::String(program_id.to_string()),
            );
        }
    }
    if let Some(value) = canonical_pair
        .as_ref()
        .and_then(|pair| pair.conversation_id.as_ref())
        .or(result.conversation_id.as_ref())
    {
        patch.insert("conversationId".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = canonical_pair
        .as_ref()
        .and_then(|pair| pair.response_id.as_ref())
        .or(result.response_id.as_ref())
    {
        patch.insert("responseId".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = result.invoke_base_url.as_ref() {
        patch.insert("invokeBaseUrl".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = result.music_ws_url.as_ref() {
        patch.insert("musicWsUrl".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = result.video_invoke_path.as_ref() {
        patch.insert("videoInvokePath".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = result.canvas_program_action.as_ref() {
        patch.insert(
            "canvasProgramAction".to_string(),
            Value::String(value.clone()),
        );
    }
    if let Some(value) = result.canvas_program_action_input.as_ref() {
        patch.insert(
            "canvasProgramActionInput".to_string(),
            Value::String(value.clone()),
        );
    }
    if let Some(value) = result.canvas_program_invoke_contract.as_ref() {
        patch.insert("canvasProgramInvokeContract".to_string(), value.clone());
    }
    if let Some(value) = result.last_seen_conversation_id.as_ref() {
        patch.insert(
            "lastSeenConversationId".to_string(),
            Value::String(value.clone()),
        );
    }
    if let Some(value) = result.last_seen_response_id.as_ref() {
        patch.insert(
            "lastSeenResponseId".to_string(),
            Value::String(value.clone()),
        );
    }
    if !result.candidate_pairs.is_empty() {
        patch.insert(
            "candidatePairs".to_string(),
            Value::Array(result.candidate_pairs.clone()),
        );
    }
    if let Some(value) = result.stable_program_pair.as_ref() {
        patch.insert("stableProgramPair".to_string(), value.clone());
    }
    if let Some(value) = result.latest_response_pair.as_ref() {
        patch.insert("latestResponsePair".to_string(), value.clone());
    }
    if let Some(value) = result.aggregate_hints.as_ref() {
        patch.insert("aggregateHints".to_string(), value.clone());
    }
    if let Some(value) = result.captured_at.as_ref() {
        patch.insert("capturedAt".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = result.last_validated_at.as_ref() {
        patch.insert("lastValidatedAt".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = result.new_chat_clicked {
        patch.insert("newChatClicked".to_string(), Value::Bool(value));
    }
    if let Some(value) = result.mode_selected {
        patch.insert("modeSelected".to_string(), Value::Bool(value));
    }
    if patch.is_empty() {
        None
    } else {
        Some(patch)
    }
}

fn canonical_pair_from_browser_invocation(
    result: &GeminiCanvasBrowserInvocationResult,
    effective_operation: Option<&str>,
) -> Option<GeminiCanvasCanonicalPair> {
    let latest_pair = result.latest_response_pair.as_ref()?;
    let latest = read_canonical_pair_from_value(latest_pair);
    let current = GeminiCanvasCanonicalPair {
        canvas_program_url: result.canvas_program_url.clone(),
        app_path: result.app_path.clone(),
        conversation_id: result.conversation_id.clone(),
        response_id: result.response_id.clone(),
    };

    if !should_promote_latest_pair(&current, &latest, effective_operation, result) {
        return None;
    }

    Some(latest)
}

fn should_promote_latest_pair(
    current: &GeminiCanvasCanonicalPair,
    latest: &GeminiCanvasCanonicalPair,
    effective_operation: Option<&str>,
    result: &GeminiCanvasBrowserInvocationResult,
) -> bool {
    let operation_prefers_latest = matches!(effective_operation, Some("image" | "music" | "video"));
    let latest_complete = latest.app_path.is_some()
        && latest.conversation_id.is_some()
        && latest.response_id.is_some();
    if !operation_prefers_latest || !latest_complete {
        return false;
    }

    let latest_differs = latest.app_path != current.app_path
        || latest.conversation_id != current.conversation_id
        || latest.response_id != current.response_id;
    if !latest_differs {
        return false;
    }

    let latest_matches_concrete_final = concrete_app_path_from_url(result.final_url.as_deref())
        .as_deref()
        == latest.app_path.as_deref()
        || concrete_app_path_from_url(result.page_url.as_deref()).as_deref()
            == latest.app_path.as_deref();
    if latest_matches_concrete_final {
        return true;
    }

    is_generic_canvas_landing_url(result.final_url.as_deref())
        || is_generic_canvas_landing_url(result.page_url.as_deref())
}

fn is_generic_canvas_landing_url(value: Option<&str>) -> bool {
    let Some(value) = value.map(str::trim).filter(|entry| !entry.is_empty()) else {
        return false;
    };
    let lowered = value.to_ascii_lowercase();
    lowered.ends_with("/app") || lowered.ends_with("/canvas")
}

fn concrete_app_path_from_url(value: Option<&str>) -> Option<String> {
    let value = value.map(str::trim).filter(|entry| !entry.is_empty())?;
    let parsed = url::Url::parse(value).ok()?;
    let path = parsed.path().trim();
    (path.starts_with("/app/") && path.len() > "/app/".len()).then(|| path.to_string())
}

fn read_canonical_pair_from_value(value: &Value) -> GeminiCanvasCanonicalPair {
    GeminiCanvasCanonicalPair {
        canvas_program_url: read_pair_string(value, "programUrl")
            .or_else(|| read_pair_string(value, "canvasProgramUrl")),
        app_path: read_pair_string(value, "appPath"),
        conversation_id: read_pair_string(value, "conversationId"),
        response_id: read_pair_string(value, "responseId"),
    }
}

fn read_pair_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
}

pub fn runtime_patch_from_browser_fetch(
    result: &GeminiCanvasBrowserFetchInvocationResult,
) -> Option<HashMap<String, Value>> {
    let mut patch = HashMap::new();
    if let Some(value) = result.canvas_program_url.as_ref() {
        patch.insert("canvasProgramUrl".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = result.page_url.as_ref() {
        patch.insert("pageUrl".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = result.app_path.as_ref() {
        patch.insert("appPath".to_string(), Value::String(value.clone()));
        if let Some(program_id) = value.rsplit('/').next().filter(|entry| !entry.is_empty()) {
            patch.insert(
                "programId".to_string(),
                Value::String(program_id.to_string()),
            );
        }
    }
    if let Some(value) = result.conversation_id.as_ref() {
        patch.insert("conversationId".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = result.response_id.as_ref() {
        patch.insert("responseId".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = result.invoke_base_url.as_ref() {
        patch.insert("invokeBaseUrl".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = result.music_ws_url.as_ref() {
        patch.insert("musicWsUrl".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = result.video_invoke_path.as_ref() {
        patch.insert("videoInvokePath".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = result.last_seen_conversation_id.as_ref() {
        patch.insert(
            "lastSeenConversationId".to_string(),
            Value::String(value.clone()),
        );
    }
    if let Some(value) = result.last_seen_response_id.as_ref() {
        patch.insert(
            "lastSeenResponseId".to_string(),
            Value::String(value.clone()),
        );
    }
    if !result.candidate_pairs.is_empty() {
        patch.insert(
            "candidatePairs".to_string(),
            Value::Array(result.candidate_pairs.clone()),
        );
    }
    if let Some(value) = result.captured_at.as_ref() {
        patch.insert("capturedAt".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = result.last_validated_at.as_ref() {
        patch.insert("lastValidatedAt".to_string(), Value::String(value.clone()));
    }
    if patch.is_empty() {
        None
    } else {
        Some(patch)
    }
}
