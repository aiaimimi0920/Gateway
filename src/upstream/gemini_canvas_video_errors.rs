use crate::error::{sanitize_provider_error_message, GatewayError};

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
    GatewayError::server_error(sanitize_provider_error_message(&format!(
        "Gemini Canvas preview-frame no-key video fetch returned non-JSON body: {error}"
    )))
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
    GatewayError::server_error(sanitize_provider_error_message(&format!(
        "Gemini Canvas preview-frame no-key video poll returned non-JSON body: {error}"
    )))
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
    GatewayError::service_unavailable(sanitize_provider_error_message(&format!(
        "Gemini Canvas video follow-up could not recover a usable conversation locator from the StreamGenerate response or recent conversation list. app_path={source_path}; bootstrap_page={bootstrap_page_url}"
    )))
    .with_provider(provider)
    .with_code("gemini_canvas_stream_generate_missing_conversation_id")
}
