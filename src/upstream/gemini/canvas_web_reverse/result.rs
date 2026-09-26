use serde_json::Value;

use crate::upstream::gemini::canvas_program_web_reverse as program;

mod content;
mod invocation;
mod media;
mod worker;

pub use content::{
    decode_browser_tts_audio_payload, decode_inline_audio_payload,
    decode_modular_tts_audio_payload, extract_text_or_body_text, require_text_result,
};
pub use invocation::{
    browser_request_retry_delay_ms, parse_browser_invocation_response,
    parse_connected_fetch_invocation_response, parse_connected_fetch_json_body,
    parse_remote_browser_invocation_value, parse_remote_media_browser_invocation_value,
    parse_remote_modular_media_browser_invocation_value,
};
pub use media::{
    build_music_generation_response_from_invocation,
    build_video_generation_response_from_invocation, collect_converted_image_media_assets,
    collect_image_media_assets, convert_media_asset, require_music_media_asset,
    require_video_media_asset,
};
pub(crate) use worker::{
    build_gemini_canvas_browser_executor_service_result, classify_http_replay_worker_failure,
    extract_http_replay_worker_success, parse_http_replay_worker_output,
};

#[derive(Debug, serde::Deserialize)]
pub struct GeminiCanvasBrowserOwnedPoolResult {
    pub ok: bool,
    pub status: Option<u16>,
    pub result: Option<Value>,
    pub error: Option<GeminiCanvasBrowserOwnedPoolError>,
}

#[derive(Debug, serde::Deserialize)]
pub struct GeminiCanvasBrowserOwnedPoolError {
    pub code: Option<String>,
    pub message: Option<String>,
    pub status: Option<u16>,
    pub body: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct GeminiCanvasBrowserOwnedInvocationResult {
    pub operation: String,
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
    pub media: Vec<GeminiCanvasBrowserOwnedMediaAsset>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GeminiCanvasBrowserOwnedMediaAsset {
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

#[derive(Debug, serde::Deserialize)]
pub struct GeminiCanvasBrowserOwnedFetchInvocationResult {
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

impl From<GeminiCanvasBrowserOwnedMediaAsset> for program::GeminiCanvasBrowserMediaAsset {
    fn from(value: GeminiCanvasBrowserOwnedMediaAsset) -> Self {
        Self {
            kind: value.kind,
            url: value.url,
            mime_type: value.mime_type,
            body_base64: value.body_base64,
            alt: value.alt,
            width: value.width,
            height: value.height,
            duration_seconds: value.duration_seconds,
        }
    }
}

impl From<GeminiCanvasBrowserOwnedInvocationResult>
    for program::GeminiCanvasBrowserInvocationResult
{
    fn from(value: GeminiCanvasBrowserOwnedInvocationResult) -> Self {
        Self {
            operation: value.operation,
            bootstrap_operation: None,
            share_url: value.share_url,
            share_id: value.share_id,
            share_follow_kind: value.share_follow_kind,
            before_url: value.before_url,
            final_url: value.final_url,
            page_url: value.page_url,
            canvas_program_url: value.canvas_program_url,
            app_path: value.app_path,
            conversation_id: value.conversation_id,
            response_id: value.response_id,
            invoke_base_url: None,
            music_ws_url: None,
            video_invoke_path: None,
            canvas_program_action: None,
            canvas_program_action_input: None,
            canvas_program_invoke_contract: None,
            last_seen_conversation_id: value.last_seen_conversation_id,
            last_seen_response_id: value.last_seen_response_id,
            candidate_pairs: value.candidate_pairs,
            stable_program_pair: value.stable_program_pair,
            latest_response_pair: value.latest_response_pair,
            aggregate_hints: value.aggregate_hints,
            captured_at: value.captured_at,
            last_validated_at: value.last_validated_at,
            new_chat_clicked: value.new_chat_clicked,
            mode_selected: value.mode_selected,
            body_text: value.body_text,
            body_base64: value.body_base64,
            mime_type: value.mime_type,
            text: value.text,
            media: value.media.into_iter().map(Into::into).collect(),
        }
    }
}
