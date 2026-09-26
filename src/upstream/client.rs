// ---------------------------------------------------------------------------
// upstream/client.rs — Upstream HTTP client
//
// Handles building request plans, executing non-streaming requests, and
// initiating streaming responses.
// ---------------------------------------------------------------------------

use base64::Engine;
use deadpool_redis::Pool as RedisPool;
use futures::StreamExt;
use rquest::header::{HeaderMap, HeaderName, HeaderValue};
use rquest::{Client, Method, RequestBuilder};
use rquest_util::Emulation;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::{sleep, timeout};
use tracing::debug;

use crate::error::{
    classify_network_error, classify_upstream_error, sanitize_provider_error_message, GatewayError,
};
use crate::object_storage::gateway_object_storage;
use crate::protocol::accio;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse, EndpointKind};
use crate::protocol::freebuff;
use crate::protocol::gemini_business;
use crate::protocol::gemini_canvas;
use crate::protocol::gemini_web;
use crate::protocol::grok;
use crate::protocol::kiro;
use crate::protocol::lumalabs;
use crate::protocol::openai;
use crate::protocol::producer;
use crate::protocol::responses;
use crate::protocol::udio;
use crate::protocol::xfyun_websocket;
use crate::routing::candidate::{ProviderAccountPayload, ProviderExecutionMode};
use crate::upstream::accio as accio_upstream;
use crate::upstream::aistudio_web as aistudio_web_reverse_modular;
use crate::upstream::anthropic_messages as anthropic_messages_upstream;
use crate::upstream::azure_openai as azure_openai_upstream;
use crate::upstream::bedrock_converse_official_api_common as bedrock_converse_common;
use crate::upstream::bedrock_runtime_helpers::sign_bedrock_runtime_headers_if_needed;
use crate::upstream::browser_executor_helpers::{
    browser_executor_endpoint_kind_key, build_browser_executor_runtime_health,
    build_browser_executor_service_invocation_failure_response,
    build_browser_executor_service_invocation_success_response,
    unsupported_browser_executor_provider_error,
};
use crate::upstream::browser_executor_response::decode_remote_browser_executor_response;
use crate::upstream::browser_worker_runtime_helpers::{
    gemini_canvas_browser_pool_base_url, gemini_canvas_browser_pool_log_open_error,
    gemini_canvas_browser_pool_log_path, gemini_canvas_browser_pool_script_path,
    gemini_canvas_browser_pool_spawn_failed_error, gemini_canvas_browser_pool_start_mutex,
    gemini_canvas_browser_pool_start_timeout_error,
    gemini_canvas_http_replay_worker_input_serialize_error,
    gemini_canvas_http_replay_worker_script_path,
    gemini_canvas_http_replay_worker_spawn_failed_error,
    gemini_canvas_http_replay_worker_stdin_error, gemini_canvas_http_replay_worker_timeout_error,
    gemini_canvas_http_replay_worker_wait_failed_error,
    maybe_dump_gemini_canvas_http_replay_worker_debug,
};
use crate::upstream::browser_worker_types::{
    BrowserExecutorInvocationRequest, GeminiCanvasHttpReplayWorkerSuccess,
};
use crate::upstream::canonical_sse::canonical_response_to_openai_sse_bytes;
use crate::upstream::chataibot as chataibot_upstream;
use crate::upstream::chatgpt::{
    self as chatgpt_upstream, official_api as chatgpt_official_api_modular,
    web_reverse as chatgpt_web_reverse_modular,
};
use crate::upstream::cohere_chat_official_api_common as cohere_chat_common;
use crate::upstream::common::{
    build_request_builder_from_plan, insert_header_map_value, RequestPlan,
};
use crate::upstream::custom_http_request_plan;
use crate::upstream::gemini::api as gemini_api_modular;
use crate::upstream::gemini::canvas_program_web_reverse as gemini_canvas_program_web_reverse_modular;
use crate::upstream::gemini::canvas_program_web_reverse::{
    GeminiCanvasBrowserFetchInvocationResult, GeminiCanvasBrowserInvocationResult,
};
use crate::upstream::gemini::canvas_web_reverse as gemini_canvas_web_reverse_modular;
use crate::upstream::gemini::web_reverse as gemini_web_reverse_modular;
use crate::upstream::gemini_business_helpers::{
    extract_gemini_business_upload_file_id, gemini_business_unsupported_images_endpoint_error,
    parse_gemini_business_stream_response_objects,
};
use crate::upstream::gemini_canvas_asset_helpers::{
    convert_gemini_canvas_asset, gemini_canvas_asset_url_is_caller_usable,
    infer_gemini_canvas_media_kind, infer_gemini_canvas_media_mime_type,
    normalize_gemini_canvas_direct_http_asset_url, resolve_relative_url,
    should_forward_gemini_canvas_download_cookies, sniff_image_mime_type_from_bytes,
};
use crate::upstream::gemini_canvas_client_types::{
    GeminiCanvasDirectHttpApiKeyTransport, GeminiCanvasProgramAppEndpointApiContext,
    GeminiCanvasRuntimeApiContext, GeminiCanvasSignalerChannel,
};
use crate::upstream::gemini_canvas_conversation_helpers::{
    preview_gemini_canvas_conversation_entries, select_gemini_canvas_recent_conversation_entry,
};
use crate::upstream::gemini_canvas_debug_snapshot::{
    gemini_canvas_debug_headers_snapshot_from_hash_map,
    gemini_canvas_debug_headers_snapshot_from_header_map,
};
use crate::upstream::gemini_canvas_diagnostics::{
    append_gemini_canvas_image_edit_debug_json,
    append_gemini_canvas_image_edit_request_debug_snapshot,
    append_gemini_canvas_image_edit_stream_response_debug_snapshot,
    append_gemini_canvas_image_edit_trace,
};
use crate::upstream::gemini_canvas_direct_http_helpers::{
    apply_gemini_canvas_direct_http_stream_generate_headers,
    build_gemini_canvas_direct_http_bootstrap_response_meta,
    build_gemini_canvas_direct_http_page_harvest_bootstrap_request_contract,
    build_gemini_canvas_direct_http_page_harvest_failure_error,
    build_gemini_canvas_direct_http_text_bootstrap_failure_error,
    build_gemini_canvas_direct_http_text_bootstrap_request_contract,
    build_gemini_canvas_direct_http_text_generic_preflight_specs,
    fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale,
    gemini_canvas_direct_http_api_key_transports, gemini_canvas_direct_http_bootstrap_candidates,
    gemini_canvas_direct_http_text_fast_version_preflight_spec,
    gemini_canvas_direct_http_text_state_variant_preflight_specs,
    gemini_canvas_image_fetch_invalid_inline_bytes_error,
    gemini_canvas_image_fetch_missing_inline_bytes_error,
    gemini_canvas_image_json_attempts_exhausted_error,
    gemini_canvas_media_fetch_bad_asset_url_error, gemini_canvas_media_fetch_bad_redirect_error,
    gemini_canvas_media_fetch_cookie_mismatch_html_error,
    gemini_canvas_media_fetch_cookie_mismatch_redirect_error,
    gemini_canvas_media_fetch_redirect_exhausted_error,
    gemini_canvas_media_fetch_redirect_missing_location_error,
    redact_gemini_canvas_api_key_for_logs, resolve_gemini_canvas_direct_http_bootstrap_page_path,
    resolve_gemini_canvas_direct_http_preflight_source_path,
    resolve_gemini_canvas_direct_http_referer,
    resolve_gemini_canvas_direct_http_stream_generate_model_header,
    send_gemini_canvas_direct_http_json_with_options, GEMINI_CANVAS_PUBLIC_PAGE_API_KEY_FALLBACKS,
};
use crate::upstream::gemini_canvas_error_helpers::{
    classify_gemini_canvas_pure_http_error, gemini_canvas_body_indicates_context_busy,
};
use crate::upstream::gemini_canvas_fetch_headers::build_gemini_canvas_direct_http_media_fetch_headers;
use crate::upstream::gemini_canvas_followup_types::{
    build_gemini_canvas_direct_http_video_missing_asset_error,
    build_gemini_canvas_followup_bootstrap_candidates, build_gemini_canvas_image_recovery_strategy,
    build_gemini_canvas_media_followup_missing_asset_error,
    build_gemini_canvas_media_followup_preflight_plan, build_gemini_canvas_page_poll_urls,
    build_gemini_canvas_video_accepted_response_from_body,
    build_gemini_canvas_video_completion_requests, classify_gemini_canvas_page_target_mode,
    classify_gemini_canvas_video_stage_body, extract_gemini_canvas_video_operation_download_uri,
    finalize_gemini_canvas_media_followup_attempt_state,
    gemini_canvas_conversation_page_missing_asset_error,
    gemini_canvas_conversation_page_poll_fetch_error_entry,
    gemini_canvas_conversation_page_poll_refresh_body_entry,
    gemini_canvas_conversation_page_poll_refresh_error_entry,
    gemini_canvas_media_followup_bootstrap_failed_error, gemini_canvas_media_followup_failed_error,
    gemini_canvas_modular_video_unsupported_count_error,
    gemini_canvas_program_video_browser_fallback_forbidden_error,
    gemini_canvas_program_video_invoke_target_missing_error,
    gemini_canvas_program_video_no_key_empty_body_error,
    gemini_canvas_program_video_no_key_invalid_json_error,
    gemini_canvas_program_video_no_key_poll_empty_body_error,
    gemini_canvas_program_video_no_key_poll_invalid_json_error,
    gemini_canvas_program_video_no_key_request_contract_missing_error,
    gemini_canvas_program_video_no_key_request_exhausted_error,
    gemini_canvas_video_body_has_usable_asset, gemini_canvas_video_followup_missing_locator_error,
    gemini_canvas_video_missing_asset_error, gemini_canvas_video_missing_operation_error,
    gemini_canvas_video_music_modality_mismatch_error, gemini_canvas_video_operation_timeout_error,
    gemini_canvas_video_unsupported_count_error, plan_gemini_canvas_conversation_page_poll_timing,
    prepare_gemini_canvas_direct_http_image_context as prepare_image_context,
    prepare_gemini_canvas_page_seed, resolve_gemini_canvas_conversation_page_poll_refresh_target,
    resolve_gemini_canvas_conversation_page_poll_remaining,
    should_sleep_after_gemini_canvas_conversation_page_poll_attempt,
    try_extract_gemini_canvas_conversation_page_poll_assets_from_body,
    GeminiCanvasDirectHttpImageContext, GeminiCanvasDirectHttpImageJsonAction,
    GeminiCanvasDirectHttpImageJsonPolicy, GeminiCanvasDirectHttpImagePrimaryResult,
    GeminiCanvasFollowupTarget, GeminiCanvasImageEditFollowupContext,
    GeminiCanvasImageEditPostAckFollowupResult, GeminiCanvasImageRecoveryMode,
    GeminiCanvasImageRecoveryStrategy, GeminiCanvasImageResponsePlan,
    GeminiCanvasImageTemplateRetryAction, GeminiCanvasMediaCaptureParityPreflightResult,
    GeminiCanvasMediaFollowupAttemptOutcome, GeminiCanvasMediaFollowupAttemptState,
    GeminiCanvasMediaFollowupContext, GeminiCanvasMediaFollowupPreflightContract,
    GeminiCanvasMediaFollowupPreflightOutcome, GeminiCanvasMediaFollowupPreflightPlan,
    GeminiCanvasMediaFollowupPreflightStrategy, GeminiCanvasVideoCompletionAttemptOutcome,
    GeminiCanvasVideoCompletionAttemptState, GeminiCanvasVideoCompletionPollState,
    GeminiCanvasVideoCompletionRequests,
};
use crate::upstream::gemini_canvas_form_helpers::{
    append_query_pairs_to_url, extract_gemini_canvas_batchexecute_xsrf_token,
    gemini_canvas_browser_fetch_headers_from_header_map, header_map_string_from_form,
    maybe_retry_gemini_canvas_form_xsrf_token,
    maybe_retry_gemini_canvas_stream_template_access_token, serialize_form_urlencoded_pairs,
    should_attempt_gemini_canvas_browser_backed_image_edit_retry, upsert_form_field,
};
use crate::upstream::gemini_canvas_image_edit_local_helpers::{
    build_gemini_canvas_image_edit_heavy_builder_debug_extra,
    build_gemini_canvas_image_edit_seed_debug_snapshot,
    build_gemini_canvas_image_edit_signaler_poll_url,
    build_gemini_canvas_image_edit_stream_request_contract,
    build_gemini_canvas_image_edit_stream_response_heavy_debug_extra,
    build_gemini_canvas_image_edit_stream_response_meta,
    build_gemini_canvas_image_edit_stream_response_template_after_refresh_debug_extra,
    build_gemini_canvas_image_edit_stream_response_template_before_refresh_debug_extra,
    build_gemini_canvas_image_edit_template_miss_debug_snapshot,
    build_gemini_canvas_image_edit_template_post_refresh_debug_extra,
    build_gemini_canvas_image_edit_template_pre_refresh_debug_extra,
    build_gemini_canvas_image_edit_uploaded_refs_debug_snapshot,
    extract_gemini_canvas_image_edit_signaler_assets_from_body,
    finish_gemini_canvas_image_edit_signaler_missing_asset,
    gemini_canvas_image_edit_conversation_bootstrap_missing_error,
    gemini_canvas_image_edit_conversation_followup_failed_error,
    gemini_canvas_image_edit_post_ack_bootstrap_missing_error,
    gemini_canvas_image_edit_post_ack_missing_app_url_error,
    gemini_canvas_image_edit_signaler_page_failure_entry,
    gemini_canvas_image_edit_signaler_poll_error_entry,
    gemini_canvas_image_edit_signaler_refresh_error_entry,
    gemini_canvas_image_page_refresh_bootstrap_missing_error,
    gemini_canvas_runtime_mirror_json_path, gemini_canvas_sidecar_has_any_key,
    prepare_gemini_canvas_image_edit_signaler_poll_state_with_http,
    prewarm_gemini_canvas_image_edit_signaler_with_http, read_gemini_canvas_runtime_mirror_json,
    record_gemini_canvas_image_edit_signaler_app_path,
    record_gemini_canvas_image_edit_signaler_poll_body_preview,
    refresh_gemini_canvas_image_edit_signaler_creds_from_body_with_http,
    resolved_gemini_canvas_browser_runtime_state_object_key,
    send_gemini_canvas_signaler_poll_request_refreshing_session_with_http,
    try_extract_gemini_canvas_image_edit_signaler_assets_response_from_body,
    try_finish_gemini_canvas_image_edit_signaler_handoff_ready,
    update_gemini_canvas_image_edit_signaler_next_aid_from_body,
};
use crate::upstream::gemini_canvas_music_helpers::{
    build_gemini_canvas_music_accepted_response_from_body,
    decode_gemini_canvas_program_music_no_key_audio,
    gemini_canvas_music_body_indicates_accepted_progress, gemini_canvas_music_missing_asset_error,
    gemini_canvas_music_response_requires_browser_followup,
    gemini_canvas_program_music_invoke_target_missing_error,
    gemini_canvas_program_music_no_key_browserless_stage_incomplete_error,
    gemini_canvas_program_music_no_key_contract_missing_error,
    gemini_canvas_program_music_no_key_post_1060_contract_missing_error,
    gemini_canvas_program_music_no_key_prelude_stage_incomplete_error,
    gemini_canvas_program_music_no_key_request_contract_missing_error,
    gemini_canvas_program_music_page_poll_failed_error,
    gemini_canvas_program_music_recent_page_poll_failed_error,
    select_preferred_gemini_canvas_music_asset,
};
use crate::upstream::gemini_canvas_official_api_helpers::{
    execute_gemini_canvas_official_music, execute_gemini_canvas_official_video,
};
use crate::upstream::gemini_canvas_program_route_helpers::{
    ensure_gemini_canvas_program_payload_avoids_official_api_identity,
    gemini_canvas_program_image_pure_http_required_error,
    gemini_canvas_program_runtime_material_official_api_key_forbidden_error,
    gemini_canvas_program_tts_pure_http_required_error,
    should_attempt_gemini_canvas_program_modular_media_direct_http,
    should_treat_gemini_canvas_program_modular_media_direct_http_as_authoritative,
};
use crate::upstream::gemini_canvas_request_headers::{
    apply_browser_fetch_client_hints, apply_gemini_canvas_capture_aligned_batchexecute_headers,
    apply_gemini_canvas_cookie_header, apply_gemini_canvas_navigation_headers,
    apply_gemini_canvas_replay_template_headers, apply_gemini_canvas_response_cookies,
    apply_gemini_canvas_same_origin_batchexecute_headers, apply_gemini_canvas_signed_headers,
};
use crate::upstream::gemini_canvas_runtime_error_helpers::{
    append_gateway_error_fields, append_gateway_error_summary,
    compact_response_preview as compact_gemini_diagnostic_preview,
    compact_sanitized_response_preview, summarize_gateway_error,
    wrap_gemini_canvas_stream_parse_error,
};
use crate::upstream::gemini_canvas_runtime_helpers::{
    current_unix_timestamp_i64, gemini_canvas_http_origin, gemini_canvas_page_base_url,
    gemini_canvas_pure_http_session_from_payload_or_storage,
    gemini_canvas_runtime_api_missing_google_api_key_error, gemini_canvas_signaler_zx_token,
    origin_from_url, persist_gemini_canvas_program_runtime_material,
};
use crate::upstream::gemini_canvas_text_helpers::{
    build_gemini_canvas_text_success_body, gemini_canvas_generic_welcome_response_error,
    gemini_canvas_text_response_is_generic_welcome,
};
use crate::upstream::gemini_canvas_upload_http::upload_gemini_canvas_image_edit_inputs_with_http;
use crate::upstream::grok as grok_upstream;
use crate::upstream::header_map_helpers::header_map_string;
use crate::upstream::headers::build_upstream_headers_with;
use crate::upstream::kiro as kiro_upstream;
use crate::upstream::openai_compatible_request_plan as openai_compatible_request_plan_modular;
use crate::upstream::producer_media_helpers::{
    build_producer_browser_executor_payload_from_prepared, execute_producer_browser_worker,
    execute_producer_image_http, execute_producer_music_http, execute_producer_video_http,
    prepare_producer_browser_execution_input, prepare_producer_browser_executor_service_input,
    should_fallback_producer_video_http_error,
};
use crate::upstream::producer_session_helpers::maybe_refresh_producer_headers;
use crate::upstream::qwen::{
    official_api as qwen_official_api_modular, web_reverse as qwen_web_reverse_modular,
};
use crate::upstream::request_time_browser_policy::{
    remote_browser_executor_required_unavailable_error, request_time_browser_forbidden_error,
    RequestTimeBrowserPolicy,
};
use crate::upstream::response_preview_helpers::{
    compact_response_preview, truncate_response_preview,
};
use crate::upstream::response_types::{BinaryUpstreamResponse, UpstreamStreamingResponse};
use crate::upstream::search_provider_helpers;
use crate::upstream::upstream_payload_helpers::{
    merge_extra_body_patch_into_payload, read_json_string,
};

pub use crate::upstream::browser_worker_types::{
    BrowserExecutorInvocationError, BrowserExecutorServiceHealth,
    BrowserExecutorServiceInvocationRequest, BrowserExecutorServiceInvocationResponse,
};

#[derive(Clone, Debug)]
// ---------------------------------------------------------------------------
// UpstreamClient
// ---------------------------------------------------------------------------

/// Thin wrapper around [`rquest::Client`] that knows how to talk to AI
/// provider APIs.
pub struct UpstreamClient {
    pub(crate) freebuff: std::sync::Arc<crate::protocol::freebuff::RunRuntime>,
    pub(crate) http: Client,
    plain_http: Client,
    pub(crate) timeout: Duration,
    browser_executor_base_url: Option<String>,
    browser_executor_bearer_token: Option<String>,
    request_time_browser_policy: RequestTimeBrowserPolicy,
    redis_pool: Option<RedisPool>,
    pg_pool: Option<PgPool>,
}

const BROWSER_EXECUTOR_HTTP_GRACE: Duration = Duration::from_secs(20);

fn browser_executor_remote_request_timeout(default_timeout: Duration, input: &Value) -> Duration {
    let worker_timeout = input
        .get("timeoutMs")
        .and_then(Value::as_u64)
        .map(Duration::from_millis)
        .unwrap_or(default_timeout.max(Duration::from_secs(30)));
    worker_timeout.saturating_add(BROWSER_EXECUTOR_HTTP_GRACE)
}

#[path = "client_initialization.rs"]
mod initialization;

#[path = "client/browser_executor.rs"]
mod browser_executor;
#[path = "client/canvas_assets_part_1.rs"]
mod canvas_assets_part_1;
#[path = "client/canvas_assets_part_2.rs"]
mod canvas_assets_part_2;
#[path = "client/canvas_assets_part_3.rs"]
mod canvas_assets_part_3;
#[path = "client/canvas_browser_part_1.rs"]
mod canvas_browser_part_1;
#[path = "client/canvas_browser_part_2.rs"]
mod canvas_browser_part_2;
#[path = "client/canvas_browser_part_3.rs"]
mod canvas_browser_part_3;
#[path = "client/canvas_completion_part_1.rs"]
mod canvas_completion_part_1;
#[path = "client/canvas_completion_part_2.rs"]
mod canvas_completion_part_2;
#[path = "client/canvas_completion_part_3.rs"]
mod canvas_completion_part_3;
#[path = "client/canvas_followups_part_1.rs"]
mod canvas_followups_part_1;
#[path = "client/canvas_followups_part_2.rs"]
mod canvas_followups_part_2;
#[path = "client/canvas_image_edit.rs"]
mod canvas_image_edit;
#[path = "client/canvas_official_part_1.rs"]
mod canvas_official_part_1;
#[path = "client/canvas_official_part_2.rs"]
mod canvas_official_part_2;
#[path = "client/canvas_official_part_3.rs"]
mod canvas_official_part_3;
#[path = "client/canvas_preflight_part_1.rs"]
mod canvas_preflight_part_1;
#[path = "client/canvas_preflight_part_2.rs"]
mod canvas_preflight_part_2;
#[path = "client/canvas_preflight_part_3.rs"]
mod canvas_preflight_part_3;
#[path = "client/canvas_primary_part_1.rs"]
mod canvas_primary_part_1;
#[path = "client/canvas_primary_part_2.rs"]
mod canvas_primary_part_2;
#[path = "client/canvas_program_part_1.rs"]
mod canvas_program_part_1;
#[path = "client/canvas_program_part_2.rs"]
mod canvas_program_part_2;
#[path = "client/canvas_program_part_3.rs"]
mod canvas_program_part_3;
#[path = "client/canvas_program_routes_part_1.rs"]
mod canvas_program_routes_part_1;
#[path = "client/canvas_program_routes_part_2.rs"]
mod canvas_program_routes_part_2;
#[path = "client/canvas_program_routes_part_3.rs"]
mod canvas_program_routes_part_3;
#[path = "client/canvas_relay.rs"]
mod canvas_relay;
#[path = "client/canvas_stream_generate.rs"]
mod canvas_stream_generate;
#[path = "client/canvas_video_continuation.rs"]
mod canvas_video_continuation;
#[path = "client/gemini_business_execution.rs"]
mod gemini_business_execution;
#[path = "client/passthrough.rs"]
mod passthrough;
#[path = "client/request_plan_part_1.rs"]
mod request_plan_part_1;
#[path = "client/request_plan_part_2.rs"]
mod request_plan_part_2;
#[path = "client/streaming.rs"]
mod streaming;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[path = "client/fallback_policy.rs"]
mod fallback_policy;
pub(crate) use fallback_policy::{
    apply_gemini_canvas_browser_video_continuation,
    build_gemini_canvas_text_direct_http_fallback_attempts,
    build_gemini_canvas_video_continuation_seed_body, gemini_canvas_stream_collection_policy,
    gemini_canvas_video_continuation_from_request, should_fallback_gemini_canvas_image_to_browser,
    should_preserve_gemini_canvas_video_continuation_as_pending,
    GeminiCanvasTextDirectHttpFallbackAttempt, GeminiCanvasVideoContinuation,
};

#[cfg(test)]
#[path = "client/tests.rs"]
mod tests;
