use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::protocol::gemini_canvas;
use crate::routing::candidate::ProviderAccountPayload;

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
