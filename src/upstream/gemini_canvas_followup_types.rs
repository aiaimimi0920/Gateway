#[cfg(test)]
use std::time::Duration;
use std::time::SystemTime;

use serde_json::Value;

use crate::error::{sanitize_provider_error_message, GatewayError};
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::protocol::{gemini_canvas, gemini_web};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::gemini_canvas_asset_helpers::gemini_canvas_asset_url_is_caller_usable;
use crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel;
use crate::upstream::gemini_canvas_image_edit_local_helpers::gemini_canvas_image_edit_force_heavy_only_enabled;
use crate::upstream::gemini_canvas_runtime_error_helpers::compact_response_preview;

#[path = "gemini_canvas_image_normalization.rs"]
mod image_normalization;

#[path = "gemini_canvas_followup_target.rs"]
mod followup_target;
#[cfg(test)]
use followup_target::*;
pub(crate) use followup_target::{
    build_gemini_canvas_followup_bootstrap_candidates, build_gemini_canvas_page_poll_urls,
    classify_gemini_canvas_page_target_mode, prepare_gemini_canvas_page_seed,
    GeminiCanvasFollowupTarget, GeminiCanvasPageTargetMode,
};

#[path = "gemini_canvas_page_poll.rs"]
mod page_poll;
#[cfg(test)]
use page_poll::*;
pub(crate) use page_poll::{
    gemini_canvas_conversation_page_missing_asset_error,
    gemini_canvas_conversation_page_poll_fetch_error_entry,
    gemini_canvas_conversation_page_poll_refresh_body_entry,
    gemini_canvas_conversation_page_poll_refresh_error_entry,
    plan_gemini_canvas_conversation_page_poll_timing,
    resolve_gemini_canvas_conversation_page_poll_refresh_target,
    resolve_gemini_canvas_conversation_page_poll_remaining,
    should_sleep_after_gemini_canvas_conversation_page_poll_attempt,
    try_extract_gemini_canvas_conversation_page_poll_assets_from_body,
};

#[path = "gemini_canvas_video_completion.rs"]
mod video_completion;
#[cfg(test)]
use video_completion::*;
pub(crate) use video_completion::{
    build_gemini_canvas_video_accepted_response_from_body,
    build_gemini_canvas_video_completion_requests, classify_gemini_canvas_video_stage_body,
    extract_gemini_canvas_video_operation_download_uri, gemini_canvas_video_body_has_usable_asset,
    GeminiCanvasVideoCompletionAttemptOutcome, GeminiCanvasVideoCompletionAttemptState,
    GeminiCanvasVideoCompletionPollState, GeminiCanvasVideoCompletionRequests,
};

#[path = "gemini_canvas_video_errors.rs"]
mod video_errors;
pub(crate) use video_errors::{
    build_gemini_canvas_direct_http_video_missing_asset_error,
    gemini_canvas_modular_video_unsupported_count_error,
    gemini_canvas_program_video_browser_fallback_forbidden_error,
    gemini_canvas_program_video_invoke_target_missing_error,
    gemini_canvas_program_video_no_key_empty_body_error,
    gemini_canvas_program_video_no_key_invalid_json_error,
    gemini_canvas_program_video_no_key_poll_empty_body_error,
    gemini_canvas_program_video_no_key_poll_invalid_json_error,
    gemini_canvas_program_video_no_key_request_contract_missing_error,
    gemini_canvas_program_video_no_key_request_exhausted_error,
    gemini_canvas_video_followup_missing_locator_error, gemini_canvas_video_missing_asset_error,
    gemini_canvas_video_missing_operation_error, gemini_canvas_video_music_modality_mismatch_error,
    gemini_canvas_video_operation_timeout_error, gemini_canvas_video_unsupported_count_error,
};

#[path = "gemini_canvas_image_recovery.rs"]
mod image_recovery;
pub(crate) use image_recovery::{
    build_gemini_canvas_image_recovery_strategy, GeminiCanvasDirectHttpImageJsonAction,
    GeminiCanvasDirectHttpImageJsonPolicy, GeminiCanvasImageRecoveryMode,
    GeminiCanvasImageRecoveryStrategy, GeminiCanvasImageTemplateRetryAction,
};

#[cfg(test)]
#[path = "gemini_canvas_followup_test_support.rs"]
mod followup_test_support;
#[cfg(test)]
#[path = "gemini_canvas_image_recovery_tests.rs"]
mod image_recovery_tests;

#[derive(Clone, Debug)]
pub(crate) struct GeminiCanvasImageEditFollowupContext {
    pub(crate) prompt: String,
    pub(crate) request_started_at: SystemTime,
    pub(crate) locale_hint: Option<String>,
    pub(crate) signaler_session: Option<gemini_canvas::GeminiCanvasPureHttpSession>,
    pub(crate) signaler_channel: Option<GeminiCanvasSignalerChannel>,
    pub(crate) signaler_app_urls: Vec<String>,
    pub(crate) signaler_app_url: Option<String>,
    pub(crate) signaler_conversation_id: Option<String>,
    pub(crate) signaler_response_id: Option<String>,
}

pub(crate) struct GeminiCanvasImageEditPostAckFollowupResult {
    pub(crate) preview: String,
    pub(crate) page_body: Option<String>,
}

pub(crate) struct GeminiCanvasMediaCaptureParityPreflightResult {
    pub(crate) selected_bootstrap_body: String,
    pub(crate) maziqc_probe_body: Option<String>,
    pub(crate) maziqc_full_body: Option<String>,
    pub(crate) o30o0e_body: Option<String>,
    pub(crate) k4wwud_body: Option<String>,
}

pub(crate) struct GeminiCanvasMediaFollowupPreflightPlan {
    pub(crate) mode_index: i64,
    pub(crate) batchexecute_header_id: Option<String>,
    pub(crate) followup_model_header: String,
    pub(crate) activity_request: gemini_web::GeminiWebRequest,
    pub(crate) followup_request: gemini_web::GeminiWebRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeminiCanvasMediaFollowupPreflightStrategy {
    ParityThenLegacy,
    LegacyOnly,
}

impl GeminiCanvasMediaFollowupPreflightStrategy {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ParityThenLegacy => "parity_then_legacy",
            Self::LegacyOnly => "legacy_only",
        }
    }
}

pub(crate) struct GeminiCanvasMediaFollowupPreflightContract {
    pub(crate) preflight_plan: GeminiCanvasMediaFollowupPreflightPlan,
    pub(crate) strategy: GeminiCanvasMediaFollowupPreflightStrategy,
    pub(crate) selected_bootstrap_body: Option<String>,
}

pub(crate) enum GeminiCanvasMediaFollowupPreflightOutcome {
    Completed(String),
    Ready(GeminiCanvasMediaFollowupPreflightContract),
}

pub(crate) struct GeminiCanvasMediaFollowupAttemptState {
    pub(crate) last_body: Option<String>,
    pub(crate) last_error: Option<GatewayError>,
}

pub(crate) enum GeminiCanvasMediaFollowupAttemptOutcome {
    Completed(String),
    Incomplete(GeminiCanvasMediaFollowupAttemptState),
}

pub(crate) struct GeminiCanvasMediaFollowupContext {
    pub(crate) bootstrap: gemini_web::GeminiWebBootstrap,
    pub(crate) bootstrap_page_url: String,
    pub(crate) session: gemini_canvas::GeminiCanvasPureHttpSession,
    pub(crate) followup_target: GeminiCanvasFollowupTarget,
}

pub(crate) struct GeminiCanvasDirectHttpImageContext {
    pub(crate) mode_index: i64,
    pub(crate) request_started_at: SystemTime,
    pub(crate) initial_stream_allows_replay_template: bool,
    pub(crate) image_edit_uploads: Option<Vec<gemini_canvas::GeminiCanvasImageEditUpload>>,
    pub(crate) image_edit_followup_context: Option<GeminiCanvasImageEditFollowupContext>,
    pub(crate) image_json_policy: GeminiCanvasDirectHttpImageJsonPolicy,
}

pub(crate) async fn prepare_gemini_canvas_direct_http_image_context(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    prompt: &str,
) -> Result<GeminiCanvasDirectHttpImageContext, GatewayError> {
    let mode_index =
        gemini_canvas::stream_generate_mode_index(gemini_canvas::GeminiCanvasMediaOperation::Image);
    let image_edit_uploads = if req.endpoint_kind == EndpointKind::ImagesEdits {
        Some(image_normalization::normalize_uploads(&req.raw_body).await?)
    } else {
        None
    };
    let request_started_at = SystemTime::now();
    let image_edit_followup_context = if req.endpoint_kind == EndpointKind::ImagesEdits {
        Some(GeminiCanvasImageEditFollowupContext {
            prompt: prompt.to_string(),
            request_started_at,
            locale_hint: None,
            signaler_session: None,
            signaler_channel: None,
            signaler_app_urls: Vec::new(),
            signaler_app_url: None,
            signaler_conversation_id: None,
            signaler_response_id: None,
        })
    } else {
        None
    };
    let image_json_policy = GeminiCanvasDirectHttpImageJsonPolicy::from_request(
        payload,
        req,
        gemini_canvas::GeminiCanvasMediaOperation::Image,
    )?;
    let initial_stream_allows_replay_template = !(req.endpoint_kind == EndpointKind::ImagesEdits
        && gemini_canvas_image_edit_force_heavy_only_enabled());

    Ok(GeminiCanvasDirectHttpImageContext {
        mode_index,
        request_started_at,
        initial_stream_allows_replay_template,
        image_edit_uploads,
        image_edit_followup_context,
        image_json_policy,
    })
}

pub(crate) enum GeminiCanvasDirectHttpImagePrimaryResult {
    StreamBody(String),
    FinalResponse(Value),
}

pub(crate) enum GeminiCanvasImageResponsePlan<'a> {
    FinalResponse(Value),
    Materialize(Vec<&'a gemini_canvas::GeminiCanvasMediaAsset>),
}

pub(crate) fn plan_gemini_canvas_image_response<'a>(
    req: &CanonicalRelayRequest,
    prompt: &str,
    image_assets: &'a [gemini_canvas::GeminiCanvasMediaAsset],
    provider: &str,
    missing_asset_message: &'static str,
    missing_asset_code: &'static str,
) -> Result<GeminiCanvasImageResponsePlan<'a>, GatewayError> {
    if image_assets.is_empty() {
        return Err(GatewayError::server_error(missing_asset_message)
            .with_provider(provider)
            .with_code(missing_asset_code));
    }

    if gemini_canvas::prefers_url_response(req)?
        && image_assets
            .iter()
            .all(|asset| gemini_canvas_asset_url_is_caller_usable(&asset.url))
    {
        return Ok(GeminiCanvasImageResponsePlan::FinalResponse(
            gemini_canvas::build_openai_images_response_from_urls(req, prompt, image_assets)?,
        ));
    }

    Ok(GeminiCanvasImageResponsePlan::Materialize(
        image_assets
            .iter()
            .take(gemini_canvas::requested_output_count(req))
            .collect(),
    ))
}

pub(crate) fn build_gemini_canvas_media_followup_preflight_plan(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
    operation: gemini_canvas::GeminiCanvasMediaOperation,
) -> Result<GeminiCanvasMediaFollowupPreflightPlan, GatewayError> {
    let mode_index = gemini_canvas::stream_generate_mode_index(operation);
    let batchexecute_header_id = Some(gemini_canvas::new_batchexecute_header_id());
    let followup_model_header = gemini_canvas::build_text_batchexecute_model_header_variant(
        Some(gemini_canvas::GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID),
        batchexecute_header_id.as_deref(),
        true,
        false,
    );
    let activity_request = gemini_canvas::build_text_batchexecute_request(
        "ESY5D",
        serde_json::json!([[["bard_activity_enabled"]]]),
        bootstrap,
        source_path,
    )?;
    let followup_request =
        gemini_canvas::build_text_bootstrap_preflight_request(bootstrap, source_path)?;

    Ok(GeminiCanvasMediaFollowupPreflightPlan {
        mode_index,
        batchexecute_header_id,
        followup_model_header,
        activity_request,
        followup_request,
    })
}

pub(crate) fn build_gemini_canvas_media_followup_missing_asset_error(
    provider: &str,
    operation: gemini_canvas::GeminiCanvasMediaOperation,
    followup_target: &GeminiCanvasFollowupTarget,
    bootstrap_page_url: &str,
    attempt: usize,
    body: &str,
) -> GatewayError {
    let operation_label = match operation {
        gemini_canvas::GeminiCanvasMediaOperation::Image => "image",
        gemini_canvas::GeminiCanvasMediaOperation::Music => "music",
        gemini_canvas::GeminiCanvasMediaOperation::Video => "video",
    };
    GatewayError::service_unavailable(sanitize_provider_error_message(&format!(
        "Gemini Canvas pure HTTP {operation_label} aPya6c follow-up returned a body but did not expose a usable media asset. locator_mode={}; app_path={}; response_id={}; conversation_id={}; bootstrap_page={}; attempt={}; body_preview={}",
        followup_target.mode.as_str(),
        followup_target.source_path,
        followup_target.response_id(),
        followup_target.conversation_id(),
        bootstrap_page_url,
        attempt,
        compact_response_preview(body, 220)
    )))
    .with_provider(provider)
    .with_code("gemini_canvas_media_followup_missing_asset")
}

pub(crate) fn gemini_canvas_media_followup_failed_error(
    provider: &str,
    followup_target: &GeminiCanvasFollowupTarget,
    bootstrap_page_url: &str,
    upstream: &str,
) -> GatewayError {
    GatewayError::service_unavailable(sanitize_provider_error_message(&format!(
        "Gemini Canvas pure HTTP media aPya6c follow-up failed. locator_mode={}; app_path={}; response_id={}; conversation_id={}; bootstrap_page={}; upstream={upstream}",
        followup_target.mode.as_str(),
        followup_target.source_path,
        followup_target.response_id(),
        followup_target.conversation_id(),
        bootstrap_page_url,
    )))
    .with_provider(provider)
    .with_code("gemini_canvas_media_followup_failed")
}

pub(crate) fn gemini_canvas_media_followup_bootstrap_failed_error(
    locator_mode: GeminiCanvasPageTargetMode,
    app_path: &str,
    attempted_urls: &str,
    failures: &str,
) -> GatewayError {
    GatewayError::service_unavailable(sanitize_provider_error_message(&format!(
        "Gemini Canvas media follow-up could not bootstrap the conversation page. locator_mode={}; app_path={app_path}; attempted_urls={attempted_urls}; failures={failures}",
        locator_mode.as_str(),
    )))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_media_followup_bootstrap_failed")
}

pub(crate) fn finalize_gemini_canvas_media_followup_attempt_state(
    provider: &str,
    followup_target: &GeminiCanvasFollowupTarget,
    bootstrap_page_url: &str,
    state: GeminiCanvasMediaFollowupAttemptState,
) -> Result<String, GatewayError> {
    if let Some(mut error) = state.last_error {
        error.message = sanitize_provider_error_message(&error.message);
        return Err(error);
    }
    if let Some(body) = state.last_body {
        return Ok(body);
    }
    Err(gemini_canvas_media_followup_missing_result_error(
        provider,
        followup_target,
        bootstrap_page_url,
    ))
}

pub(crate) fn gemini_canvas_media_followup_missing_result_error(
    provider: &str,
    followup_target: &GeminiCanvasFollowupTarget,
    bootstrap_page_url: &str,
) -> GatewayError {
    GatewayError::service_unavailable(sanitize_provider_error_message(&format!(
        "Gemini Canvas pure HTTP media aPya6c follow-up produced neither body nor explicit error. locator_mode={}; app_path={}; response_id={}; conversation_id={}; bootstrap_page={}",
        followup_target.mode.as_str(),
        followup_target.source_path,
        followup_target.response_id(),
        followup_target.conversation_id(),
        bootstrap_page_url
    )))
    .with_provider(provider)
    .with_code("gemini_canvas_media_followup_missing_result")
}

#[cfg(test)]
#[path = "gemini_canvas_followup_error_tests.rs"]
mod followup_error_tests;

#[cfg(test)]
#[path = "gemini_canvas_page_poll_tests.rs"]
mod page_poll_tests;

#[cfg(test)]
#[path = "gemini_canvas_followup_target_tests.rs"]
mod followup_target_tests;

#[cfg(test)]
#[path = "gemini_canvas_followup_plan_tests.rs"]
mod followup_plan_tests;

#[cfg(test)]
#[path = "gemini_canvas_video_completion_tests.rs"]
mod video_completion_tests;

#[cfg(test)]
#[path = "gemini_canvas_diagnostic_tests.rs"]
mod diagnostic_tests;
