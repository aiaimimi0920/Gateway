mod app_endpoint;
mod bootstrap;
mod browser_operation;
mod handle;
mod payload;
mod result;

pub use app_endpoint::{
    build_program_app_endpoint_official_extra_headers,
    build_program_batchexecute_request_from_invoke_contract,
    build_program_stream_generate_request_from_invoke_contract,
    gemini_canvas_program_direct_http_exhausted_error,
    gemini_canvas_program_direct_http_get_invalid_json_error,
    gemini_canvas_program_direct_http_invalid_json_error,
    missing_gemini_canvas_program_app_endpoint_handle_error,
    preferred_app_endpoint_action_aspect_ratio, preferred_app_endpoint_action_hints,
    preferred_app_endpoint_action_prompt, preferred_app_endpoint_harvest_target_url,
    preferred_app_endpoint_invoke_base_url, preferred_app_endpoint_invoke_contract,
    preferred_app_endpoint_model_name, preferred_app_endpoint_music_ws_url,
    preferred_app_endpoint_page_url, preferred_app_endpoint_video_request_url,
    GeminiCanvasProgramActionHints, GeminiCanvasProgramAppInvokeContract,
};
pub use bootstrap::{
    build_program_bootstrap_invocation_input, build_program_bootstrap_invocation_input_from_config,
    build_program_bootstrap_probe_input, build_program_create_pure_http_request,
    current_program_create_reqid, default_gemini_canvas_program_bootstrap_prompt,
    gemini_canvas_program_create_missing_handle_patch_error,
    runtime_patch_from_pure_http_create_response, GEMINI_CANVAS_PROGRAM_CREATE_RPCID,
};
pub use browser_operation::{
    append_api_key_query_if_missing, apply_program_connected_fetch_identity_contract,
    build_browser_operation_invocation_input, build_browser_operation_invocation_input_from_config,
    build_connected_fetch_invocation_input, build_connected_fetch_invocation_input_with_method,
    build_connected_fetch_invocation_input_with_method_for_payload,
    connected_fetch_mode_is_canvas_page_music_no_key, connected_fetch_mode_is_canvas_page_no_key,
    connected_fetch_mode_is_canvas_preview_music_no_key,
    connected_fetch_mode_is_canvas_preview_no_key, connected_fetch_mode_is_canvas_proxy,
    program_prefers_canvas_proxy_contract,
    program_prefers_preview_no_key_generate_content_contract,
};
pub use handle::{
    gemini_canvas_program_bootstrap_incomplete_error,
    gemini_canvas_program_payload_conversation_id,
    gemini_canvas_program_payload_handle_matches_operation,
    gemini_canvas_program_payload_has_concrete_handle, gemini_canvas_program_payload_locator,
    gemini_canvas_program_payload_operation, gemini_canvas_program_payload_page_url,
    gemini_canvas_program_payload_response_id, gemini_canvas_program_payload_source_path,
    is_concrete_gemini_canvas_app_path, is_concrete_gemini_canvas_conversation_id,
    is_concrete_gemini_canvas_program_url, normalize_gemini_canvas_program_bootstrap_operation,
    strip_gemini_canvas_program_handle_hints_from_payload,
};
pub use payload::{
    force_program_owned_payload, relay_config_from_payload, unsupported_request_plan_error,
};
pub use result::{
    decode_connected_fetch_body_bytes, gemini_canvas_program_bootstrap_missing_handle_patch_error,
    parse_connected_fetch_get_json_body, parse_connected_fetch_invocation_response,
    parse_connected_fetch_json_body, parse_program_bootstrap_invocation_response,
    parse_program_browser_invocation_response, runtime_patch_from_browser_fetch,
    runtime_patch_from_browser_invocation, GeminiCanvasBrowserFetchInvocationResult,
    GeminiCanvasBrowserInvocationResult, GeminiCanvasBrowserMediaAsset,
    GeminiCanvasBrowserPoolError, GeminiCanvasBrowserPoolResult,
};

#[cfg(test)]
mod tests;
