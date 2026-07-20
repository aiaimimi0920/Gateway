mod browser_operation;
mod execution;
mod payload;
mod result;

pub use browser_operation::{
    build_browser_executor_invocation_input, build_browser_operation_invocation_input,
    build_browser_operation_invocation_input_from_values, build_connected_fetch_invocation_input,
};
pub(crate) use browser_operation::{
    build_connected_fetch_form_invocation_input, build_http_replay_worker_input,
    prepare_gemini_canvas_browser_executor_service_input,
};
pub(crate) use execution::decode_direct_http_inline_image_asset;
pub(crate) use execution::plan_direct_http_image_response;
pub use execution::{
    browser_pool_missing_image_asset_error, build_downloaded_image_from_bytes,
    build_image_generation_response_from_invocation, decode_browser_pool_inline_image_asset,
    decode_inline_image_asset, execute_browser_request, execute_browser_request_with_recovery,
    execute_connected_fetch_get_bytes, execute_connected_fetch_get_bytes_with_mode,
    execute_connected_fetch_json, execute_connected_fetch_json_with_mode,
    select_downloaded_image_mime_type,
};
pub use payload::{
    force_browser_owned_payload, relay_config_from_payload, unsupported_request_plan_error,
};
pub(crate) use result::build_gemini_canvas_browser_executor_service_result;
pub(crate) use result::classify_http_replay_worker_failure;
pub(crate) use result::extract_http_replay_worker_success;
pub(crate) use result::parse_http_replay_worker_output;
pub use result::{
    browser_request_retry_delay_ms, build_music_generation_response_from_invocation,
    build_video_generation_response_from_invocation, collect_converted_image_media_assets,
    collect_image_media_assets, convert_media_asset, decode_browser_tts_audio_payload,
    decode_inline_audio_payload, decode_modular_tts_audio_payload, extract_text_or_body_text,
    parse_browser_invocation_response, parse_connected_fetch_invocation_response,
    parse_connected_fetch_json_body, parse_remote_browser_invocation_value,
    parse_remote_media_browser_invocation_value,
    parse_remote_modular_media_browser_invocation_value, require_music_media_asset,
    require_text_result, require_video_media_asset, GeminiCanvasBrowserOwnedFetchInvocationResult,
    GeminiCanvasBrowserOwnedInvocationResult, GeminiCanvasBrowserOwnedMediaAsset,
    GeminiCanvasBrowserOwnedPoolError, GeminiCanvasBrowserOwnedPoolResult,
};

#[cfg(test)]
mod tests;
