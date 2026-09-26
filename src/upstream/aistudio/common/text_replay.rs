use std::sync::OnceLock;

use regex::Regex;
use serde_json::{json, Value};

use crate::error::GatewayError;

mod prompt;
#[cfg(test)]
mod tests;
mod tool_bridge;

pub use self::prompt::build_prompt_from_generate_content_body;
use self::prompt::extract_model_message_blocks;
use self::tool_bridge::{
    build_deterministic_roundtrip_text_response, build_deterministic_tool_call_response,
    build_synthetic_generate_content_response,
};

pub const AISTUDIO_CODE_ASSISTANT_OFFLINE_PATH: &str = "/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";

pub const AISTUDIO_STREAM_CODE_ASSISTANT_OFFLINE_GENERATION_PATH: &str = "/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/StreamCodeAssistantOfflineGeneration";

pub const AISTUDIO_DETERMINISTIC_TOOL_BRIDGE_OWNER: &str = "aistudio_deterministic_tool_bridge";

pub const AISTUDIO_DETERMINISTIC_ROUNDTRIP_BRIDGE_OWNER: &str =
    "aistudio_deterministic_roundtrip_bridge";

#[derive(Debug, Clone)]
pub struct AIStudioProgramOwnedTextReplayPlan {
    pub model: String,
    pub prompt_text: String,
    pub code_assistant_offline_path: &'static str,
    pub stream_code_assistant_offline_generation_path: &'static str,
    pub transport_owner: Option<&'static str>,
    pub deterministic_response: Option<Value>,
}

pub fn build_program_owned_text_replay_plan(
    request_url: &str,
    request_body: &Value,
) -> Result<AIStudioProgramOwnedTextReplayPlan, GatewayError> {
    let model = extract_requested_model_from_url(request_url)
        .unwrap_or_else(|| "gemini-3-flash-preview".to_string());
    let prompt_text = build_prompt_from_generate_content_body(request_body).ok_or_else(|| {
        GatewayError::bad_request(
            "AI Studio program-owned text replay could not infer a prompt from the generateContent request body.",
        )
        .with_code("aistudio_generate_content_prompt_unavailable")
    })?;

    let (transport_owner, deterministic_response) = if let Some(response) =
        build_deterministic_tool_call_response(request_body, &model)
    {
        (
            Some(AISTUDIO_DETERMINISTIC_TOOL_BRIDGE_OWNER),
            Some(response),
        )
    } else if let Some(response) = build_deterministic_roundtrip_text_response(request_body, &model)
    {
        (
            Some(AISTUDIO_DETERMINISTIC_ROUNDTRIP_BRIDGE_OWNER),
            Some(response),
        )
    } else {
        (None, None)
    };

    Ok(AIStudioProgramOwnedTextReplayPlan {
        model,
        prompt_text,
        code_assistant_offline_path: AISTUDIO_CODE_ASSISTANT_OFFLINE_PATH,
        stream_code_assistant_offline_generation_path:
            AISTUDIO_STREAM_CODE_ASSISTANT_OFFLINE_GENERATION_PATH,
        transport_owner,
        deterministic_response,
    })
}

pub fn extract_requested_model_from_url(url: &str) -> Option<String> {
    static MODEL_RE: OnceLock<Regex> = OnceLock::new();
    let re = MODEL_RE.get_or_init(|| {
        Regex::new(r"/models/([^/?#:]+):(stream)?generateContent")
            .expect("valid aistudio model extraction regex")
    });
    let normalized = normalize_string(Some(url))?;
    re.captures(normalized.as_str())
        .and_then(|captures| captures.get(1))
        .map(|capture| capture.as_str().to_string())
}

pub fn extract_code_assistant_generation_id(body_text: &str) -> Option<String> {
    let parsed = serde_json::from_str::<Value>(body_text).ok()?;
    parsed
        .as_array()
        .and_then(|items| items.first())
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub fn extract_final_text_from_code_assistant_stream(body_text: &str) -> Option<String> {
    let parsed = serde_json::from_str::<Value>(body_text).ok()?;
    let mut blocks = Vec::new();
    extract_model_message_blocks(&parsed, &mut blocks);
    let last_block = blocks.pop()?;
    last_block
        .into_iter()
        .filter_map(|entry| normalize_string(Some(entry.as_str())))
        .last()
}

pub fn build_code_assistant_offline_request_body(
    prompt_text: &str,
    model_path: &str,
    app_id: &str,
    opaque_token: &str,
) -> Value {
    json!([
        [[[[[null, prompt_text]], "user"]]],
        opaque_token,
        null,
        null,
        null,
        2,
        null,
        model_path,
        null,
        null,
        null,
        app_id,
        null,
        [2],
        null,
        1,
        null,
        null,
        [1, null, null, 3],
        null,
        app_id
    ])
}

pub fn build_stream_code_assistant_offline_generation_request_body(
    generation_id: &str,
    app_id: &str,
) -> Value {
    json!([generation_id, null, null, app_id])
}

pub fn build_text_only_generate_content_response(text: &str, model: &str) -> Value {
    build_synthetic_generate_content_response(text, model, &[])
}

fn normalize_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
