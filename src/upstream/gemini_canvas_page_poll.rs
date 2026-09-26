use super::followup_target::GeminiCanvasPageTargetMode;
use crate::error::{sanitize_provider_error_message, GatewayError};
use crate::protocol::gemini_canvas;
use crate::upstream::gemini_canvas_runtime_error_helpers::{
    compact_response_preview, summarize_gateway_error,
};
use std::time::Duration;

pub(crate) fn gemini_canvas_conversation_page_missing_asset_error(
    operation: gemini_canvas::GeminiCanvasMediaOperation,
    locator_mode: GeminiCanvasPageTargetMode,
    app_path: &str,
    attempt_count: usize,
    poll_budget_secs: u64,
    failures: &str,
    last_page_preview: &str,
) -> GatewayError {
    let code = match operation {
        gemini_canvas::GeminiCanvasMediaOperation::Image => {
            "gemini_canvas_page_missing_image_asset"
        }
        gemini_canvas::GeminiCanvasMediaOperation::Music => {
            "gemini_canvas_page_missing_music_asset"
        }
        gemini_canvas::GeminiCanvasMediaOperation::Video => {
            "gemini_canvas_page_missing_video_asset"
        }
    };
    GatewayError::server_error(sanitize_provider_error_message(&format!(
        "Gemini Canvas conversation page poll did not expose a usable media asset. locator_mode={}; app_path={app_path}; attempts={attempt_count}; poll_budget_secs={poll_budget_secs}; failures={failures}; last_page_preview={last_page_preview}",
        locator_mode.as_str(),
    )))
    .with_provider("gemini_canvas_compatible")
    .with_code(code)
}

pub(crate) fn gemini_canvas_conversation_page_poll_stage_entry(
    attempt: usize,
    label: &str,
    detail: impl AsRef<str>,
) -> String {
    sanitize_provider_error_message(&format!(
        "attempt={} {}={}",
        attempt,
        label,
        detail.as_ref()
    ))
}

pub(crate) fn gemini_canvas_conversation_page_poll_refresh_body_entry(
    attempt: usize,
    body: &str,
) -> String {
    gemini_canvas_conversation_page_poll_stage_entry(
        attempt,
        "page_refresh",
        compact_response_preview(body, 220),
    )
}

pub(crate) fn gemini_canvas_conversation_page_poll_refresh_error_entry(
    attempt: usize,
    error: &GatewayError,
) -> String {
    gemini_canvas_conversation_page_poll_stage_entry(
        attempt,
        "page_refresh",
        summarize_gateway_error(error),
    )
}

pub(crate) fn gemini_canvas_conversation_page_poll_url_entry(
    attempt: usize,
    page_url: &str,
    label: &str,
    detail: impl AsRef<str>,
) -> String {
    sanitize_provider_error_message(&format!(
        "attempt={} url={} {}={}",
        attempt,
        page_url,
        label,
        detail.as_ref()
    ))
}

pub(crate) fn gemini_canvas_conversation_page_poll_fetch_error_entry(
    attempt: usize,
    page_url: &str,
    error: &GatewayError,
) -> String {
    gemini_canvas_conversation_page_poll_url_entry(
        attempt,
        page_url,
        "fetch",
        summarize_gateway_error(error),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeminiCanvasConversationPagePollBodyExtractFailure {
    pub(crate) failure_entry: String,
    pub(crate) page_preview: String,
}

pub(crate) fn try_extract_gemini_canvas_conversation_page_poll_assets_from_body(
    attempt: usize,
    page_url: &str,
    operation: gemini_canvas::GeminiCanvasMediaOperation,
    page_body: String,
) -> Result<
    (Vec<gemini_canvas::GeminiCanvasMediaAsset>, String),
    GeminiCanvasConversationPagePollBodyExtractFailure,
> {
    match gemini_canvas::extract_page_blob_media_assets(&page_body, operation) {
        Ok(assets) => Ok((assets, page_body)),
        Err(error) => Err(GeminiCanvasConversationPagePollBodyExtractFailure {
            failure_entry: gemini_canvas_conversation_page_poll_url_entry(
                attempt,
                page_url,
                "extract",
                summarize_gateway_error(&error),
            ),
            page_preview: compact_response_preview(&page_body, 220),
        }),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeminiCanvasConversationPagePollTiming {
    pub(crate) poll_budget: Duration,
    pub(crate) sleep_between_attempts: Duration,
    pub(crate) max_fetch_timeout: Duration,
}

pub(crate) fn plan_gemini_canvas_conversation_page_poll_timing(
    operation: gemini_canvas::GeminiCanvasMediaOperation,
    has_bootstrap_override: bool,
    force_root_app_followup: bool,
    timeout: Duration,
) -> GeminiCanvasConversationPagePollTiming {
    let poll_budget = match operation {
        gemini_canvas::GeminiCanvasMediaOperation::Image => {
            if has_bootstrap_override {
                timeout
                    .min(Duration::from_secs(45))
                    .max(Duration::from_secs(24))
            } else if force_root_app_followup {
                timeout
                    .min(Duration::from_secs(30))
                    .max(Duration::from_secs(12))
            } else {
                timeout
                    .min(Duration::from_secs(12))
                    .max(Duration::from_secs(5))
            }
        }
        gemini_canvas::GeminiCanvasMediaOperation::Music => timeout
            .min(Duration::from_secs(90))
            .max(Duration::from_secs(45)),
        gemini_canvas::GeminiCanvasMediaOperation::Video => timeout
            .min(Duration::from_secs(120))
            .max(Duration::from_secs(45)),
    };

    let sleep_between_attempts = match operation {
        gemini_canvas::GeminiCanvasMediaOperation::Image => {
            if has_bootstrap_override || force_root_app_followup {
                Duration::from_secs(3)
            } else {
                Duration::from_secs(2)
            }
        }
        gemini_canvas::GeminiCanvasMediaOperation::Music
        | gemini_canvas::GeminiCanvasMediaOperation::Video => Duration::from_secs(4),
    };

    let max_fetch_timeout = match operation {
        gemini_canvas::GeminiCanvasMediaOperation::Image => {
            if has_bootstrap_override {
                Duration::from_secs(18)
            } else if force_root_app_followup {
                Duration::from_secs(10)
            } else {
                Duration::from_secs(12)
            }
        }
        gemini_canvas::GeminiCanvasMediaOperation::Music
        | gemini_canvas::GeminiCanvasMediaOperation::Video => Duration::from_secs(20),
    };

    GeminiCanvasConversationPagePollTiming {
        poll_budget,
        sleep_between_attempts,
        max_fetch_timeout,
    }
}

pub(crate) fn resolve_gemini_canvas_conversation_page_poll_remaining(
    poll_budget: Duration,
    elapsed: Duration,
) -> Option<Duration> {
    poll_budget
        .checked_sub(elapsed)
        .filter(|remaining| *remaining > Duration::from_secs(1))
}

pub(crate) fn should_sleep_after_gemini_canvas_conversation_page_poll_attempt(
    poll_budget: Duration,
    elapsed: Duration,
    sleep_between_attempts: Duration,
) -> bool {
    match poll_budget.checked_sub(elapsed) {
        Some(remaining) => remaining > sleep_between_attempts + Duration::from_secs(1),
        None => false,
    }
}

pub(crate) fn resolve_gemini_canvas_conversation_page_poll_refresh_target<'a>(
    operation: gemini_canvas::GeminiCanvasMediaOperation,
    has_bootstrap_override: bool,
    force_root_app_followup: bool,
    conversation_url: Option<&'a str>,
    app_bootstrap_url: &'a str,
) -> Option<&'a str> {
    if operation != gemini_canvas::GeminiCanvasMediaOperation::Image {
        return None;
    }
    if !has_bootstrap_override && !force_root_app_followup {
        return None;
    }

    Some(conversation_url.unwrap_or(app_bootstrap_url))
}
