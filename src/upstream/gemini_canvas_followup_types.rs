use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use regex::Regex;
use serde_json::Value;
use tracing::debug;

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::protocol::{gemini_canvas, gemini_web};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::gemini::canvas_program_web_reverse as gemini_canvas_program_web_reverse_modular;
use crate::upstream::gemini_canvas_asset_helpers::gemini_canvas_asset_url_is_caller_usable;
use crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel;
use crate::upstream::gemini_canvas_image_edit_local_helpers::gemini_canvas_image_edit_force_heavy_only_enabled;
use crate::upstream::gemini_canvas_runtime_error_helpers::summarize_gateway_error;
use crate::upstream::gemini_canvas_runtime_helpers::gemini_canvas_page_base_url;

fn compact_response_preview(body_text: &str, max_chars: usize) -> String {
    let mut chars = body_text.chars();
    let preview: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{preview}...")
    } else {
        preview
    }
}

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

pub(crate) enum GeminiCanvasMediaFollowupAttemptOutcome {
    Completed(String),
    Incomplete(GeminiCanvasMediaFollowupAttemptState),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeminiCanvasImageRecoveryMode {
    Generation,
    EditAsync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GeminiCanvasImageRecoveryStrategy {
    pub(crate) mode: GeminiCanvasImageRecoveryMode,
    pub(crate) template_retry_action: GeminiCanvasImageTemplateRetryAction,
}

pub(crate) fn resolve_gemini_canvas_image_recovery_mode(
    endpoint_kind: EndpointKind,
) -> GeminiCanvasImageRecoveryMode {
    if endpoint_kind == EndpointKind::ImagesEdits {
        GeminiCanvasImageRecoveryMode::EditAsync
    } else {
        GeminiCanvasImageRecoveryMode::Generation
    }
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

pub(crate) fn prepare_gemini_canvas_direct_http_image_context(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    prompt: &str,
) -> Result<GeminiCanvasDirectHttpImageContext, GatewayError> {
    let mode_index =
        gemini_canvas::stream_generate_mode_index(gemini_canvas::GeminiCanvasMediaOperation::Image);
    let image_edit_uploads = if req.endpoint_kind == EndpointKind::ImagesEdits {
        Some(gemini_canvas::extract_image_edit_uploads(req)?)
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

pub(crate) struct GeminiCanvasPageSeed {
    pub(crate) locator: Option<gemini_canvas::GeminiCanvasStreamGenerateLocator>,
    pub(crate) conversation_page_url: Option<String>,
    pub(crate) app_bootstrap_url: String,
    pub(crate) share_bootstrap_url: String,
    pub(crate) prefer_root_app_path: bool,
    pub(crate) has_page_url_override: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeminiCanvasPageTargetMode {
    ForcedRootApp,
    SignalerPageBootstrap,
    ConversationListRecovered,
    Resolved,
    RootAppFallback,
}

impl GeminiCanvasPageTargetMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ForcedRootApp => "forced_root_app",
            Self::SignalerPageBootstrap => "signaler_page_bootstrap",
            Self::ConversationListRecovered => "conversation_list_recovered",
            Self::Resolved => "resolved",
            Self::RootAppFallback => "root_app_fallback",
        }
    }
}

pub(crate) struct GeminiCanvasFollowupTarget {
    pub(crate) locator: Option<gemini_canvas::GeminiCanvasStreamGenerateLocator>,
    pub(crate) source_path: String,
    pub(crate) mode: GeminiCanvasPageTargetMode,
}

impl GeminiCanvasFollowupTarget {
    pub(crate) fn from_bootstrap(
        prefer_root_app_path: bool,
        bootstrap_app_page_path: Option<&str>,
        locator: Option<gemini_canvas::GeminiCanvasStreamGenerateLocator>,
        initial_mode: GeminiCanvasPageTargetMode,
    ) -> Self {
        let source_path = if prefer_root_app_path {
            gemini_web::GEMINI_WEB_DEFAULT_APP_PATH.to_string()
        } else {
            bootstrap_app_page_path
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| locator.as_ref().map(|locator| locator.app_path.clone()))
                .unwrap_or_else(|| gemini_web::GEMINI_WEB_DEFAULT_APP_PATH.to_string())
        };
        Self {
            locator,
            source_path,
            mode: initial_mode,
        }
    }

    pub(crate) fn adopt_video_recovered_locator(
        &mut self,
        locator: gemini_canvas::GeminiCanvasStreamGenerateLocator,
    ) {
        self.source_path = locator.app_path.clone();
        self.locator = Some(locator);
        self.mode = GeminiCanvasPageTargetMode::ConversationListRecovered;
    }

    pub(crate) fn response_id(&self) -> &str {
        self.locator
            .as_ref()
            .map(|locator| locator.response_id.as_str())
            .unwrap_or("<none>")
    }

    pub(crate) fn conversation_id(&self) -> &str {
        self.locator
            .as_ref()
            .map(|locator| locator.conversation_id.as_str())
            .unwrap_or("<none>")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeminiCanvasImageTemplateRetryAction {
    ReturnOriginal,
    ReturnOriginalWithLog(&'static str),
    RetryLegacyTemplate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GeminiCanvasDirectHttpImageJsonAction {
    Skip,
    TryJson,
    ReturnOriginal,
    ReturnOriginalWithSummary {
        context_key: &'static str,
        summary: String,
    },
    TryJsonWithErrorContext {
        context_key: &'static str,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct GeminiCanvasDirectHttpImageJsonPolicy {
    pub(crate) fallback_enabled: bool,
    pub(crate) inline_preferred: bool,
    pub(crate) prefill_failure_summary: Option<String>,
}

impl GeminiCanvasDirectHttpImageJsonPolicy {
    pub(crate) fn from_request(
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
    ) -> Result<Self, GatewayError> {
        let program_owned = payload.adapter == "gemini_canvas_program_web_reverse_compatible";
        let fallback_enabled = operation == gemini_canvas::GeminiCanvasMediaOperation::Image
            && req.endpoint_kind != EndpointKind::ImagesEdits
            && gemini_canvas::image_json_fallback_enabled(payload)
            && !program_owned;
        let inline_preferred = fallback_enabled
            && operation == gemini_canvas::GeminiCanvasMediaOperation::Image
            && !gemini_canvas::prefers_url_response(req)?;
        Ok(Self {
            fallback_enabled,
            inline_preferred,
            prefill_failure_summary: None,
        })
    }

    pub(crate) fn initial_action(&self) -> GeminiCanvasDirectHttpImageJsonAction {
        if self.inline_preferred {
            GeminiCanvasDirectHttpImageJsonAction::TryJson
        } else {
            GeminiCanvasDirectHttpImageJsonAction::Skip
        }
    }

    pub(crate) fn note_prefill_failure(&mut self, summary: String) {
        self.prefill_failure_summary = Some(summary);
    }

    pub(crate) fn on_stream_failure(&self) -> GeminiCanvasDirectHttpImageJsonAction {
        if !self.fallback_enabled || self.inline_preferred {
            if let Some(summary) = self.prefill_failure_summary.clone() {
                return GeminiCanvasDirectHttpImageJsonAction::ReturnOriginalWithSummary {
                    context_key: "image_json_prefill_failure",
                    summary,
                };
            }
            return GeminiCanvasDirectHttpImageJsonAction::ReturnOriginal;
        }

        GeminiCanvasDirectHttpImageJsonAction::TryJsonWithErrorContext {
            context_key: "stream_generate_failure",
        }
    }

    pub(crate) fn on_materialize_failure(&self) -> GeminiCanvasDirectHttpImageJsonAction {
        if self.inline_preferred {
            GeminiCanvasDirectHttpImageJsonAction::TryJsonWithErrorContext {
                context_key: "image_json_recovery_failure",
            }
        } else {
            GeminiCanvasDirectHttpImageJsonAction::ReturnOriginal
        }
    }
}

pub(crate) fn classify_gemini_canvas_page_target_mode(
    force_root_app_followup: bool,
    has_bootstrap_override: bool,
    locator_present: bool,
    locator_recovered_from_list: bool,
) -> GeminiCanvasPageTargetMode {
    if force_root_app_followup && !has_bootstrap_override {
        GeminiCanvasPageTargetMode::ForcedRootApp
    } else if has_bootstrap_override {
        GeminiCanvasPageTargetMode::SignalerPageBootstrap
    } else if locator_recovered_from_list {
        GeminiCanvasPageTargetMode::ConversationListRecovered
    } else if locator_present {
        GeminiCanvasPageTargetMode::Resolved
    } else {
        GeminiCanvasPageTargetMode::RootAppFallback
    }
}

pub(crate) fn classify_gemini_canvas_image_template_retry(
    recovery_mode: GeminiCanvasImageRecoveryMode,
    initial_stream_allows_replay_template: bool,
    image_edit_template_async_followup_ready: bool,
) -> GeminiCanvasImageTemplateRetryAction {
    if recovery_mode == GeminiCanvasImageRecoveryMode::EditAsync {
        return if image_edit_template_async_followup_ready {
            GeminiCanvasImageTemplateRetryAction::ReturnOriginalWithLog(
                "gemini canvas direct HTTP image-edit template lane reached async-followup-ready; suppressing legacy heavy retry",
            )
        } else {
            GeminiCanvasImageTemplateRetryAction::ReturnOriginalWithLog(
                "gemini canvas direct HTTP image-edit template lane ended without a usable asset; suppressing legacy heavy retry to preserve caller-visible transport budget",
            )
        };
    }
    if !initial_stream_allows_replay_template {
        return GeminiCanvasImageTemplateRetryAction::ReturnOriginal;
    }
    GeminiCanvasImageTemplateRetryAction::RetryLegacyTemplate
}

pub(crate) fn build_gemini_canvas_image_recovery_strategy(
    endpoint_kind: EndpointKind,
    initial_stream_allows_replay_template: bool,
    body_text: &str,
) -> GeminiCanvasImageRecoveryStrategy {
    let mode = resolve_gemini_canvas_image_recovery_mode(endpoint_kind);
    let image_edit_template_async_followup_ready = mode == GeminiCanvasImageRecoveryMode::EditAsync
        && initial_stream_allows_replay_template
        && gemini_canvas::stream_generate_indicates_image_edit_async_followup_ready(body_text);
    let template_retry_action = classify_gemini_canvas_image_template_retry(
        mode,
        initial_stream_allows_replay_template,
        image_edit_template_async_followup_ready,
    );
    GeminiCanvasImageRecoveryStrategy {
        mode,
        template_retry_action,
    }
}

pub(crate) fn build_gemini_canvas_followup_bootstrap_candidates(
    prefer_root_app_path: bool,
    conversation_url: Option<&str>,
    app_bootstrap_url: &str,
    share_bootstrap_url: &str,
) -> Vec<String> {
    let mut candidates = vec![app_bootstrap_url.to_string()];
    if !prefer_root_app_path {
        if let Some(url) = conversation_url {
            if !candidates.iter().any(|candidate| candidate == url) {
                candidates.push(url.to_string());
            }
        }
    }
    if !candidates
        .iter()
        .any(|candidate| candidate == share_bootstrap_url)
    {
        candidates.push(share_bootstrap_url.to_string());
    }
    candidates
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

pub(crate) fn resolve_gemini_canvas_followup_locator(
    payload: &ProviderAccountPayload,
    operation: gemini_canvas::GeminiCanvasMediaOperation,
    stream_body: &str,
    provider: &str,
    context_label: &str,
) -> Result<Option<gemini_canvas::GeminiCanvasStreamGenerateLocator>, GatewayError> {
    let operation_name = match operation {
        gemini_canvas::GeminiCanvasMediaOperation::Image => "image",
        gemini_canvas::GeminiCanvasMediaOperation::Music => "music",
        gemini_canvas::GeminiCanvasMediaOperation::Video => "video",
    };
    let payload_locator =
        gemini_canvas_program_web_reverse_modular::gemini_canvas_program_payload_handle_matches_operation(
            payload,
            operation_name,
        )
        .then(|| {
            gemini_canvas_program_web_reverse_modular::gemini_canvas_program_payload_locator(
                payload,
                Some(stream_body),
            )
        })
        .flatten();
    match gemini_canvas::extract_stream_generate_locator(stream_body) {
        Ok(locator) => Ok(Some(locator)),
        Err(error)
            if matches!(
                operation,
                gemini_canvas::GeminiCanvasMediaOperation::Image
                    | gemini_canvas::GeminiCanvasMediaOperation::Music
                    | gemini_canvas::GeminiCanvasMediaOperation::Video
            ) =>
        {
            if let Some(locator) = payload_locator {
                debug!(
                    provider,
                    operation = ?operation,
                    app_path = %locator.app_path,
                    conversation_id = %locator.conversation_id,
                    response_id = %locator.response_id,
                    error = %summarize_gateway_error(&error),
                    "{context_label} did not expose a locator; reusing concrete program handle from payload"
                );
                Ok(Some(locator))
            } else {
                debug!(
                    provider,
                    operation = ?operation,
                    error = %summarize_gateway_error(&error),
                    "{context_label} did not expose a locator; falling back to root /app bootstrap"
                );
                Ok(None)
            }
        }
        Err(error) => Err(error),
    }
}

pub(crate) fn prepare_gemini_canvas_page_seed(
    payload: &ProviderAccountPayload,
    runtime: &gemini_canvas::GeminiCanvasRuntime,
    operation: gemini_canvas::GeminiCanvasMediaOperation,
    stream_body: &str,
    force_root_app_followup: bool,
    bootstrap_page_url_override: Option<&str>,
    allow_payload_page_url_fallback: bool,
    provider: &str,
    context_label: &str,
) -> Result<GeminiCanvasPageSeed, GatewayError> {
    let locator = resolve_gemini_canvas_followup_locator(
        payload,
        operation,
        stream_body,
        provider,
        context_label,
    )?;
    let page_base_url = gemini_canvas_page_base_url(payload);
    let base_url = page_base_url.trim_end_matches('/');
    let app_bootstrap_url = format!("{base_url}{}", gemini_web::GEMINI_WEB_DEFAULT_APP_PATH);
    let share_bootstrap_url = gemini_canvas::direct_http_referrer(base_url, &runtime.share_id);
    let override_conversation_url = bootstrap_page_url_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let has_page_url_override = override_conversation_url.is_some();
    let conversation_page_url = override_conversation_url.clone().or_else(|| {
        locator
            .as_ref()
            .map(|locator| format!("{base_url}{}", locator.app_path))
    });
    let conversation_page_url = if allow_payload_page_url_fallback {
        conversation_page_url.or_else(|| {
            gemini_canvas_program_web_reverse_modular::gemini_canvas_program_payload_page_url(
                payload, base_url,
            )
        })
    } else {
        conversation_page_url
    };

    Ok(GeminiCanvasPageSeed {
        prefer_root_app_path: force_root_app_followup
            || (locator.is_none()
                && matches!(
                    operation,
                    gemini_canvas::GeminiCanvasMediaOperation::Image
                        | gemini_canvas::GeminiCanvasMediaOperation::Video
                )),
        locator,
        conversation_page_url,
        app_bootstrap_url,
        share_bootstrap_url,
        has_page_url_override,
    })
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

pub(crate) fn build_gemini_canvas_page_poll_urls(
    force_root_app_followup: bool,
    has_bootstrap_override: bool,
    conversation_url: Option<&str>,
    app_bootstrap_url: &str,
    share_bootstrap_url: &str,
) -> Vec<String> {
    let mut poll_urls = Vec::new();
    if let Some(url) = conversation_url {
        if !force_root_app_followup || has_bootstrap_override {
            poll_urls.push(url.to_string());
        }
    }
    if !poll_urls
        .iter()
        .any(|candidate| candidate == app_bootstrap_url)
    {
        poll_urls.push(app_bootstrap_url.to_string());
    }
    if !has_bootstrap_override
        && !poll_urls
            .iter()
            .any(|candidate| candidate == share_bootstrap_url)
    {
        poll_urls.push(share_bootstrap_url.to_string());
    }
    poll_urls
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
    GatewayError::service_unavailable(format!(
        "Gemini Canvas pure HTTP {operation_label} aPya6c follow-up returned a body but did not expose a usable media asset. locator_mode={}; app_path={}; response_id={}; conversation_id={}; bootstrap_page={}; attempt={}; body_preview={}",
        followup_target.mode.as_str(),
        followup_target.source_path,
        followup_target.response_id(),
        followup_target.conversation_id(),
        bootstrap_page_url,
        attempt,
        compact_response_preview(body, 220)
    ))
    .with_provider(provider)
    .with_code("gemini_canvas_media_followup_missing_asset")
}

pub(crate) fn gemini_canvas_media_followup_failed_error(
    provider: &str,
    followup_target: &GeminiCanvasFollowupTarget,
    bootstrap_page_url: &str,
    upstream: &str,
) -> GatewayError {
    GatewayError::service_unavailable(format!(
        "Gemini Canvas pure HTTP media aPya6c follow-up failed. locator_mode={}; app_path={}; response_id={}; conversation_id={}; bootstrap_page={}; upstream={upstream}",
        followup_target.mode.as_str(),
        followup_target.source_path,
        followup_target.response_id(),
        followup_target.conversation_id(),
        bootstrap_page_url,
    ))
    .with_provider(provider)
    .with_code("gemini_canvas_media_followup_failed")
}

pub(crate) fn gemini_canvas_media_followup_bootstrap_failed_error(
    locator_mode: GeminiCanvasPageTargetMode,
    app_path: &str,
    attempted_urls: &str,
    failures: &str,
) -> GatewayError {
    GatewayError::service_unavailable(format!(
        "Gemini Canvas media follow-up could not bootstrap the conversation page. locator_mode={}; app_path={app_path}; attempted_urls={attempted_urls}; failures={failures}",
        locator_mode.as_str(),
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_media_followup_bootstrap_failed")
}

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
    GatewayError::server_error(format!(
        "Gemini Canvas conversation page poll did not expose a usable media asset. locator_mode={}; app_path={app_path}; attempts={attempt_count}; poll_budget_secs={poll_budget_secs}; failures={failures}; last_page_preview={last_page_preview}",
        locator_mode.as_str(),
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code(code)
}

pub(crate) fn gemini_canvas_conversation_page_poll_stage_entry(
    attempt: usize,
    label: &str,
    detail: impl AsRef<str>,
) -> String {
    format!("attempt={} {}={}", attempt, label, detail.as_ref())
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
    format!(
        "attempt={} url={} {}={}",
        attempt,
        page_url,
        label,
        detail.as_ref()
    )
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

pub(crate) fn build_gemini_canvas_direct_http_video_missing_asset_error(
    provider: &str,
) -> GatewayError {
    GatewayError::server_error(
        "Gemini Canvas direct HTTP video generation completed without returning a video asset.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_no_video_asset")
}

pub(crate) fn gemini_canvas_program_video_invoke_target_missing_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas program-owned video lane is missing an explicit app invoke target.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_program_video_invoke_target_missing")
}

pub(crate) fn gemini_canvas_program_video_no_key_request_contract_missing_error(
    provider: &str,
) -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas program-owned video StreamGenerate contract is missing requestUrl/requestBody.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_program_video_no_key_contract_missing")
}

pub(crate) fn gemini_canvas_program_video_browser_fallback_forbidden_error(
    provider: &str,
) -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas program-owned video did not expose a direct no-key contract. Browser execution fallback is disabled on the default path.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_program_video_browser_fallback_forbidden")
}

pub(crate) fn gemini_canvas_program_video_no_key_empty_body_error(provider: &str) -> GatewayError {
    GatewayError::server_error(
        "Gemini Canvas preview-frame no-key video fetch returned an empty body.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_program_video_no_key_empty_body")
}

pub(crate) fn gemini_canvas_program_video_no_key_invalid_json_error(
    provider: &str,
    error: &str,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Gemini Canvas preview-frame no-key video fetch returned non-JSON body: {error}"
    ))
    .with_provider(provider)
    .with_code("gemini_canvas_program_video_no_key_invalid_json")
}

pub(crate) fn gemini_canvas_program_video_no_key_request_exhausted_error(
    provider: &str,
) -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas preview-frame no-key video invoke exhausted all candidate request URLs.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_program_video_no_key_request_exhausted")
}

pub(crate) fn gemini_canvas_program_video_no_key_poll_empty_body_error(
    provider: &str,
) -> GatewayError {
    GatewayError::server_error(
        "Gemini Canvas preview-frame no-key video poll returned an empty body.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_program_video_no_key_poll_empty_body")
}

pub(crate) fn gemini_canvas_program_video_no_key_poll_invalid_json_error(
    provider: &str,
    error: &str,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Gemini Canvas preview-frame no-key video poll returned non-JSON body: {error}"
    ))
    .with_provider(provider)
    .with_code("gemini_canvas_program_video_no_key_invalid_json")
}

pub(crate) fn gemini_canvas_video_operation_timeout_error(provider: &str) -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas video generation timed out before the operation completed.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_video_operation_timeout")
}

pub(crate) fn gemini_canvas_video_missing_operation_error(provider: &str) -> GatewayError {
    GatewayError::server_error("Gemini Canvas video generation did not return an operation name.")
        .with_provider(provider)
        .with_code("gemini_canvas_video_missing_operation")
}

pub(crate) fn gemini_canvas_video_music_modality_mismatch_error(provider: &str) -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas video generation resolved a music-branded media body instead of a real video result.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_video_music_modality_mismatch")
}

pub(crate) fn gemini_canvas_video_unsupported_count_error(provider: &str) -> GatewayError {
    GatewayError::bad_request(
        "Gemini Canvas video generation currently supports only n=1 requests.",
    )
    .with_provider(provider)
    .with_code("unsupported_gemini_canvas_video_count")
}

pub(crate) fn gemini_canvas_modular_video_unsupported_count_error(provider: &str) -> GatewayError {
    GatewayError::bad_request(
        "Gemini Canvas modular browser relay video generation currently supports only n=1 requests.",
    )
    .with_provider(provider)
    .with_code("unsupported_gemini_canvas_modular_video_count")
}

pub(crate) fn gemini_canvas_video_missing_asset_error(provider: &str) -> GatewayError {
    GatewayError::server_error(
        "Gemini Canvas video generation completed without a downloadable video asset.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_no_video_asset")
}

pub(crate) fn gemini_canvas_video_followup_missing_locator_error(
    provider: &str,
    source_path: &str,
    bootstrap_page_url: &str,
) -> GatewayError {
    GatewayError::service_unavailable(format!(
        "Gemini Canvas video follow-up could not recover a usable conversation locator from the StreamGenerate response or recent conversation list. app_path={source_path}; bootstrap_page={bootstrap_page_url}"
    ))
    .with_provider(provider)
    .with_code("gemini_canvas_stream_generate_missing_conversation_id")
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

    GatewayError::server_error(format!(
        "Gemini Canvas pure HTTP video completion follow-up did not expose a usable video asset. source_path={source_path}; conversation_id={conversation_id}; response_id={response_id}; attempts={attempt}; job_id={}; last_completion_preview={completion_preview}; last_metadata_preview={metadata_preview}",
        job_id.unwrap_or("<none>")
    ))
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

pub(crate) fn finalize_gemini_canvas_media_followup_attempt_state(
    provider: &str,
    followup_target: &GeminiCanvasFollowupTarget,
    bootstrap_page_url: &str,
    state: GeminiCanvasMediaFollowupAttemptState,
) -> Result<String, GatewayError> {
    if let Some(error) = state.last_error {
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
    GatewayError::service_unavailable(format!(
        "Gemini Canvas pure HTTP media aPya6c follow-up produced neither body nor explicit error. locator_mode={}; app_path={}; response_id={}; conversation_id={}; bootstrap_page={}",
        followup_target.mode.as_str(),
        followup_target.source_path,
        followup_target.response_id(),
        followup_target.conversation_id(),
        bootstrap_page_url
    ))
    .with_provider(provider)
    .with_code("gemini_canvas_media_followup_missing_result")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::Duration;

    use serde_json::{json, Value};

    use crate::protocol::canonical::{
        CanonicalMessage, CanonicalRelayRequest, ContentPart, MessageRole, ProtocolFamily,
    };

    fn sample_followup_target() -> GeminiCanvasFollowupTarget {
        GeminiCanvasFollowupTarget {
            locator: Some(gemini_canvas::GeminiCanvasStreamGenerateLocator {
                response_id: "resp_123".to_string(),
                conversation_id: "c_456".to_string(),
                app_path: "/app/followup".to_string(),
            }),
            source_path: "/app/followup".to_string(),
            mode: GeminiCanvasPageTargetMode::Resolved,
        }
    }

    fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: adapter.to_string(),
            base_url: base_url.to_string(),
            api_key: "sk-test".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: HashMap::new(),
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
            extra_body: None,
            session_auth: None,
            keepalive: None,
        }
    }

    fn make_request(protocol: ProtocolFamily, endpoint: EndpointKind) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: protocol,
            endpoint_kind: endpoint,
            requested_model: Some("gpt-4o".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Hello".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    fn make_gemini_bootstrap() -> gemini_web::GeminiWebBootstrap {
        gemini_web::GeminiWebBootstrap {
            access_token: Some("at-test".to_string()),
            build_label: Some("boq-gemini".to_string()),
            session_id: Some("sid-test".to_string()),
            language: "en-US".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: Some("/app/from-bootstrap".to_string()),
        }
    }

    fn make_image_asset(url: &str) -> gemini_canvas::GeminiCanvasMediaAsset {
        gemini_canvas::GeminiCanvasMediaAsset {
            kind: "image".to_string(),
            url: url.to_string(),
            mime_type: "image/png".to_string(),
            download_token: None,
            body_base64: None,
            alt: None,
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
        }
    }

    fn make_gemini_canvas_program_payload_with_handle(
        operation: Option<&str>,
    ) -> ProviderAccountPayload {
        let mut payload = make_payload(
            "gemini_canvas_program_web_reverse_compatible",
            "https://gemini.google.com",
        );
        payload.runtime_state_object_key =
            Some("credential-runtime/gemini-canvas/program/storage-state.json".to_string());
        let mut extra = HashMap::from([
            ("shareId".to_string(), json!("canvas-share-789")),
            ("appPath".to_string(), json!("/app/4abc4e7577b6149f")),
            ("conversationId".to_string(), json!("c_4abc4e7577b6149f")),
            ("responseId".to_string(), json!("r_payload")),
        ]);
        if let Some(operation) = operation {
            extra.insert("canvasProgramOperation".to_string(), json!(operation));
        }
        payload.extra_body = Some(extra);
        payload
    }

    fn assert_policy_action(
        actual: GeminiCanvasDirectHttpImageJsonAction,
        expected: GeminiCanvasDirectHttpImageJsonAction,
    ) {
        assert_eq!(actual, expected);
    }

    #[test]
    fn gemini_canvas_media_followup_failed_error_matches_contract() {
        let error = gemini_canvas_media_followup_failed_error(
            "gemini_canvas_compatible",
            &sample_followup_target(),
            "https://example.test/bootstrap",
            "timeout",
        );
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_media_followup_failed")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas pure HTTP media aPya6c follow-up failed. locator_mode=resolved; app_path=/app/followup; response_id=resp_123; conversation_id=c_456; bootstrap_page=https://example.test/bootstrap; upstream=timeout"
        );
    }

    #[test]
    fn gemini_canvas_media_followup_missing_result_error_matches_contract() {
        let error = gemini_canvas_media_followup_missing_result_error(
            "gemini_canvas_compatible",
            &sample_followup_target(),
            "https://example.test/bootstrap",
        );
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_media_followup_missing_result")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas pure HTTP media aPya6c follow-up produced neither body nor explicit error. locator_mode=resolved; app_path=/app/followup; response_id=resp_123; conversation_id=c_456; bootstrap_page=https://example.test/bootstrap"
        );
    }

    #[test]
    fn gemini_canvas_media_followup_bootstrap_failed_error_matches_contract() {
        let error = gemini_canvas_media_followup_bootstrap_failed_error(
            GeminiCanvasPageTargetMode::RootAppFallback,
            "/app/bootstrap",
            "https://a.test/app,https://b.test/share",
            "https://a.test/app: timeout | https://b.test/share: 403 challenge",
        );
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_media_followup_bootstrap_failed")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas media follow-up could not bootstrap the conversation page. locator_mode=root_app_fallback; app_path=/app/bootstrap; attempted_urls=https://a.test/app,https://b.test/share; failures=https://a.test/app: timeout | https://b.test/share: 403 challenge"
        );
    }

    #[test]
    fn gemini_canvas_program_video_invoke_target_missing_error_matches_contract() {
        let error = gemini_canvas_program_video_invoke_target_missing_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_video_invoke_target_missing")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas program-owned video lane is missing an explicit app invoke target."
        );
    }

    #[test]
    fn gemini_canvas_program_video_no_key_request_contract_missing_error_matches_contract() {
        let error = gemini_canvas_program_video_no_key_request_contract_missing_error(
            "gemini_canvas_compatible",
        );
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_video_no_key_contract_missing")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas program-owned video StreamGenerate contract is missing requestUrl/requestBody."
        );
    }

    #[test]
    fn gemini_canvas_program_video_browser_fallback_forbidden_error_matches_contract() {
        let error = gemini_canvas_program_video_browser_fallback_forbidden_error(
            "gemini_canvas_compatible",
        );
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_video_browser_fallback_forbidden")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas program-owned video did not expose a direct no-key contract. Browser execution fallback is disabled on the default path."
        );
    }

    #[test]
    fn gemini_canvas_program_video_no_key_empty_body_error_matches_contract() {
        let error = gemini_canvas_program_video_no_key_empty_body_error("gemini_canvas_compatible");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_video_no_key_empty_body")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas preview-frame no-key video fetch returned an empty body."
        );
    }

    #[test]
    fn gemini_canvas_program_video_no_key_invalid_json_error_matches_contract() {
        let error = gemini_canvas_program_video_no_key_invalid_json_error(
            "gemini_canvas_compatible",
            "expected value",
        );
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_video_no_key_invalid_json")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas preview-frame no-key video fetch returned non-JSON body: expected value"
        );
    }

    #[test]
    fn gemini_canvas_program_video_no_key_request_exhausted_error_matches_contract() {
        let error =
            gemini_canvas_program_video_no_key_request_exhausted_error("gemini_canvas_compatible");
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_video_no_key_request_exhausted")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas preview-frame no-key video invoke exhausted all candidate request URLs."
        );
    }

    #[test]
    fn gemini_canvas_program_video_no_key_poll_empty_body_error_matches_contract() {
        let error =
            gemini_canvas_program_video_no_key_poll_empty_body_error("gemini_canvas_compatible");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_video_no_key_poll_empty_body")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas preview-frame no-key video poll returned an empty body."
        );
    }

    #[test]
    fn gemini_canvas_program_video_no_key_poll_invalid_json_error_matches_contract() {
        let error = gemini_canvas_program_video_no_key_poll_invalid_json_error(
            "gemini_canvas_compatible",
            "expected value",
        );
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_video_no_key_invalid_json")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas preview-frame no-key video poll returned non-JSON body: expected value"
        );
    }

    #[test]
    fn gemini_canvas_video_operation_timeout_error_matches_contract() {
        let error = gemini_canvas_video_operation_timeout_error("gemini_canvas_compatible");
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_video_operation_timeout")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas video generation timed out before the operation completed."
        );
    }

    #[test]
    fn gemini_canvas_video_missing_operation_error_matches_contract() {
        let error = gemini_canvas_video_missing_operation_error("gemini_canvas_compatible");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_video_missing_operation")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas video generation did not return an operation name."
        );
    }

    #[test]
    fn gemini_canvas_video_music_modality_mismatch_error_matches_contract() {
        let error = gemini_canvas_video_music_modality_mismatch_error("gemini_canvas_compatible");
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_video_music_modality_mismatch")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas video generation resolved a music-branded media body instead of a real video result."
        );
    }

    #[test]
    fn gemini_canvas_video_unsupported_count_error_matches_contract() {
        let error = gemini_canvas_video_unsupported_count_error("gemini_canvas_compatible");
        assert_eq!(error.http_status, Some(400));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("unsupported_gemini_canvas_video_count")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas video generation currently supports only n=1 requests."
        );
    }

    #[test]
    fn gemini_canvas_modular_video_unsupported_count_error_matches_contract() {
        let error = gemini_canvas_modular_video_unsupported_count_error("gemini_canvas_compatible");
        assert_eq!(error.http_status, Some(400));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("unsupported_gemini_canvas_modular_video_count")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas modular browser relay video generation currently supports only n=1 requests."
        );
    }

    #[test]
    fn gemini_canvas_video_missing_asset_error_matches_contract() {
        let error = gemini_canvas_video_missing_asset_error("gemini_canvas_compatible");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(error.code.as_deref(), Some("gemini_canvas_no_video_asset"));
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas video generation completed without a downloadable video asset."
        );
    }

    #[test]
    fn gemini_canvas_video_followup_missing_locator_error_matches_contract() {
        let error = gemini_canvas_video_followup_missing_locator_error(
            "gemini_canvas_compatible",
            "/app/path",
            "https://example.test/bootstrap",
        );
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_stream_generate_missing_conversation_id")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas video follow-up could not recover a usable conversation locator from the StreamGenerate response or recent conversation list. app_path=/app/path; bootstrap_page=https://example.test/bootstrap"
        );
    }

    #[test]
    fn gemini_canvas_media_followup_preflight_strategy_labels_match_contract() {
        assert_eq!(
            GeminiCanvasMediaFollowupPreflightStrategy::ParityThenLegacy.as_str(),
            "parity_then_legacy"
        );
        assert_eq!(
            GeminiCanvasMediaFollowupPreflightStrategy::LegacyOnly.as_str(),
            "legacy_only"
        );
    }

    #[test]
    fn gemini_canvas_followup_target_bootstrap_and_recovery_helpers_preserve_contract() {
        let mut target = GeminiCanvasFollowupTarget::from_bootstrap(
            false,
            Some("/app/bootstrap-handle"),
            Some(gemini_canvas::GeminiCanvasStreamGenerateLocator {
                response_id: "r_bootstrap".to_string(),
                conversation_id: "c_bootstrap".to_string(),
                app_path: "/app/locator-handle".to_string(),
            }),
            GeminiCanvasPageTargetMode::Resolved,
        );
        assert_eq!(target.source_path, "/app/bootstrap-handle");
        assert_eq!(target.mode, GeminiCanvasPageTargetMode::Resolved);
        assert_eq!(target.response_id(), "r_bootstrap");
        assert_eq!(target.conversation_id(), "c_bootstrap");

        target.adopt_video_recovered_locator(gemini_canvas::GeminiCanvasStreamGenerateLocator {
            response_id: "r_recovered".to_string(),
            conversation_id: "c_recovered".to_string(),
            app_path: "/app/recovered-handle".to_string(),
        });
        assert_eq!(target.source_path, "/app/recovered-handle");
        assert_eq!(
            target.mode,
            GeminiCanvasPageTargetMode::ConversationListRecovered
        );
        assert_eq!(target.response_id(), "r_recovered");
        assert_eq!(target.conversation_id(), "c_recovered");
    }

    #[test]
    fn conversation_page_missing_asset_error_matches_operation_contracts() {
        let image = gemini_canvas_conversation_page_missing_asset_error(
            gemini_canvas::GeminiCanvasMediaOperation::Image,
            GeminiCanvasPageTargetMode::Resolved,
            "/app/image",
            5,
            30,
            "poll=timeout",
            "<page>",
        );
        assert_eq!(image.http_status, Some(500));
        assert_eq!(
            image.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            image.code.as_deref(),
            Some("gemini_canvas_page_missing_image_asset")
        );
        assert_eq!(
            image.message.as_str(),
            "Gemini Canvas conversation page poll did not expose a usable media asset. locator_mode=resolved; app_path=/app/image; attempts=5; poll_budget_secs=30; failures=poll=timeout; last_page_preview=<page>"
        );

        let music = gemini_canvas_conversation_page_missing_asset_error(
            gemini_canvas::GeminiCanvasMediaOperation::Music,
            GeminiCanvasPageTargetMode::RootAppFallback,
            "/app/music",
            2,
            45,
            "none",
            "<none>",
        );
        assert_eq!(
            music.code.as_deref(),
            Some("gemini_canvas_page_missing_music_asset")
        );

        let video = gemini_canvas_conversation_page_missing_asset_error(
            gemini_canvas::GeminiCanvasMediaOperation::Video,
            GeminiCanvasPageTargetMode::ForcedRootApp,
            "/app/video",
            1,
            60,
            "upstream=empty",
            "<tail>",
        );
        assert_eq!(
            video.code.as_deref(),
            Some("gemini_canvas_page_missing_video_asset")
        );
    }

    #[test]
    fn conversation_page_poll_stage_entry_preserves_contract() {
        let entry =
            gemini_canvas_conversation_page_poll_stage_entry(3, "page_refresh", "preview-body");

        assert_eq!(entry, "attempt=3 page_refresh=preview-body");
    }

    #[test]
    fn conversation_page_poll_refresh_body_entry_preserves_preview_contract() {
        let body = "refresh body with locator ".repeat(12);
        let entry = gemini_canvas_conversation_page_poll_refresh_body_entry(5, &body);

        assert_eq!(
            entry,
            format!(
                "attempt=5 page_refresh={}",
                compact_response_preview(&body, 220)
            )
        );
    }

    #[test]
    fn conversation_page_poll_refresh_error_entry_preserves_error_summary_contract() {
        let error = GatewayError::service_unavailable("refresh failed")
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_refresh_failed");
        let entry = gemini_canvas_conversation_page_poll_refresh_error_entry(6, &error);

        assert_eq!(
            entry,
            "attempt=6 page_refresh=status=503, code=gemini_canvas_refresh_failed, message=refresh failed"
        );
    }

    #[test]
    fn conversation_page_poll_url_entry_preserves_contract() {
        let entry = gemini_canvas_conversation_page_poll_url_entry(
            4,
            "https://gemini.google.com/app/abc",
            "extract",
            "missing image asset",
        );

        assert_eq!(
            entry,
            "attempt=4 url=https://gemini.google.com/app/abc extract=missing image asset"
        );
    }

    #[test]
    fn conversation_page_poll_fetch_error_entry_preserves_error_summary_contract() {
        let error = GatewayError::service_unavailable("page fetch failed")
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_page_fetch_failed");
        let entry = gemini_canvas_conversation_page_poll_fetch_error_entry(
            8,
            "https://gemini.google.com/app/abc",
            &error,
        );

        assert_eq!(
            entry,
            "attempt=8 url=https://gemini.google.com/app/abc fetch=status=503, code=gemini_canvas_page_fetch_failed, message=page fetch failed"
        );
    }

    #[test]
    fn conversation_page_poll_body_extract_preserves_success_body_contract() {
        let page_body = r#"
            <img src="https:\/\/lh3.googleusercontent.com\/gg-dl\/ABCDEF12345\/image.png">
        "#
        .to_string();

        let (assets, returned_body) =
            try_extract_gemini_canvas_conversation_page_poll_assets_from_body(
                5,
                "https://gemini.google.com/app/abc",
                gemini_canvas::GeminiCanvasMediaOperation::Image,
                page_body.clone(),
            )
            .expect("image asset should be recovered from conversation page body");

        assert_eq!(returned_body, page_body);
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].kind, "image");
        assert_eq!(
            assets[0].url,
            "https://lh3.googleusercontent.com/gg-dl/ABCDEF12345/image.png"
        );
    }

    #[test]
    fn conversation_page_poll_body_extract_failure_records_preview_and_entry_contract() {
        let page_body = "pending conversation page without usable asset ".repeat(8);
        let failure = try_extract_gemini_canvas_conversation_page_poll_assets_from_body(
            7,
            "https://gemini.google.com/app/abc",
            gemini_canvas::GeminiCanvasMediaOperation::Image,
            page_body.clone(),
        )
        .expect_err("missing page asset should return extraction failure");

        assert_eq!(
            failure.page_preview,
            compact_response_preview(&page_body, 220)
        );
        assert_eq!(
            failure.failure_entry,
            "attempt=7 url=https://gemini.google.com/app/abc extract=status=500, code=gemini_canvas_page_missing_image_asset, message=Gemini Canvas page blob did not include a usable media asset.; download_candidate_count=0; candidate_filenames=<none>"
        );
    }

    #[test]
    fn conversation_page_poll_timing_preserves_image_contracts() {
        let concrete_page = plan_gemini_canvas_conversation_page_poll_timing(
            gemini_canvas::GeminiCanvasMediaOperation::Image,
            true,
            false,
            Duration::from_secs(300),
        );
        assert_eq!(concrete_page.poll_budget, Duration::from_secs(45));
        assert_eq!(concrete_page.sleep_between_attempts, Duration::from_secs(3));
        assert_eq!(concrete_page.max_fetch_timeout, Duration::from_secs(18));

        let forced_root = plan_gemini_canvas_conversation_page_poll_timing(
            gemini_canvas::GeminiCanvasMediaOperation::Image,
            false,
            true,
            Duration::from_secs(300),
        );
        assert_eq!(forced_root.poll_budget, Duration::from_secs(30));
        assert_eq!(forced_root.sleep_between_attempts, Duration::from_secs(3));
        assert_eq!(forced_root.max_fetch_timeout, Duration::from_secs(10));

        let default_image = plan_gemini_canvas_conversation_page_poll_timing(
            gemini_canvas::GeminiCanvasMediaOperation::Image,
            false,
            false,
            Duration::from_secs(300),
        );
        assert_eq!(default_image.poll_budget, Duration::from_secs(12));
        assert_eq!(default_image.sleep_between_attempts, Duration::from_secs(2));
        assert_eq!(default_image.max_fetch_timeout, Duration::from_secs(12));

        let minimum_budget = plan_gemini_canvas_conversation_page_poll_timing(
            gemini_canvas::GeminiCanvasMediaOperation::Image,
            true,
            false,
            Duration::from_secs(10),
        );
        assert_eq!(minimum_budget.poll_budget, Duration::from_secs(24));
    }

    #[test]
    fn conversation_page_poll_timing_preserves_media_contracts() {
        let music = plan_gemini_canvas_conversation_page_poll_timing(
            gemini_canvas::GeminiCanvasMediaOperation::Music,
            false,
            false,
            Duration::from_secs(300),
        );
        assert_eq!(music.poll_budget, Duration::from_secs(90));
        assert_eq!(music.sleep_between_attempts, Duration::from_secs(4));
        assert_eq!(music.max_fetch_timeout, Duration::from_secs(20));

        let video = plan_gemini_canvas_conversation_page_poll_timing(
            gemini_canvas::GeminiCanvasMediaOperation::Video,
            false,
            false,
            Duration::from_secs(300),
        );
        assert_eq!(video.poll_budget, Duration::from_secs(120));
        assert_eq!(video.sleep_between_attempts, Duration::from_secs(4));
        assert_eq!(video.max_fetch_timeout, Duration::from_secs(20));

        let minimum_media = plan_gemini_canvas_conversation_page_poll_timing(
            gemini_canvas::GeminiCanvasMediaOperation::Music,
            true,
            true,
            Duration::from_secs(10),
        );
        assert_eq!(minimum_media.poll_budget, Duration::from_secs(45));
    }

    #[test]
    fn conversation_page_poll_remaining_budget_enforces_one_second_guard() {
        assert_eq!(
            resolve_gemini_canvas_conversation_page_poll_remaining(
                Duration::from_secs(10),
                Duration::from_secs(8),
            ),
            Some(Duration::from_secs(2))
        );
        assert_eq!(
            resolve_gemini_canvas_conversation_page_poll_remaining(
                Duration::from_secs(10),
                Duration::from_secs(9),
            ),
            None
        );
        assert_eq!(
            resolve_gemini_canvas_conversation_page_poll_remaining(
                Duration::from_secs(10),
                Duration::from_secs(11),
            ),
            None
        );
    }

    #[test]
    fn conversation_page_poll_sleep_decision_preserves_retry_window_guard() {
        assert!(
            should_sleep_after_gemini_canvas_conversation_page_poll_attempt(
                Duration::from_secs(10),
                Duration::from_secs(4),
                Duration::from_secs(4),
            )
        );
        assert!(
            !should_sleep_after_gemini_canvas_conversation_page_poll_attempt(
                Duration::from_secs(10),
                Duration::from_secs(5),
                Duration::from_secs(4),
            )
        );
        assert!(
            !should_sleep_after_gemini_canvas_conversation_page_poll_attempt(
                Duration::from_secs(10),
                Duration::from_secs(11),
                Duration::from_secs(4),
            )
        );
    }

    #[test]
    fn conversation_page_poll_refresh_target_preserves_image_contract() {
        assert_eq!(
            resolve_gemini_canvas_conversation_page_poll_refresh_target(
                gemini_canvas::GeminiCanvasMediaOperation::Image,
                true,
                false,
                Some("https://gemini.google.com/app/conversation"),
                "https://gemini.google.com/app"
            ),
            Some("https://gemini.google.com/app/conversation")
        );
        assert_eq!(
            resolve_gemini_canvas_conversation_page_poll_refresh_target(
                gemini_canvas::GeminiCanvasMediaOperation::Image,
                true,
                false,
                None,
                "https://gemini.google.com/app"
            ),
            Some("https://gemini.google.com/app")
        );
        assert_eq!(
            resolve_gemini_canvas_conversation_page_poll_refresh_target(
                gemini_canvas::GeminiCanvasMediaOperation::Image,
                false,
                true,
                Some("https://gemini.google.com/app/conversation"),
                "https://gemini.google.com/app"
            ),
            Some("https://gemini.google.com/app/conversation")
        );
    }

    #[test]
    fn conversation_page_poll_refresh_target_skips_non_refresh_lanes() {
        assert_eq!(
            resolve_gemini_canvas_conversation_page_poll_refresh_target(
                gemini_canvas::GeminiCanvasMediaOperation::Image,
                false,
                false,
                Some("https://gemini.google.com/app/conversation"),
                "https://gemini.google.com/app"
            ),
            None
        );
        assert_eq!(
            resolve_gemini_canvas_conversation_page_poll_refresh_target(
                gemini_canvas::GeminiCanvasMediaOperation::Music,
                true,
                true,
                Some("https://gemini.google.com/app/conversation"),
                "https://gemini.google.com/app"
            ),
            None
        );
        assert_eq!(
            resolve_gemini_canvas_conversation_page_poll_refresh_target(
                gemini_canvas::GeminiCanvasMediaOperation::Video,
                true,
                true,
                Some("https://gemini.google.com/app/conversation"),
                "https://gemini.google.com/app"
            ),
            None
        );
    }

    #[test]
    fn program_owned_image_b64_policy_disables_inline_json_prefill() {
        let mut payload = make_payload(
            "gemini_canvas_program_web_reverse_compatible",
            "https://gemini.google.com",
        );
        payload.extra_body = Some(HashMap::from([(
            "imageJsonFallbackEnabled".to_string(),
            json!("true"),
        )]));
        let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
        req.raw_body = json!({
            "response_format": "b64_json"
        });

        let policy = GeminiCanvasDirectHttpImageJsonPolicy::from_request(
            &payload,
            &req,
            gemini_canvas::GeminiCanvasMediaOperation::Image,
        )
        .expect("policy");

        assert_eq!(
            policy.initial_action(),
            GeminiCanvasDirectHttpImageJsonAction::Skip
        );
        assert_eq!(
            policy.on_stream_failure(),
            GeminiCanvasDirectHttpImageJsonAction::ReturnOriginal
        );
        assert_eq!(
            policy.on_materialize_failure(),
            GeminiCanvasDirectHttpImageJsonAction::ReturnOriginal
        );
    }

    #[test]
    fn gemini_canvas_direct_http_image_json_policy_inline_prefill_and_recovery_contract() {
        let mut payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        payload.extra_body = Some(HashMap::from([(
            "imageJsonFallbackEnabled".to_string(),
            json!("true"),
        )]));
        let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
        req.raw_body = json!({
            "response_format": "b64_json"
        });

        let mut policy = GeminiCanvasDirectHttpImageJsonPolicy::from_request(
            &payload,
            &req,
            gemini_canvas::GeminiCanvasMediaOperation::Image,
        )
        .expect("policy");

        assert_policy_action(
            policy.initial_action(),
            GeminiCanvasDirectHttpImageJsonAction::TryJson,
        );

        policy.note_prefill_failure("prefill failed".to_string());
        assert_eq!(
            policy.on_stream_failure(),
            GeminiCanvasDirectHttpImageJsonAction::ReturnOriginalWithSummary {
                context_key: "image_json_prefill_failure",
                summary: "prefill failed".to_string(),
            }
        );
    }

    #[test]
    fn gemini_canvas_direct_http_image_json_policy_stream_fallback_only_for_non_inline_mode() {
        let mut payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        payload.extra_body = Some(HashMap::from([(
            "imageJsonFallbackEnabled".to_string(),
            json!("true"),
        )]));
        let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
        req.raw_body = json!({
            "response_format": "url"
        });

        let policy = GeminiCanvasDirectHttpImageJsonPolicy::from_request(
            &payload,
            &req,
            gemini_canvas::GeminiCanvasMediaOperation::Image,
        )
        .expect("policy");

        assert_policy_action(
            policy.initial_action(),
            GeminiCanvasDirectHttpImageJsonAction::Skip,
        );
        assert_eq!(
            policy.on_stream_failure(),
            GeminiCanvasDirectHttpImageJsonAction::TryJsonWithErrorContext {
                context_key: "stream_generate_failure",
            }
        );
    }

    #[test]
    fn gemini_canvas_direct_http_image_json_policy_disables_legacy_json_for_edits() {
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesEdits);
        req.raw_body = json!({
            "response_format": "b64_json"
        });

        let policy = GeminiCanvasDirectHttpImageJsonPolicy::from_request(
            &payload,
            &req,
            gemini_canvas::GeminiCanvasMediaOperation::Image,
        )
        .expect("policy");

        assert_policy_action(
            policy.initial_action(),
            GeminiCanvasDirectHttpImageJsonAction::Skip,
        );
        assert_policy_action(
            policy.on_stream_failure(),
            GeminiCanvasDirectHttpImageJsonAction::ReturnOriginal,
        );
    }

    #[test]
    fn gemini_canvas_direct_http_image_json_policy_materialize_recovery_matches_inline_mode() {
        let mut payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        payload.extra_body = Some(HashMap::from([(
            "imageJsonFallbackEnabled".to_string(),
            json!("true"),
        )]));

        let mut inline_req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
        inline_req.raw_body = json!({
            "response_format": "b64_json"
        });
        let inline_policy = GeminiCanvasDirectHttpImageJsonPolicy::from_request(
            &payload,
            &inline_req,
            gemini_canvas::GeminiCanvasMediaOperation::Image,
        )
        .expect("inline policy");
        assert_eq!(
            inline_policy.on_materialize_failure(),
            GeminiCanvasDirectHttpImageJsonAction::TryJsonWithErrorContext {
                context_key: "image_json_recovery_failure",
            }
        );

        let mut url_req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
        url_req.raw_body = json!({
            "response_format": "url"
        });
        let url_policy = GeminiCanvasDirectHttpImageJsonPolicy::from_request(
            &payload,
            &url_req,
            gemini_canvas::GeminiCanvasMediaOperation::Image,
        )
        .expect("url policy");
        assert_policy_action(
            url_policy.on_materialize_failure(),
            GeminiCanvasDirectHttpImageJsonAction::ReturnOriginal,
        );
    }

    #[test]
    fn prepare_gemini_canvas_direct_http_image_context_builds_edit_followup_state() {
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesEdits);
        req.raw_body = json!({
            "image": "data:image/png;base64,aGVsbG8="
        });

        let context =
            prepare_gemini_canvas_direct_http_image_context(&payload, &req, "prompt text")
                .expect("image edit context");

        assert_eq!(
            context.mode_index,
            gemini_canvas::stream_generate_mode_index(
                gemini_canvas::GeminiCanvasMediaOperation::Image
            )
        );
        assert!(context.image_edit_uploads.is_some());
        assert!(context.image_edit_followup_context.is_some());
        assert_eq!(
            context
                .image_edit_followup_context
                .as_ref()
                .map(|value| value.prompt.as_str()),
            Some("prompt text")
        );
        assert!(context.initial_stream_allows_replay_template);
    }

    #[test]
    fn resolve_gemini_canvas_followup_locator_uses_payload_handle_for_image() {
        let payload = make_gemini_canvas_program_payload_with_handle(Some("image"));

        let locator = resolve_gemini_canvas_followup_locator(
            &payload,
            gemini_canvas::GeminiCanvasMediaOperation::Image,
            "body without locator",
            "gemini_canvas_compatible",
            "test",
        )
        .expect("locator resolution")
        .expect("payload locator");

        assert_eq!(locator.app_path, "/app/4abc4e7577b6149f");
        assert_eq!(locator.conversation_id, "c_4abc4e7577b6149f");
        assert_eq!(locator.response_id, "r_payload");
    }

    #[test]
    fn resolve_gemini_canvas_followup_locator_keeps_music_on_stream_locator_contract() {
        let payload = make_gemini_canvas_program_payload_with_handle(Some("image"));

        let locator = resolve_gemini_canvas_followup_locator(
            &payload,
            gemini_canvas::GeminiCanvasMediaOperation::Music,
            "body without locator",
            "gemini_canvas_compatible",
            "test",
        )
        .expect("music should gracefully fall back when payload handle modality mismatches");

        assert!(locator.is_none());
    }

    #[test]
    fn prepare_gemini_canvas_page_seed_prefers_override_without_locator() {
        let payload = make_payload(
            "gemini_canvas_program_web_reverse_compatible",
            "https://gemini.google.com",
        );

        let runtime = gemini_canvas::GeminiCanvasRuntime {
            runtime_state_object_key: "credential-runtime/gemini-canvas/program/storage-state.json"
                .to_string(),
            share_id: "canvas-share-789".to_string(),
            api_base_url: "https://gemini.google.com".to_string(),
        };

        let seed = prepare_gemini_canvas_page_seed(
            &payload,
            &runtime,
            gemini_canvas::GeminiCanvasMediaOperation::Image,
            "body without locator",
            false,
            Some(" https://gemini.google.com/app/4abc4e7577b6149f "),
            true,
            "gemini_canvas_compatible",
            "test",
        )
        .expect("page seed");

        assert!(seed.locator.is_none());
        assert!(seed.prefer_root_app_path);
        assert_eq!(
            seed.conversation_page_url.as_deref(),
            Some("https://gemini.google.com/app/4abc4e7577b6149f")
        );
        assert_eq!(seed.app_bootstrap_url, "https://gemini.google.com/app");
        assert_eq!(
            seed.share_bootstrap_url,
            "https://gemini.google.com/share/canvas-share-789"
        );
        assert!(seed.has_page_url_override);
    }

    #[test]
    fn build_gemini_canvas_followup_bootstrap_candidates_prefers_app_then_concrete_then_share() {
        let candidates = build_gemini_canvas_followup_bootstrap_candidates(
            false,
            Some("https://gemini.google.com/app/4abc4e7577b6149f"),
            "https://gemini.google.com/app",
            "https://gemini.google.com/share/canvas-share-789",
        );

        assert_eq!(
            candidates,
            vec![
                "https://gemini.google.com/app".to_string(),
                "https://gemini.google.com/app/4abc4e7577b6149f".to_string(),
                "https://gemini.google.com/share/canvas-share-789".to_string()
            ]
        );
    }

    #[test]
    fn build_gemini_canvas_page_poll_urls_skips_concrete_page_for_forced_root_without_override() {
        let candidates = build_gemini_canvas_page_poll_urls(
            true,
            false,
            Some("https://gemini.google.com/app/4abc4e7577b6149f"),
            "https://gemini.google.com/app",
            "https://gemini.google.com/share/canvas-share-789",
        );

        assert_eq!(
            candidates,
            vec![
                "https://gemini.google.com/app".to_string(),
                "https://gemini.google.com/share/canvas-share-789".to_string()
            ]
        );
    }

    #[test]
    fn build_gemini_canvas_media_followup_preflight_plan_preserves_mode_and_source_path() {
        let bootstrap = make_gemini_bootstrap();
        let plan = build_gemini_canvas_media_followup_preflight_plan(
            &bootstrap,
            "/app/4abc4e7577b6149f",
            gemini_canvas::GeminiCanvasMediaOperation::Music,
        )
        .expect("preflight plan");

        assert_eq!(
            plan.mode_index,
            gemini_canvas::stream_generate_mode_index(
                gemini_canvas::GeminiCanvasMediaOperation::Music
            )
        );
        assert!(plan
            .followup_model_header
            .contains(gemini_canvas::GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID));
        assert!(plan
            .activity_request
            .query
            .iter()
            .any(|(key, value)| key == "source-path" && value == "/app/4abc4e7577b6149f"));
        assert!(plan
            .followup_request
            .query
            .iter()
            .any(|(key, value)| key == "rpcids"
                && value == gemini_canvas::GEMINI_CANVAS_TEXT_BOOTSTRAP_RPCID));
    }

    #[test]
    fn gemini_canvas_followup_target_adopts_bootstrap_path_and_recovered_locator() {
        let mut target = GeminiCanvasFollowupTarget::from_bootstrap(
            false,
            Some("/app/from-bootstrap"),
            None,
            GeminiCanvasPageTargetMode::RootAppFallback,
        );
        assert_eq!(target.source_path, "/app/from-bootstrap");
        assert_eq!(target.mode, GeminiCanvasPageTargetMode::RootAppFallback);
        assert_eq!(target.response_id(), "<none>");

        target.adopt_video_recovered_locator(gemini_canvas::GeminiCanvasStreamGenerateLocator {
            response_id: "r_recovered".to_string(),
            conversation_id: "c_recovered".to_string(),
            app_path: "/app/recovered".to_string(),
        });
        assert_eq!(target.source_path, "/app/recovered");
        assert_eq!(
            target.mode,
            GeminiCanvasPageTargetMode::ConversationListRecovered
        );
        assert_eq!(target.response_id(), "r_recovered");
        assert_eq!(target.conversation_id(), "c_recovered");
    }

    #[test]
    fn finalize_gemini_canvas_media_followup_attempt_state_prefers_error_over_body() {
        let target = GeminiCanvasFollowupTarget::from_bootstrap(
            false,
            Some("/app/from-bootstrap"),
            Some(gemini_canvas::GeminiCanvasStreamGenerateLocator {
                response_id: "r_followup".to_string(),
                conversation_id: "c_followup".to_string(),
                app_path: "/app/from-bootstrap".to_string(),
            }),
            GeminiCanvasPageTargetMode::Resolved,
        );
        let state = GeminiCanvasMediaFollowupAttemptState {
            last_body: Some("{\"status\":\"pending\"}".to_string()),
            last_error: Some(
                GatewayError::service_unavailable("usable asset missing")
                    .with_provider("gemini_canvas_compatible")
                    .with_code("gemini_canvas_media_followup_missing_asset"),
            ),
        };

        let error = finalize_gemini_canvas_media_followup_attempt_state(
            "gemini_canvas_compatible",
            &target,
            "https://gemini.google.com/app",
            state,
        )
        .expect_err("follow-up finalize should preserve richer error");

        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_media_followup_missing_asset")
        );
        assert!(error.message.contains("usable asset missing"));
    }

    #[test]
    fn build_gemini_canvas_media_followup_missing_asset_error_keeps_body_preview() {
        let target = GeminiCanvasFollowupTarget::from_bootstrap(
            false,
            Some("/app/from-bootstrap"),
            Some(gemini_canvas::GeminiCanvasStreamGenerateLocator {
                response_id: "r_followup".to_string(),
                conversation_id: "c_followup".to_string(),
                app_path: "/app/from-bootstrap".to_string(),
            }),
            GeminiCanvasPageTargetMode::Resolved,
        );

        let error = build_gemini_canvas_media_followup_missing_asset_error(
            "gemini_canvas_compatible",
            gemini_canvas::GeminiCanvasMediaOperation::Video,
            &target,
            "https://gemini.google.com/app",
            2,
            "{\"status\":\"pending\",\"detail\":\"still rendering\"}",
        );

        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_media_followup_missing_asset")
        );
        assert!(error.message.contains("attempt=2"));
        assert!(error.message.contains("still rendering"));
    }

    #[test]
    fn plan_gemini_canvas_video_completion_attempt_sleep_matches_pending_contract() {
        assert_eq!(
            plan_gemini_canvas_video_completion_attempt_sleep(true, false, 1),
            Some(Duration::from_secs(8))
        );
        assert_eq!(
            plan_gemini_canvas_video_completion_attempt_sleep(false, true, 1),
            Some(Duration::from_secs(8))
        );
        assert_eq!(
            plan_gemini_canvas_video_completion_attempt_sleep(false, false, 1),
            Some(Duration::from_secs(4))
        );
        assert_eq!(
            plan_gemini_canvas_video_completion_attempt_sleep(false, false, 3),
            None
        );
    }

    #[test]
    fn build_gemini_canvas_video_completion_missing_asset_error_keeps_previews() {
        let error = build_gemini_canvas_video_completion_missing_asset_error(
            "/app/video",
            "c_video",
            "r_video",
            4,
            Some("job_123"),
            Some("{\"status\":\"pending\"}"),
            Some("{\"meta\":\"still rendering\"}"),
        );

        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_video_completion_followup_missing_asset")
        );
        assert!(error.message.contains("job_id=job_123"));
        assert!(error.message.contains("pending"));
        assert!(error.message.contains("still rendering"));
    }

    #[test]
    fn build_gemini_canvas_direct_http_video_missing_asset_error_matches_contract() {
        let error =
            build_gemini_canvas_direct_http_video_missing_asset_error("gemini_canvas_compatible");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(error.code.as_deref(), Some("gemini_canvas_no_video_asset"));
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas direct HTTP video generation completed without returning a video asset."
        );
    }

    #[test]
    fn extract_gemini_canvas_video_operation_download_uri_reports_standard_contracts() {
        let missing_payload = json!({
            "response": {}
        });
        let missing_payload_error = extract_gemini_canvas_video_operation_download_uri(
            "gemini_canvas_compatible",
            &missing_payload,
        )
        .expect_err("missing payload should fail");
        assert_eq!(
            missing_payload_error.code.as_deref(),
            Some("gemini_canvas_no_video_asset")
        );
        assert_eq!(
            missing_payload_error.message.as_str(),
            "Gemini Canvas video operation completed without a downloadable video payload."
        );

        let missing_uri = json!({
            "response": {
                "generatedVideos": [
                    { "video": {} }
                ]
            }
        });
        let missing_uri_error = extract_gemini_canvas_video_operation_download_uri(
            "gemini_canvas_compatible",
            &missing_uri,
        )
        .expect_err("missing uri should fail");
        assert_eq!(
            missing_uri_error.code.as_deref(),
            Some("gemini_canvas_no_video_asset")
        );
        assert_eq!(
            missing_uri_error.message.as_str(),
            "Gemini Canvas video operation completed without video.uri."
        );

        let uri_payload = json!({
            "response": {
                "generatedVideos": [
                    { "video": { "uri": " https://example.com/video.mp4 " } }
                ]
            }
        });
        let uri = extract_gemini_canvas_video_operation_download_uri(
            "gemini_canvas_compatible",
            &uri_payload,
        )
        .expect("uri should exist");
        assert_eq!(uri, "https://example.com/video.mp4");
    }

    #[test]
    fn build_gemini_canvas_video_completion_requests_recovers_job_and_rpcs() {
        let bootstrap = make_gemini_bootstrap();
        let primary_body = r#"
            )]}'
            [["wrb.fr",null,"[null,[\"c_98256aabc450ff93\",\"r_931fae4dfff02e00\"],null,null,[[\"rc_pending\",[\"正在生成视频，这可能需要几分钟时间，请稍后回来查看完成状态。\nhttp://googleusercontent.com/video_gen_chip/0\n\"],null,null,null,null,null,null,[1],\"zh\",null,null,[{\"65\":[[\"http://googleusercontent.com/video_gen_chip/0\"],\"e3136f2f-29f4-4f33-9223-271d4697c60a\"]}]]]]"]]
        "#;

        let requests = build_gemini_canvas_video_completion_requests(
            &bootstrap,
            "/app/video",
            "c_98256aabc450ff93",
            "r_931fae4dfff02e00",
            primary_body,
        )
        .expect("video completion requests");

        assert_eq!(
            requests.job_id.as_deref(),
            Some("e3136f2f-29f4-4f33-9223-271d4697c60a")
        );
        assert!(requests
            .job_poll_request
            .as_ref()
            .expect("job poll request")
            .as_ref()
            .expect("job poll request result")
            .query
            .iter()
            .any(|(key, value)| key == "rpcids" && value == "kwDCne"));
        assert!(requests
            .completion_request
            .query
            .iter()
            .any(|(key, value)| key == "rpcids" && value == "hNvQHb"));
        assert!(requests
            .metadata_request
            .query
            .iter()
            .any(|(key, value)| key == "rpcids" && value == "MUAZcd"));
    }

    #[test]
    fn gemini_canvas_video_completion_poll_state_tracks_attempt_progress() {
        let mut poll_state = GeminiCanvasVideoCompletionPollState::default();
        assert_eq!(poll_state.next_attempt(), 1);

        let next_sleep_for =
            poll_state.record_attempt_progress(&GeminiCanvasVideoCompletionAttemptState {
                completion: GeminiCanvasVideoStageProgress {
                    body: "{\"status\":\"pending\"}".to_string(),
                    pending: true,
                },
                metadata: Some(GeminiCanvasVideoStageProgress {
                    body: "{\"meta\":\"settling\"}".to_string(),
                    pending: false,
                }),
            });

        assert_eq!(next_sleep_for, Some(Duration::from_secs(8)));
        let error = poll_state.build_missing_asset_error(
            "/app/video",
            "c_video",
            "r_video",
            Some("job_456"),
        );
        assert!(error.message.contains("job_id=job_456"));
        assert!(error.message.contains("pending"));
        assert!(error.message.contains("settling"));
    }

    #[test]
    fn classify_gemini_canvas_video_stage_body_marks_pending_progress() {
        let body = r#"
            )]}'
            [["wrb.fr",null,"[null,[\"c_98256aabc450ff93\",\"r_931fae4dfff02e00\"],null,null,[[\"rc_pending\",[\"正在生成视频，这可能需要几分钟时间，请稍后回来查看完成状态。\nhttp://googleusercontent.com/video_gen_chip/0\n\"],null,null,null,null,null,null,[1],\"zh\",null,null,[{\"65\":[[\"http://googleusercontent.com/video_gen_chip/0\"],\"e3136f2f-29f4-4f33-9223-271d4697c60a\"]}]]]]"]]
        "#
        .to_string();

        let progress = classify_gemini_canvas_video_stage_body(body)
            .expect_err("pending body should stay in progress contract");
        assert!(progress.pending);
        assert!(progress.body.contains("video_gen_chip"));
    }

    #[test]
    fn classify_gemini_canvas_video_stage_body_accepts_page_blob_asset() {
        let body = r#"
            <video
                src="https:\/\/contribution.usercontent.google.com\/download?c\u003dxyz789\u0026filename\u003dvideo.mp4\u0026opi\u003d103135050">
            </video>
        "#
        .to_string();

        let result = classify_gemini_canvas_video_stage_body(body.clone())
            .expect("video asset body should complete");
        assert_eq!(result, body);
    }

    #[test]
    fn resolve_gemini_canvas_image_recovery_mode_matches_endpoint_kind() {
        assert_eq!(
            resolve_gemini_canvas_image_recovery_mode(EndpointKind::ImagesEdits),
            GeminiCanvasImageRecoveryMode::EditAsync
        );
        assert_eq!(
            resolve_gemini_canvas_image_recovery_mode(EndpointKind::ImagesGenerations),
            GeminiCanvasImageRecoveryMode::Generation
        );
    }

    #[test]
    fn plan_gemini_canvas_image_response_returns_url_body_when_assets_are_caller_usable() {
        let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
        req.raw_body = json!({
            "response_format": "url",
            "n": 2
        });
        let assets = vec![
            make_image_asset("https://example.com/a.png"),
            make_image_asset("https://example.com/b.png"),
        ];

        let result = plan_gemini_canvas_image_response(
            &req,
            "neon city",
            &assets,
            "gemini_canvas_compatible",
            "missing image asset",
            "gemini_canvas_no_image_asset",
        )
        .expect("planned image response");

        match result {
            GeminiCanvasImageResponsePlan::FinalResponse(body) => {
                let data = body
                    .get("data")
                    .and_then(Value::as_array)
                    .expect("url response data");
                assert_eq!(data.len(), 2);
                assert_eq!(
                    data[0].get("url").and_then(Value::as_str),
                    Some("https://example.com/a.png")
                );
            }
            GeminiCanvasImageResponsePlan::Materialize(_) => {
                panic!("expected direct url response plan");
            }
        }
    }

    #[test]
    fn plan_gemini_canvas_image_response_materializes_requested_asset_count() {
        let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
        req.raw_body = json!({
            "response_format": "b64_json",
            "n": 1
        });
        let assets = vec![
            make_image_asset("blob:https://gemini.google.com/example-1"),
            make_image_asset("https://example.com/b.png"),
        ];

        let result = plan_gemini_canvas_image_response(
            &req,
            "neon city",
            &assets,
            "gemini_canvas_compatible",
            "missing image asset",
            "gemini_canvas_no_image_asset",
        )
        .expect("planned image response");

        match result {
            GeminiCanvasImageResponsePlan::Materialize(assets) => {
                assert_eq!(assets.len(), 1);
                assert_eq!(assets[0].url, "blob:https://gemini.google.com/example-1");
            }
            GeminiCanvasImageResponsePlan::FinalResponse(_) => {
                panic!("expected materialize response plan");
            }
        }
    }

    #[test]
    fn classify_gemini_canvas_page_target_mode_covers_lane_variants() {
        assert_eq!(
            classify_gemini_canvas_page_target_mode(true, false, false, false),
            GeminiCanvasPageTargetMode::ForcedRootApp
        );
        assert_eq!(
            classify_gemini_canvas_page_target_mode(false, true, true, false),
            GeminiCanvasPageTargetMode::SignalerPageBootstrap
        );
        assert_eq!(
            classify_gemini_canvas_page_target_mode(false, false, true, true),
            GeminiCanvasPageTargetMode::ConversationListRecovered
        );
        assert_eq!(
            classify_gemini_canvas_page_target_mode(false, false, true, false),
            GeminiCanvasPageTargetMode::Resolved
        );
        assert_eq!(
            classify_gemini_canvas_page_target_mode(false, false, false, false),
            GeminiCanvasPageTargetMode::RootAppFallback
        );
    }

    #[test]
    fn classify_gemini_canvas_image_template_retry_matches_edit_and_non_edit_contracts() {
        assert_eq!(
            classify_gemini_canvas_image_template_retry(
                GeminiCanvasImageRecoveryMode::EditAsync,
                true,
                true,
            ),
            GeminiCanvasImageTemplateRetryAction::ReturnOriginalWithLog(
                "gemini canvas direct HTTP image-edit template lane reached async-followup-ready; suppressing legacy heavy retry",
            )
        );
        assert_eq!(
            classify_gemini_canvas_image_template_retry(
                GeminiCanvasImageRecoveryMode::EditAsync,
                true,
                false,
            ),
            GeminiCanvasImageTemplateRetryAction::ReturnOriginalWithLog(
                "gemini canvas direct HTTP image-edit template lane ended without a usable asset; suppressing legacy heavy retry to preserve caller-visible transport budget",
            )
        );
        assert_eq!(
            classify_gemini_canvas_image_template_retry(
                GeminiCanvasImageRecoveryMode::Generation,
                false,
                false,
            ),
            GeminiCanvasImageTemplateRetryAction::ReturnOriginal
        );
        assert_eq!(
            classify_gemini_canvas_image_template_retry(
                GeminiCanvasImageRecoveryMode::Generation,
                true,
                false,
            ),
            GeminiCanvasImageTemplateRetryAction::RetryLegacyTemplate
        );
    }

    #[test]
    fn build_gemini_canvas_image_recovery_strategy_matches_lane_contracts() {
        let edit_ready = build_gemini_canvas_image_recovery_strategy(
            EndpointKind::ImagesEdits,
            true,
            concat!(
                ")]}'\n\n",
                "126\n",
                "[[\"wrb.fr\",null,\"[null,[null,\\\"r_async123\\\"],{\\\"18\\\":\\\"r_async123\\\",\\\"21\\\":[\\\"token\\\"],\\\"44\\\":true}]\"]]\n"
            ),
        );
        assert_eq!(edit_ready.mode, GeminiCanvasImageRecoveryMode::EditAsync);
        assert_eq!(
            edit_ready.template_retry_action,
            GeminiCanvasImageTemplateRetryAction::ReturnOriginalWithLog(
                "gemini canvas direct HTTP image-edit template lane reached async-followup-ready; suppressing legacy heavy retry",
            )
        );

        let generation_retry = build_gemini_canvas_image_recovery_strategy(
            EndpointKind::ImagesGenerations,
            true,
            "plain body without async marker",
        );
        assert_eq!(
            generation_retry.mode,
            GeminiCanvasImageRecoveryMode::Generation
        );
        assert_eq!(
            generation_retry.template_retry_action,
            GeminiCanvasImageTemplateRetryAction::RetryLegacyTemplate
        );
    }

    #[test]
    fn prepare_gemini_canvas_direct_http_image_context_skips_edit_state_for_generations() {
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);

        let context =
            prepare_gemini_canvas_direct_http_image_context(&payload, &req, "prompt text")
                .expect("image generation context");

        assert_eq!(
            context.mode_index,
            gemini_canvas::stream_generate_mode_index(
                gemini_canvas::GeminiCanvasMediaOperation::Image
            )
        );
        assert!(context.image_edit_uploads.is_none());
        assert!(context.image_edit_followup_context.is_none());
        assert!(context.initial_stream_allows_replay_template);
    }

    #[test]
    fn build_gemini_canvas_video_accepted_response_from_body_prefers_stream_locator() {
        let body = concat!(
            ")]}'\n\n",
            "126\n",
            "[[\"wrb.fr\",null,\"[null,[\\\"c_stream\\\",\\\"r_stream\\\"],null,null,[[\\\"rc_done\\\",[\\\"ready\\\"],null,null,null,null,null,null,[1],\\\"zh\\\",null,null,[{\\\"65\\\":[[\\\"/app/video-stream\\\"],\\\"job-1\\\"]}]]]]\"]]\n"
        );

        let response = build_gemini_canvas_video_accepted_response_from_body(
            "gemini-2.5-pro",
            "make a clip",
            body,
            Some("c_fallback"),
            Some("r_fallback"),
            Some("/app/fallback"),
        );

        assert_eq!(response["provider"], json!("gemini_canvas"));
        assert_eq!(response["conversation_id"], json!("c_stream"));
        assert_eq!(response["response_id"], json!("r_stream"));
        assert_eq!(response["app_path"], json!("/app/video-stream"));
        assert_eq!(response["job_id"], json!("job-1"));
    }
}
