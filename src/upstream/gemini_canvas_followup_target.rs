use crate::error::GatewayError;
use crate::protocol::{gemini_canvas, gemini_web};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::gemini::canvas_program_web_reverse as gemini_canvas_program_web_reverse_modular;
use crate::upstream::gemini_canvas_runtime_error_helpers::summarize_gateway_error;
use crate::upstream::gemini_canvas_runtime_helpers::gemini_canvas_page_base_url;
use tracing::debug;

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
