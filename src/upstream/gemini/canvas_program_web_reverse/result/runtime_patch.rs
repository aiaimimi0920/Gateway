use std::collections::HashMap;

use serde_json::Value;

use crate::error::GatewayError;

use super::super::normalize_gemini_canvas_program_bootstrap_operation;
use super::types::{GeminiCanvasBrowserFetchInvocationResult, GeminiCanvasBrowserInvocationResult};

pub fn gemini_canvas_program_bootstrap_missing_handle_patch_error(provider: &str) -> GatewayError {
    GatewayError::server_error(
        "Gemini Canvas program bootstrap completed without emitting runtime handle fields.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_program_bootstrap_missing_handle_patch")
}

#[derive(Debug, Clone)]
struct GeminiCanvasCanonicalPair {
    canvas_program_url: Option<String>,
    app_path: Option<String>,
    conversation_id: Option<String>,
    response_id: Option<String>,
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
