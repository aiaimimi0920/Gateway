use crate::error::{sanitize_provider_error_message, GatewayError};
use crate::protocol::{gemini_canvas, gemini_web};
use crate::upstream::gemini_canvas_runtime_error_helpers::compact_response_preview;
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Debug)]
pub(crate) struct GeminiCanvasVideoStageProgress {
    pub(crate) body: String,
    pub(crate) pending: bool,
}

pub(crate) struct GeminiCanvasVideoCompletionAttemptState {
    pub(crate) completion: GeminiCanvasVideoStageProgress,
    pub(crate) metadata: Option<GeminiCanvasVideoStageProgress>,
}

impl GeminiCanvasVideoCompletionAttemptState {
    pub(crate) fn next_sleep_for(&self, attempt: usize) -> Option<Duration> {
        plan_gemini_canvas_video_completion_attempt_sleep(
            self.completion.pending,
            self.metadata
                .as_ref()
                .map(|value| value.pending)
                .unwrap_or(false),
            attempt,
        )
    }
}

#[derive(Default)]
pub(crate) struct GeminiCanvasVideoCompletionPollState {
    pub(crate) attempt: usize,
    pub(crate) last_completion_body: Option<String>,
    pub(crate) last_metadata_body: Option<String>,
}

impl GeminiCanvasVideoCompletionPollState {
    pub(crate) fn next_attempt(&mut self) -> usize {
        self.attempt += 1;
        self.attempt
    }

    pub(crate) fn record_attempt_progress(
        &mut self,
        attempt_state: &GeminiCanvasVideoCompletionAttemptState,
    ) -> Option<Duration> {
        self.last_completion_body = Some(attempt_state.completion.body.clone());
        self.last_metadata_body = attempt_state
            .metadata
            .as_ref()
            .map(|value| value.body.clone());
        attempt_state.next_sleep_for(self.attempt)
    }

    pub(crate) fn build_missing_asset_error(
        &self,
        source_path: &str,
        conversation_id: &str,
        response_id: &str,
        job_id: Option<&str>,
    ) -> GatewayError {
        build_gemini_canvas_video_completion_missing_asset_error(
            source_path,
            conversation_id,
            response_id,
            self.attempt,
            job_id,
            self.last_completion_body.as_deref(),
            self.last_metadata_body.as_deref(),
        )
    }
}

pub(crate) enum GeminiCanvasVideoCompletionAttemptOutcome {
    Completed(String),
    Continue(GeminiCanvasVideoCompletionAttemptState),
}

pub(crate) struct GeminiCanvasVideoCompletionRequests {
    pub(crate) job_id: Option<String>,
    pub(crate) job_poll_request: Option<Result<gemini_web::GeminiWebRequest, GatewayError>>,
    pub(crate) completion_request: gemini_web::GeminiWebRequest,
    pub(crate) metadata_request: gemini_web::GeminiWebRequest,
}

pub(crate) fn build_gemini_canvas_video_accepted_response_from_body(
    model: &str,
    prompt: &str,
    body_text: &str,
    fallback_conversation_id: Option<&str>,
    fallback_response_id: Option<&str>,
    fallback_app_path: Option<&str>,
) -> Value {
    let locator_hint = gemini_canvas::extract_stream_generate_locator(body_text).ok();
    let accepted_hints = extract_gemini_canvas_video_accepted_hints(body_text);
    let conversation_id_hint = locator_hint
        .as_ref()
        .map(|locator| locator.conversation_id.as_str())
        .or(accepted_hints.conversation_id.as_deref())
        .or(fallback_conversation_id);
    let response_id_hint = locator_hint
        .as_ref()
        .map(|locator| locator.response_id.as_str())
        .or(accepted_hints.response_id.as_deref())
        .or(fallback_response_id);
    let app_path_hint = accepted_hints
        .app_path
        .as_deref()
        .or_else(|| {
            locator_hint
                .as_ref()
                .map(|locator| locator.app_path.as_str())
        })
        .or(fallback_app_path);
    let job_id_hint = accepted_hints
        .job_id
        .or_else(|| gemini_canvas::extract_video_generation_job_id(body_text));
    gemini_canvas::build_video_generation_accepted_response(
        model,
        prompt,
        conversation_id_hint,
        response_id_hint,
        app_path_hint,
        job_id_hint.as_deref(),
        Some(body_text),
    )
}

#[derive(Default)]
struct GeminiCanvasVideoAcceptedHints {
    conversation_id: Option<String>,
    response_id: Option<String>,
    app_path: Option<String>,
    job_id: Option<String>,
}

fn extract_gemini_canvas_video_accepted_hints(body_text: &str) -> GeminiCanvasVideoAcceptedHints {
    let normalized = body_text
        .replace("\\\"", "\"")
        .replace("\\/", "/")
        .replace("\\u002f", "/");
    let mut hints = GeminiCanvasVideoAcceptedHints {
        conversation_id: scan_stream_generate_id_hint(&normalized, "c_"),
        response_id: scan_stream_generate_id_hint(&normalized, "r_"),
        ..Default::default()
    };

    static VIDEO_ACCEPTED_JOB_HINT_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = VIDEO_ACCEPTED_JOB_HINT_REGEX.get_or_init(|| {
        Regex::new(r#""65"\s*:\s*\[\s*\[\s*"([^"]+)"\s*\]\s*,\s*"([^"]+)""#)
            .expect("Gemini Canvas video accepted hint regex must compile")
    });
    if let Some(captures) = regex.captures(&normalized) {
        hints.app_path = captures
            .get(1)
            .map(|matched| matched.as_str().trim().to_string())
            .filter(|value| value.starts_with("/app/"));
        hints.job_id = captures
            .get(2)
            .map(|matched| matched.as_str().trim().to_string())
            .filter(|value| !value.is_empty());
    }

    hints
}

fn scan_stream_generate_id_hint(body_text: &str, prefix: &str) -> Option<String> {
    let bytes = body_text.as_bytes();
    let prefix_bytes = prefix.as_bytes();
    let mut index = 0usize;
    while index + prefix_bytes.len() <= bytes.len() {
        if &bytes[index..index + prefix_bytes.len()] != prefix_bytes {
            index += 1;
            continue;
        }
        let mut end = index + prefix_bytes.len();
        while end < bytes.len() {
            let ch = bytes[end] as char;
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-') {
                end += 1;
            } else {
                break;
            }
        }
        if end > index + prefix_bytes.len() {
            return Some(body_text[index..end].to_string());
        }
        index += 1;
    }
    None
}

pub(crate) fn build_gemini_canvas_video_completion_requests(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
    conversation_id: &str,
    response_id: &str,
    primary_body: &str,
) -> Result<GeminiCanvasVideoCompletionRequests, GatewayError> {
    let job_id = gemini_canvas::extract_video_generation_job_id(primary_body);
    let job_poll_request = job_id
        .as_deref()
        .map(|job_id| gemini_canvas::build_video_job_poll_request(bootstrap, source_path, job_id));
    let completion_request = gemini_canvas::build_video_completion_followup_request(
        bootstrap,
        source_path,
        conversation_id,
    )?;
    let metadata_request = gemini_canvas::build_video_metadata_followup_request(
        bootstrap,
        source_path,
        conversation_id,
        response_id,
    )?;
    Ok(GeminiCanvasVideoCompletionRequests {
        job_id,
        job_poll_request,
        completion_request,
        metadata_request,
    })
}

pub(crate) fn extract_gemini_canvas_video_operation_download_uri<'a>(
    provider: &str,
    poll: &'a Value,
) -> Result<&'a str, GatewayError> {
    let video = poll
        .pointer("/response/generateVideoResponse/generatedSamples/0/video")
        .or_else(|| poll.pointer("/response/generatedVideos/0/video"))
        .or_else(|| poll.pointer("/response/generated_videos/0/video"))
        .ok_or_else(|| {
            GatewayError::server_error(
                "Gemini Canvas video operation completed without a downloadable video payload.",
            )
            .with_provider(provider)
            .with_code("gemini_canvas_no_video_asset")
        })?;
    video
        .get("uri")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::server_error("Gemini Canvas video operation completed without video.uri.")
                .with_provider(provider)
                .with_code("gemini_canvas_no_video_asset")
        })
}

pub(crate) fn plan_gemini_canvas_video_completion_attempt_sleep(
    completion_pending: bool,
    metadata_pending: bool,
    attempt: usize,
) -> Option<Duration> {
    if !completion_pending && !metadata_pending && attempt >= 3 {
        return None;
    }

    Some(if completion_pending || metadata_pending {
        Duration::from_secs(8)
    } else {
        Duration::from_secs(4)
    })
}

pub(crate) fn build_gemini_canvas_video_completion_missing_asset_error(
    source_path: &str,
    conversation_id: &str,
    response_id: &str,
    attempt: usize,
    job_id: Option<&str>,
    last_completion_body: Option<&str>,
    last_metadata_body: Option<&str>,
) -> GatewayError {
    let completion_preview = last_completion_body
        .map(|body| compact_response_preview(body, 320))
        .unwrap_or_else(|| "<none>".to_string());
    let metadata_preview = last_metadata_body
        .map(|body| compact_response_preview(body, 320))
        .unwrap_or_else(|| "<none>".to_string());

    GatewayError::server_error(sanitize_provider_error_message(&format!(
        "Gemini Canvas pure HTTP video completion follow-up did not expose a usable video asset. source_path={source_path}; conversation_id={conversation_id}; response_id={response_id}; attempts={attempt}; job_id={}; last_completion_preview={completion_preview}; last_metadata_preview={metadata_preview}",
        job_id.unwrap_or("<none>")
    )))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_video_completion_followup_missing_asset")
}

pub(crate) fn gemini_canvas_video_body_has_usable_asset(body: &str) -> bool {
    gemini_canvas::extract_stream_generate_media_assets(
        body,
        gemini_canvas::GeminiCanvasMediaOperation::Video,
    )
    .is_ok()
        || gemini_canvas::extract_page_blob_media_assets(
            body,
            gemini_canvas::GeminiCanvasMediaOperation::Video,
        )
        .is_ok()
}

pub(crate) fn classify_gemini_canvas_video_stage_body(
    body: String,
) -> Result<String, GeminiCanvasVideoStageProgress> {
    if gemini_canvas_video_body_has_usable_asset(&body) {
        return Ok(body);
    }

    let pending = gemini_canvas::response_indicates_video_generation_pending(&body);
    Err(GeminiCanvasVideoStageProgress { body, pending })
}
