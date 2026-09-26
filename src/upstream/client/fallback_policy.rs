use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeminiCanvasTextDirectHttpFallbackAttempt<'a> {
    pub(crate) label: &'static str,
    pub(crate) api_key_override: Option<&'a str>,
    pub(crate) referer_override: Option<&'a str>,
    pub(crate) preserve_cross_origin_referer: bool,
    pub(crate) include_signed_headers: bool,
}

pub(crate) fn build_gemini_canvas_text_direct_http_fallback_attempts<'a>(
    payload: &'a ProviderAccountPayload,
    runtime_api: Option<&'a GeminiCanvasRuntimeApiContext>,
) -> Vec<GeminiCanvasTextDirectHttpFallbackAttempt<'a>> {
    let mut attempts: Vec<GeminiCanvasTextDirectHttpFallbackAttempt<'a>> = Vec::new();

    if payload.api_key.trim().is_empty() {
        if let Some(runtime_api) = runtime_api {
            let referer_override = runtime_api.page_referer.trim();
            let referer_override = if referer_override.is_empty() {
                None
            } else {
                Some(referer_override)
            };
            for candidate in &runtime_api.api_key_candidates {
                let trimmed = candidate.trim();
                if trimmed.is_empty()
                    || attempts.iter().any(
                        |attempt: &GeminiCanvasTextDirectHttpFallbackAttempt<'a>| {
                            attempt.api_key_override == Some(trimmed)
                        },
                    )
                {
                    continue;
                }
                attempts.push(GeminiCanvasTextDirectHttpFallbackAttempt {
                    label: "runtime_api_harvested_key",
                    api_key_override: Some(trimmed),
                    referer_override,
                    preserve_cross_origin_referer: referer_override.is_some(),
                    include_signed_headers: false,
                });
            }
        }
    }

    attempts.push(GeminiCanvasTextDirectHttpFallbackAttempt {
        label: "legacy_payload_direct_http",
        api_key_override: None,
        referer_override: None,
        preserve_cross_origin_referer: false,
        include_signed_headers: true,
    });

    attempts
}

pub(crate) fn should_fallback_gemini_canvas_image_to_browser(error: &GatewayError) -> bool {
    matches!(
        error.code.as_deref(),
        Some(
            "gemini_canvas_auth_required"
                | "gemini_canvas_auth_redirect"
                | "gemini_canvas_pure_http_browser_challenge_required"
                | "gemini_canvas_pure_http_session_invalid"
                | "gemini_canvas_media_followup_missing_asset"
                | "gemini_canvas_media_followup_failed"
                | "gemini_canvas_media_followup_bootstrap_failed"
                | "gemini_canvas_page_missing_image_asset"
        )
    ) || matches!(error.http_status, Some(401 | 403 | 504))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeminiCanvasVideoContinuation {
    pub(crate) conversation_id: String,
    pub(crate) response_id: String,
    pub(crate) app_path: String,
    pub(crate) job_id: Option<String>,
}

pub(crate) fn apply_gemini_canvas_browser_video_continuation(
    invocation_input: &mut Value,
    continuation: &GeminiCanvasVideoContinuation,
) {
    let Some(input) = invocation_input.as_object_mut() else {
        return;
    };
    input.insert("resumeExistingMedia".to_string(), Value::Bool(true));
    input.insert(
        "conversationId".to_string(),
        Value::String(continuation.conversation_id.clone()),
    );
    input.insert(
        "responseId".to_string(),
        Value::String(continuation.response_id.clone()),
    );
    input.insert(
        "appPath".to_string(),
        Value::String(continuation.app_path.clone()),
    );
    if let Some(job_id) = continuation.job_id.as_ref() {
        input.insert("jobId".to_string(), Value::String(job_id.clone()));
    }
}

pub(crate) fn build_gemini_canvas_video_continuation_seed_body(
    continuation: &GeminiCanvasVideoContinuation,
) -> String {
    json!({
        "status": "video_generation_pending",
        "marker": "video_gen_chip",
        "conversation_id": &continuation.conversation_id,
        "response_id": &continuation.response_id,
        "app_path": &continuation.app_path,
        "job_id": continuation.job_id.as_deref(),
    })
    .to_string()
}

pub(crate) fn gemini_canvas_video_continuation_from_request(
    req: &CanonicalRelayRequest,
) -> Result<Option<GeminiCanvasVideoContinuation>, GatewayError> {
    let Some(body) = req.raw_body.as_object() else {
        return Ok(None);
    };
    let read_field = |aliases: &[&str]| {
        aliases.iter().find_map(|alias| {
            body.get(*alias)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
    };
    let conversation_id = read_field(&["conversation_id", "conversationId"]);
    let response_id = read_field(&["response_id", "responseId"]);
    let app_path = read_field(&["app_path", "appPath"]);
    let job_id = read_field(&["job_id", "jobId"]);
    if conversation_id.is_none() && response_id.is_none() && app_path.is_none() && job_id.is_none()
    {
        return Ok(None);
    }

    let invalid = |message: &'static str| {
        GatewayError::bad_request(message)
            .with_provider("gemini_canvas_compatible")
            .with_code("invalid_gemini_canvas_video_continuation")
    };
    let conversation_id = conversation_id
        .ok_or_else(|| invalid("Gemini Canvas video continuation requires conversation_id."))?;
    let response_id = response_id
        .ok_or_else(|| invalid("Gemini Canvas video continuation requires response_id."))?;
    let app_path =
        app_path.ok_or_else(|| invalid("Gemini Canvas video continuation requires app_path."))?;

    let valid_id = |value: &str, prefix: &str| {
        value
            .strip_prefix(prefix)
            .filter(|suffix| !suffix.is_empty())
            .is_some_and(|suffix| {
                suffix
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
            })
    };
    if !valid_id(&conversation_id, "c_") {
        return Err(invalid(
            "Gemini Canvas video continuation conversation_id is invalid.",
        ));
    }
    if !valid_id(&response_id, "r_") {
        return Err(invalid(
            "Gemini Canvas video continuation response_id is invalid.",
        ));
    }
    let valid_app_path = app_path
        .strip_prefix("/app/")
        .filter(|suffix| !suffix.is_empty())
        .is_some_and(|suffix| {
            suffix
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
        });
    if !valid_app_path {
        return Err(invalid(
            "Gemini Canvas video continuation app_path is invalid.",
        ));
    }

    Ok(Some(GeminiCanvasVideoContinuation {
        conversation_id,
        response_id,
        app_path,
        job_id,
    }))
}

pub(crate) fn should_preserve_gemini_canvas_video_continuation_as_pending(
    error: &GatewayError,
) -> bool {
    matches!(
        error.code.as_deref(),
        Some(
            "gemini_canvas_media_followup_missing_asset"
                | "gemini_canvas_media_followup_failed"
                | "gemini_canvas_page_missing_video_asset"
                | "gemini_canvas_video_completion_followup_missing_asset"
                | "gemini_canvas_video_operation_timeout"
        )
    ) || error.http_status == Some(504)
}

pub(crate) fn gemini_canvas_stream_collection_policy(
    mode_index: i64,
    is_image_edit_request: bool,
) -> (gemini_canvas::GeminiCanvasMediaOperation, bool) {
    let operation = match mode_index {
        gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_MUSIC_MODE_INDEX => {
            gemini_canvas::GeminiCanvasMediaOperation::Music
        }
        gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_VIDEO_MODE_INDEX => {
            gemini_canvas::GeminiCanvasMediaOperation::Video
        }
        _ => gemini_canvas::GeminiCanvasMediaOperation::Image,
    };
    let allow_early_locator =
        is_image_edit_request || operation == gemini_canvas::GeminiCanvasMediaOperation::Video;
    (operation, allow_early_locator)
}
