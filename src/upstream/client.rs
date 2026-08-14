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

use crate::error::{classify_network_error, classify_upstream_error, GatewayError};
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
    classify_browser_executor_invocation_failure, extract_browser_executor_invocation_success,
    parse_browser_executor_invocation_response_body, unsupported_browser_executor_provider_error,
};
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
    prepare_gemini_canvas_direct_http_image_context, prepare_gemini_canvas_page_seed,
    resolve_gemini_canvas_conversation_page_poll_refresh_target,
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
    append_gemini_canvas_image_edit_debug_json,
    append_gemini_canvas_image_edit_request_debug_snapshot,
    append_gemini_canvas_image_edit_stream_response_debug_snapshot,
    append_gemini_canvas_image_edit_trace,
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
    gemini_canvas_debug_headers_snapshot_from_hash_map,
    gemini_canvas_debug_headers_snapshot_from_header_map,
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
    upload_gemini_canvas_image_edit_inputs_with_http,
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
    append_gateway_error_summary, summarize_gateway_error,
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
    remote_browser_executor_required_failed_error,
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

impl UpstreamClient {
    /// Create a new client with the given request timeout.
    ///
    /// Uses a recent Chrome TLS fingerprint via rquest's BoringSSL backend so that
    /// providers that check JA3/JA4 (e.g. Grok) accept the connection.
    pub fn new(timeout_secs: u64) -> Self {
        Self::new_with_runtime(timeout_secs, None, None)
    }

    pub fn new_with_runtime(
        timeout_secs: u64,
        redis_pool: Option<RedisPool>,
        pg_pool: Option<PgPool>,
    ) -> Self {
        let http = Client::builder()
            .emulation(Emulation::Chrome136)
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .expect("failed to build rquest client");
        let plain_http = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .expect("failed to build plain rquest client");
        let browser_executor_base_url = std::env::var("GATEWAY_BROWSER_EXECUTOR_BASE_URL")
            .ok()
            .map(|value| value.trim().trim_end_matches('/').to_string())
            .filter(|value| !value.is_empty());
        let browser_executor_bearer_token = std::env::var("GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let request_time_browser_policy = RequestTimeBrowserPolicy::from_env();

        Self {
            http,
            plain_http,
            timeout: Duration::from_secs(timeout_secs),
            browser_executor_base_url,
            browser_executor_bearer_token,
            request_time_browser_policy,
            redis_pool,
            pg_pool,
        }
    }

    /// Get a reference to the underlying HTTP client (for token refresh etc.)
    pub fn client(&self) -> &Client {
        &self.http
    }

    pub fn browser_executor_runtime_health(&self) -> BrowserExecutorServiceHealth {
        build_browser_executor_runtime_health(self.browser_executor_base_url.clone())
    }

    // ── request plan ─────────────────────────────────────────────────────

    /// Determine the upstream URL and packed body for the given request.
    ///
    /// The endpoint path, method, and body format depend on the adapter type:
    /// - `openai_compatible` → `/v1/chat/completions`, packed with OpenAI format
    /// - `anthropic_compatible` → `/v1/messages`, packed with Anthropic format
    /// - `grok_compatible` → `/rest/app-chat/conversations/new`, packed with Grok format
    /// - `search_api_compatible` → JSON passthrough search-provider endpoints
    /// - `lumalabs_compatible` → custom image/video/audio passthrough via board action + SSE events
    /// - `gemini_canvas_compatible` → Gemini Canvas reverse-web generation via direct HTTP replay with optional legacy browser fallback
    /// - `producer_compatible` → Producer.ai reverse-web image / music / music-video generation via internal web routes
    /// - `suno_compatible` → Suno reverse-web image / audio / video generation via challenge check + v2-web submit + feed/v3 polling
    /// - `udio_compatible` → Udio image / music / video generation via browser worker + page-context generate/poll
    /// - `custom_http` → `base_url` as-is (no path appended), raw body forwarded
    pub fn build_request_plan(
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        stream: bool,
    ) -> Result<RequestPlan, GatewayError> {
        let mut plan = match payload.canonical_adapter() {
            "anthropic_compatible" => {
                if qwen_official_api_modular::owns_payload(payload) {
                    qwen_official_api_modular::build_request_plan(payload, req, model, stream)
                } else if anthropic_messages_upstream::owns_payload(payload) {
                    anthropic_messages_upstream::build_request_plan(payload, req, model, stream)
                } else {
                    anthropic_messages_upstream::build_request_plan(payload, req, model, stream)
                }
            }

            "xfyun_websocket_compatible" => {
                xfyun_websocket::build_request_plan(payload, req, model)
            }

            "openai_compatible" => {
                if chatgpt_official_api_modular::owns_payload(payload) {
                    chatgpt_official_api_modular::build_request_plan(payload, req, model, stream)
                } else if qwen_official_api_modular::owns_payload(payload) {
                    qwen_official_api_modular::build_request_plan(payload, req, model, stream)
                } else if azure_openai_upstream::owns_payload(payload) {
                    azure_openai_upstream::build_request_plan(payload, req, model, stream)
                } else {
                    openai_compatible_request_plan_modular::build_request_plan(
                        payload, req, model, stream,
                    )
                }
            }

            "gemini_api_compatible" | "gemini_api_modular_compatible" => {
                gemini_api_modular::build_request_plan(payload, req, model, stream)
            }

            "bedrock_converse_compatible" => {
                bedrock_converse_common::build_request_plan(payload, req, model, stream)
            }

            "cohere_compatible" => {
                cohere_chat_common::build_request_plan(payload, req, model, stream)
            }

            "accio_compatible" => accio_upstream::build_request_plan(payload, req, model, stream),

            "grok_compatible" => grok_upstream::build_request_plan(payload, req, model),

            "kiro_compatible" => kiro_upstream::build_request_plan(payload, req, model),

            "freebuff_compatible" => freebuff::build_request_plan(payload, req, model, stream),

            "qwen_web_compatible" => {
                Err(qwen_web_reverse_modular::unsupported_request_plan_error())
            }

            "chatgpt_web_reverse_compatible" => {
                Err(chatgpt_web_reverse_modular::unsupported_request_plan_error())
            }

            adapter if aistudio_web_reverse_modular::is_aistudio_web_reverse_adapter(adapter) => {
                Err(aistudio_web_reverse_modular::unsupported_request_plan_error())
            }

            "gemini_web_compatible" => Err(gemini_web::unsupported_request_plan_error()),

            "gemini_web_reverse_modular_compatible" => {
                Err(gemini_web_reverse_modular::unsupported_request_plan_error())
            }

            "gemini_business_compatible" => Err(gemini_business::unsupported_request_plan_error()),

            "chataibot_compatible" => Err(chataibot_upstream::unsupported_request_plan_error()),

            "lumalabs_compatible" => Err(lumalabs::unsupported_request_plan_error()),

            "gemini_canvas_compatible" => Err(gemini_canvas::unsupported_request_plan_error()),

            "gemini_canvas_web_reverse_compatible" => {
                Err(gemini_canvas_web_reverse_modular::unsupported_request_plan_error())
            }

            "gemini_canvas_program_web_reverse_compatible" => {
                Err(gemini_canvas_program_web_reverse_modular::unsupported_request_plan_error())
            }

            "producer_compatible" => Err(producer::unsupported_request_plan_error()),

            "suno_compatible" => Err(crate::upstream::suno::request_plan_unsupported_error()),

            "udio_compatible" => Err(udio::unsupported_request_plan_error()),

            "search_api_compatible" => {
                search_provider_helpers::build_search_provider_request_plan(payload, req)
            }

            _ => Ok(custom_http_request_plan::build_request_plan(payload, req)),
        }?;

        // Merge data-driven provider constraints (e.g. Codex "store": false).
        // Only injects keys NOT already present in the body.
        if let Some(extra) = &payload.extra_body {
            if let Some(Value::Object(map)) = plan.body.as_mut() {
                for (key, value) in extra {
                    if payload.canonical_adapter() == "kiro_compatible"
                        && kiro::is_reserved_payload_extra_key(key)
                    {
                        continue;
                    }
                    if payload.canonical_adapter() == "freebuff_compatible"
                        && freebuff::is_reserved_payload_extra_key(key)
                    {
                        continue;
                    }
                    map.entry(key.clone()).or_insert_with(|| value.clone());
                }
            }
        }

        tracing::debug!(url = %plan.url, "request plan built");
        Ok(plan)
    }

    // ── non-streaming execute ─────────────────────────────────────────────

    /// Execute a non-streaming upstream request and return a canonical response.
    ///
    /// `extra_headers` are merged into the upstream request headers (lower
    /// priority than provider config).  Pass `None` when no additional headers
    /// are needed.
    ///
    /// **Accio special case**: Accio (phoenix-gw) always returns SSE — even
    /// for "non-streaming" requests (it returns 406 for `Accept: application/json`).
    /// So for `accio_compatible` adapters, we force `stream=true` in the
    /// request plan, then read and accumulate the entire SSE stream into a
    /// single canonical response.
    ///
    /// **Grok special case**: Grok always returns NDJSON — even for
    /// "non-streaming" requests. So for `grok_compatible` adapters, we read and
    /// accumulate the entire NDJSON stream into a single canonical response.
    pub async fn execute(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<CanonicalRelayResponse, GatewayError> {
        self.execute_with_provider_account_id("", payload, req, model, extra_headers)
            .await
    }

    pub async fn execute_with_provider_account_id(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<CanonicalRelayResponse, GatewayError> {
        if payload.canonical_adapter() == "xfyun_websocket_compatible" {
            let _ = extra_headers;
            debug!(
                model,
                "sending upstream request (xfyun websocket: stream-only upstream + accumulate)"
            );
            return xfyun_websocket::execute_nonstream(payload, req, model).await;
        }

        if payload.adapter == "qwen_web_compatible" {
            debug!(
                model,
                "sending upstream request (qwen web: create chat + accumulate)"
            );
            return self
                .execute_qwen_web(payload, req, model, extra_headers)
                .await;
        }

        if payload.adapter == "chatgpt_web_reverse_compatible" {
            debug!(
                model,
                "sending upstream request (chatgpt web reverse: bootstrap + sentinel + conversation accumulate)"
            );
            return chatgpt_upstream::execute(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
                extra_headers,
            )
            .await;
        }

        if aistudio_web_reverse_modular::is_aistudio_web_reverse_adapter(&payload.adapter) {
            debug!(
                model,
                "sending upstream request (aistudio web reverse: browser-owned generateContent accumulate)"
            );
            return self
                .execute_aistudio_web(provider_account_id, payload, req, model, extra_headers)
                .await;
        }

        if matches!(payload.adapter.as_str(), "gemini_web_compatible") {
            debug!(
                model,
                "sending upstream request (gemini web: bootstrap app + StreamGenerate accumulate)"
            );
            return gemini_web_reverse_modular::execute(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
                extra_headers,
            )
            .await;
        }
        if let Some(legacy_route) = gemini_web_reverse_modular::legacy_mixed_lane_execution_route(
            payload,
            req.endpoint_kind,
            false,
        ) {
            if legacy_route.kind
                == gemini_web_reverse_modular::LegacyMixedLaneExecutionKind::TextAccumulate
            {
                debug!(
                    model,
                    endpoint = ?req.endpoint_kind,
                    "sending upstream request (gemini web reverse modular: legacy mixed-lane text accumulate)"
                );
                return gemini_web_reverse_modular::execute_legacy_text(
                    self,
                    &legacy_route.payload,
                    req,
                    model,
                    extra_headers,
                )
                .await;
            }
        }

        if payload.adapter == "gemini_canvas_compatible"
            && matches!(
                req.endpoint_kind,
                EndpointKind::ChatCompletions
                    | EndpointKind::Messages
                    | EndpointKind::Responses
                    | EndpointKind::Completions
            )
        {
            debug!(
                model,
                endpoint = ?req.endpoint_kind,
                "sending upstream request (gemini canvas: browser-backed reverse-web text accumulate)"
            );
            return self
                .execute_gemini_canvas_text(payload, req, model, extra_headers)
                .await;
        }
        if payload.adapter == "gemini_canvas_web_reverse_compatible"
            && matches!(
                req.endpoint_kind,
                EndpointKind::ChatCompletions
                    | EndpointKind::Messages
                    | EndpointKind::Responses
                    | EndpointKind::Completions
            )
        {
            let browser_owned_payload =
                gemini_canvas_web_reverse_modular::force_browser_owned_payload(payload);
            debug!(
                model,
                endpoint = ?req.endpoint_kind,
                "sending upstream request (gemini canvas browser relay modular: connected-canvas text accumulate)"
            );
            return self
                .execute_gemini_canvas_modular_browser_relay_text(
                    &browser_owned_payload,
                    req,
                    model,
                    extra_headers,
                )
                .await;
        }
        if payload.adapter == "gemini_canvas_program_web_reverse_compatible"
            && matches!(
                req.endpoint_kind,
                EndpointKind::ChatCompletions
                    | EndpointKind::Messages
                    | EndpointKind::Responses
                    | EndpointKind::Completions
            )
        {
            let program_owned_payload =
                gemini_canvas_program_web_reverse_modular::force_program_owned_payload(payload);
            debug!(
                model,
                endpoint = ?req.endpoint_kind,
                "sending upstream request (gemini canvas program modular: share-seeded concrete-program text accumulate)"
            );
            return self
                .execute_gemini_canvas_modular_browser_relay_text(
                    &program_owned_payload,
                    req,
                    model,
                    extra_headers,
                )
                .await;
        }

        let force_streaming_responses_bridge =
            payload.prefers_forced_streaming_responses(req.endpoint_kind);

        if chatgpt_official_api_modular::supports_forced_streaming_accumulate(
            payload,
            req.endpoint_kind,
        ) {
            return self
                .execute_chatgpt_official_forced_streaming_accumulate(
                    payload,
                    req,
                    model,
                    extra_headers,
                )
                .await;
        }

        if force_streaming_responses_bridge {
            let plan = Self::build_request_plan(payload, req, model, true)?;
            let headers = build_upstream_headers_with(payload, extra_headers);
            let provider = &payload.adapter;

            debug!(
                url = %plan.url,
                model,
                endpoint = ?req.endpoint_kind,
                "sending upstream request (openai-family text bridge: forced streaming + accumulate)"
            );

            let response = self
                .send_plan(&plan, headers)
                .send()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;

            let status = response.status().as_u16();

            if !response.status().is_success() {
                let body_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| String::from("<unreadable body>"));
                return Err(classify_upstream_error(status, &body_text, Some(provider)));
            }

            return responses::accumulate_responses_stream(response, model).await;
        }

        if gemini_api_modular::supports_forced_streaming_accumulate(payload.adapter.as_str()) {
            let context = gemini_api_modular::prepare_forced_streaming_execute_context(
                payload,
                req,
                model,
                extra_headers,
            )?;
            let provider = context.provider;
            let plan = context.plan;
            let headers = context.headers;

            debug!(
                url = %plan.url,
                model,
                adapter = %provider,
                "sending upstream request (gemini api: forced streaming + accumulate)"
            );

            return gemini_api_modular::execute_forced_streaming_accumulate(
                self.send_plan(&plan, headers),
                provider,
                model,
            )
            .await;
        }

        // Accio-style adapters return stream/event-oriented payloads even for
        // nominally non-streaming requests, so force streaming and accumulate.
        if payload.adapter == "accio_compatible" {
            let plan = Self::build_request_plan(payload, req, model, true)?;
            let headers = build_upstream_headers_with(payload, extra_headers);
            let provider = &payload.adapter;

            debug!(url = %plan.url, model, adapter = %payload.adapter, "sending upstream request (streaming adapter: forced streaming + accumulate)");

            let response = self
                .send_plan(&plan, headers)
                .send()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;

            let status = response.status().as_u16();

            if !response.status().is_success() {
                let body_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| String::from("<unreadable body>"));
                return Err(classify_upstream_error(status, &body_text, Some(provider)));
            }

            return accio::accumulate_accio_stream(response, model).await;
        }

        if payload.adapter == "bedrock_converse_compatible" {
            let plan = Self::build_request_plan(payload, req, model, true)?;
            let headers = sign_bedrock_runtime_headers_if_needed(
                payload,
                &plan,
                build_upstream_headers_with(payload, extra_headers),
            )?;
            let provider = &payload.adapter;

            debug!(url = %plan.url, model, adapter = %payload.adapter, "sending upstream request (bedrock converse: forced streaming + accumulate)");

            let response = self
                .send_plan(&plan, headers)
                .send()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;

            let status = response.status().as_u16();

            if !response.status().is_success() {
                let body_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| String::from("<unreadable body>"));
                return Err(classify_upstream_error(status, &body_text, Some(provider)));
            }

            return bedrock_converse_common::accumulate_stream(response, model).await;
        }

        if payload.adapter == "cohere_compatible" {
            let plan = Self::build_request_plan(payload, req, model, true)?;
            let headers = build_upstream_headers_with(payload, extra_headers);
            let provider = &payload.adapter;

            debug!(url = %plan.url, model, adapter = %payload.adapter, "sending upstream request (cohere chat: forced streaming + accumulate)");

            let response = self
                .send_plan(&plan, headers)
                .send()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;

            let status = response.status().as_u16();

            if !response.status().is_success() {
                let body_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| String::from("<unreadable body>"));
                return Err(classify_upstream_error(status, &body_text, Some(provider)));
            }

            return cohere_chat_common::accumulate_stream(response, model).await;
        }

        // Grok always returns NDJSON, so force streaming and accumulate.
        if payload.adapter == "grok_compatible" {
            let plan = Self::build_request_plan(payload, req, model, true)?;
            let headers = build_upstream_headers_with(payload, extra_headers);
            let provider = &payload.adapter;

            debug!(url = %plan.url, model, "sending upstream request (grok: forced streaming + accumulate)");

            let response = self
                .send_plan(&plan, headers)
                .send()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;

            let status = response.status().as_u16();

            if !response.status().is_success() {
                let body_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| String::from("<unreadable body>"));
                return Err(classify_upstream_error(status, &body_text, Some(provider)));
            }

            return grok::accumulate_grok_stream(response, model).await;
        }

        if payload.adapter == "kiro_compatible" {
            let plan = Self::build_request_plan(payload, req, model, true)?;
            let headers = build_upstream_headers_with(payload, extra_headers);
            let provider = &payload.adapter;

            debug!(url = %plan.url, model, "sending upstream request (kiro: forced streaming + accumulate)");

            let response = self
                .send_plan(&plan, headers)
                .send()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;

            let status = response.status().as_u16();

            if !response.status().is_success() {
                let body_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| String::from("<unreadable body>"));
                return Err(classify_upstream_error(status, &body_text, Some(provider)));
            }

            return kiro_upstream::accumulate_stream(response, model, req).await;
        }

        if anthropic_messages_upstream::owns_payload(payload) {
            let plan = Self::build_request_plan(payload, req, model, true)?;
            let headers = build_upstream_headers_with(payload, extra_headers);
            let provider = &payload.adapter;

            debug!(
                url = %plan.url,
                model,
                "sending upstream request (anthropic: forced streaming + accumulate)"
            );

            let response = self
                .send_plan(&plan, headers)
                .send()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;

            let status = response.status().as_u16();

            if !response.status().is_success() {
                let body_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| String::from("<unreadable body>"));
                return Err(classify_upstream_error(status, &body_text, Some(provider)));
            }

            return anthropic_messages_upstream::accumulate_stream(response, model).await;
        }

        if chatgpt_official_api_modular::owns_payload(payload) {
            return self
                .execute_chatgpt_official_nonstreaming(payload, req, model, extra_headers)
                .await;
        }

        let (plan, headers, provider) = (
            Self::build_request_plan(payload, req, model, false)?,
            build_upstream_headers_with(payload, extra_headers),
            payload.adapter.clone(),
        );

        debug!(url = %plan.url, model, "sending upstream request");

        let response = self
            .send_plan(&plan, headers)
            .send()
            .await
            .map_err(|e| classify_network_error(&e, Some(provider.as_str())))?;

        let status = response.status().as_u16();

        if !response.status().is_success() {
            let body_text = response
                .text()
                .await
                .unwrap_or_else(|_| String::from("<unreadable body>"));
            return Err(classify_upstream_error(
                status,
                &body_text,
                Some(provider.as_str()),
            ));
        }

        let body: Value = response
            .json()
            .await
            .map_err(|e| classify_network_error(&e, Some(provider.as_str())))?;

        let canonical = match payload.adapter.as_str() {
            "anthropic_compatible" if anthropic_messages_upstream::owns_payload(payload) => {
                anthropic_messages_upstream::unpack_response(&body)
                    .or_else(|_| accio::unpack_accio_response(&body))
            }
            "bedrock_converse_compatible" => bedrock_converse_common::unpack_response(&body)
                .or_else(|_| openai::unpack_openai_response(&body))
                .or_else(|_| responses::unpack_responses_response(&body)),
            "cohere_compatible" => cohere_chat_common::unpack_response(&body)
                .or_else(|_| openai::unpack_openai_response(&body))
                .or_else(|_| responses::unpack_responses_response(&body)),
            _ if req.endpoint_kind == EndpointKind::Responses => {
                responses::unpack_responses_response(&body)
                    .or_else(|_| openai::unpack_openai_response(&body))
                    .or_else(|_| accio::unpack_accio_response(&body))
            }
            _ => openai::unpack_openai_response(&body)
                .or_else(|_| responses::unpack_responses_response(&body))
                .or_else(|_| accio::unpack_accio_response(&body)),
        }?;

        Ok(canonical)
    }

    /// Execute a non-streaming upstream request and return the raw JSON body.
    ///
    /// Used for non-chat endpoints such as search-provider passthrough APIs whose response shape
    /// should be forwarded back to the caller without repacking into the
    /// canonical chat-completions schema.
    pub async fn execute_json_passthrough(
        &self,
        provider_account_id: &str,
        execution_mode: ProviderExecutionMode,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        if payload.adapter == "gemini_business_compatible" {
            return self
                .execute_gemini_business_images(payload, req, model, extra_headers)
                .await;
        }
        if payload.adapter == "chataibot_compatible" {
            return self
                .execute_chataibot_images(payload, req, model, extra_headers)
                .await;
        }
        if payload.adapter == "lumalabs_compatible" {
            return self
                .execute_lumalabs_media(provider_account_id, payload, req, model, extra_headers)
                .await;
        }
        if payload.adapter == "gemini_canvas_compatible" {
            return self
                .execute_gemini_canvas_media(
                    provider_account_id,
                    payload,
                    req,
                    model,
                    extra_headers,
                )
                .await;
        }
        if let Some(legacy_route) = gemini_web_reverse_modular::legacy_mixed_lane_execution_route(
            payload,
            req.endpoint_kind,
            false,
        ) {
            if legacy_route.kind == gemini_web_reverse_modular::LegacyMixedLaneExecutionKind::Media
            {
                return gemini_web_reverse_modular::execute_legacy_media(
                    self,
                    provider_account_id,
                    &legacy_route.payload,
                    req,
                    model,
                    extra_headers,
                )
                .await;
            }
        }
        if payload.adapter == "gemini_canvas_web_reverse_compatible" {
            return self
                .execute_gemini_canvas_modular_browser_relay_media(
                    provider_account_id,
                    payload,
                    req,
                    model,
                    extra_headers,
                )
                .await;
        }
        if gemini_api_modular::is_official_adapter(payload.adapter.as_str())
            && gemini_api_modular::supports_media_endpoint(req.endpoint_kind)
            && req.endpoint_kind != EndpointKind::AudioSpeech
        {
            return gemini_api_modular::execute_official_media(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
                extra_headers,
            )
            .await;
        }
        if payload.adapter == "gemini_canvas_program_web_reverse_compatible" {
            let program_owned_payload =
                gemini_canvas_program_web_reverse_modular::force_program_owned_payload(payload);
            return self
                .execute_gemini_canvas_modular_browser_relay_media(
                    provider_account_id,
                    &program_owned_payload,
                    req,
                    model,
                    extra_headers,
                )
                .await;
        }
        if aistudio_web_reverse_modular::is_aistudio_web_reverse_adapter(&payload.adapter) {
            return match aistudio_web_reverse_modular::json_passthrough_route(req.endpoint_kind) {
                Ok(aistudio_web_reverse_modular::JsonPassthroughRoute::Embeddings) => {
                    self.execute_aistudio_web_embeddings(
                        provider_account_id,
                        payload,
                        req,
                        model,
                        extra_headers,
                    )
                    .await
                }
                Ok(aistudio_web_reverse_modular::JsonPassthroughRoute::ImagesGenerations) => {
                    self.execute_aistudio_web_images(
                        provider_account_id,
                        payload,
                        req,
                        model,
                        extra_headers,
                    )
                    .await
                }
                Err(err) => Err(err),
            };
        }
        if payload.adapter == "producer_compatible" {
            return if execution_mode == ProviderExecutionMode::BrowserBacked {
                self.execute_producer_browser_backed(
                    provider_account_id,
                    payload,
                    req,
                    model,
                    extra_headers,
                )
                .await
            } else {
                self.execute_producer_media(payload, req, model, extra_headers)
                    .await
            };
        }
        if payload.adapter == "suno_compatible" {
            return if execution_mode == ProviderExecutionMode::BrowserBacked {
                self.execute_suno_browser_backed(
                    provider_account_id,
                    payload,
                    req,
                    model,
                    extra_headers,
                )
                .await
            } else {
                self.execute_suno_media(payload, req, model, extra_headers)
                    .await
            };
        }
        if payload.adapter == "udio_compatible" {
            return self
                .execute_udio_media(provider_account_id, payload, req, model, extra_headers)
                .await;
        }

        let plan = Self::build_request_plan(payload, req, model, false)?;
        let headers = build_upstream_headers_with(payload, extra_headers);
        let provider = &payload.adapter;

        debug!(url = %plan.url, model, "sending upstream passthrough request");

        let response = self
            .send_plan(&plan, headers)
            .send()
            .await
            .map_err(|e| classify_network_error(&e, Some(provider)))?;

        let status = response.status().as_u16();

        if !response.status().is_success() {
            let body_text = response
                .text()
                .await
                .unwrap_or_else(|_| String::from("<unreadable body>"));
            return Err(classify_upstream_error(status, &body_text, Some(provider)));
        }

        response
            .json()
            .await
            .map_err(|e| classify_network_error(&e, Some(provider)))
    }

    pub async fn execute_binary_passthrough(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<BinaryUpstreamResponse, GatewayError> {
        self.execute_binary_passthrough_with_provider_account_id(
            "",
            payload,
            req,
            model,
            extra_headers,
        )
        .await
    }

    pub async fn execute_binary_passthrough_with_provider_account_id(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<BinaryUpstreamResponse, GatewayError> {
        if gemini_api_modular::is_official_adapter(payload.adapter.as_str())
            && req.endpoint_kind == EndpointKind::AudioSpeech
        {
            return gemini_api_modular::execute_official_tts(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
                extra_headers,
            )
            .await;
        }
        if payload.adapter == "gemini_canvas_compatible"
            && req.endpoint_kind == EndpointKind::AudioSpeech
        {
            return self
                .execute_gemini_canvas_tts(payload, req, model, extra_headers)
                .await;
        }
        if aistudio_web_reverse_modular::is_aistudio_web_reverse_adapter(&payload.adapter)
            && aistudio_web_reverse_modular::supports_binary_passthrough_endpoint(req.endpoint_kind)
        {
            return self
                .execute_aistudio_web_tts(provider_account_id, payload, req, model, extra_headers)
                .await;
        }
        if let Some(legacy_route) = gemini_web_reverse_modular::legacy_mixed_lane_execution_route(
            payload,
            req.endpoint_kind,
            false,
        ) {
            if legacy_route.kind == gemini_web_reverse_modular::LegacyMixedLaneExecutionKind::Tts {
                return gemini_web_reverse_modular::execute_legacy_tts(
                    self,
                    &legacy_route.payload,
                    req,
                    model,
                    extra_headers,
                )
                .await;
            }
        }
        if payload.adapter == "gemini_canvas_web_reverse_compatible"
            && req.endpoint_kind == EndpointKind::AudioSpeech
        {
            return self
                .execute_gemini_canvas_modular_browser_relay_tts(payload, req, model, extra_headers)
                .await;
        }
        if payload.adapter == "gemini_canvas_program_web_reverse_compatible"
            && req.endpoint_kind == EndpointKind::AudioSpeech
        {
            let program_owned_payload =
                gemini_canvas_program_web_reverse_modular::force_program_owned_payload(payload);
            return self
                .execute_gemini_canvas_tts(&program_owned_payload, req, model, extra_headers)
                .await;
        }

        let plan = Self::build_request_plan(payload, req, model, false)?;
        let headers = build_upstream_headers_with(payload, extra_headers);
        let provider = &payload.adapter;

        debug!(url = %plan.url, model, "sending upstream binary passthrough request");

        let response = self
            .send_plan(&plan, headers)
            .send()
            .await
            .map_err(|e| classify_network_error(&e, Some(provider)))?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            let body_text = response
                .text()
                .await
                .unwrap_or_else(|_| String::from("<unreadable body>"));
            return Err(classify_upstream_error(status, &body_text, Some(provider)));
        }

        let content_type = response
            .headers()
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let mut extra_headers_out = Vec::new();
        if let Some(content_disposition) = response
            .headers()
            .get(rquest::header::CONTENT_DISPOSITION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
        {
            extra_headers_out.push(("content-disposition".to_string(), content_disposition));
        }

        let body = response
            .bytes()
            .await
            .map_err(|e| classify_network_error(&e, Some(provider)))?;

        Ok(BinaryUpstreamResponse {
            body,
            content_type,
            extra_headers: extra_headers_out,
        })
    }

    pub(crate) async fn execute_remote_browser_executor(
        &self,
        provider: &str,
        provider_account_id: &str,
        endpoint_kind: EndpointKind,
        input: Value,
    ) -> Result<Option<Value>, GatewayError> {
        let Some(base_url) = self.browser_executor_base_url.as_deref() else {
            if self.request_time_browser_policy == RequestTimeBrowserPolicy::RemoteOnly {
                return Err(remote_browser_executor_required_unavailable_error(
                    "Remote browser executor is required by GATEWAY_REQUEST_TIME_BROWSER_POLICY=remote_only, but GATEWAY_BROWSER_EXECUTOR_BASE_URL is not configured.",
                ));
            }
            if self.request_time_browser_policy == RequestTimeBrowserPolicy::Disabled {
                return Err(request_time_browser_forbidden_error(
                    "Request-time local browser execution is disabled by GATEWAY_REQUEST_TIME_BROWSER_POLICY=disabled, and GATEWAY_BROWSER_EXECUTOR_BASE_URL is not configured.",
                ));
            }
            return Ok(None);
        };

        let endpoint_kind_key = browser_executor_endpoint_kind_key(endpoint_kind);
        let remote_executor_timeout = browser_executor_remote_request_timeout(self.timeout, &input);
        let request = BrowserExecutorInvocationRequest {
            provider,
            provider_account_id,
            endpoint_kind: endpoint_kind_key,
            execution_mode: "browser_backed",
            input,
        };
        let url = format!("{base_url}/v1/internal/browser-executor/execute");
        let mut builder = self
            .http
            .request(Method::POST, &url)
            .header(rquest::header::CONTENT_TYPE, "application/json")
            .timeout(remote_executor_timeout)
            .json(&request);
        if let Some(token) = self.browser_executor_bearer_token.as_deref() {
            builder = builder.bearer_auth(token);
        }

        let response = match builder.send().await {
            Ok(response) => response,
            Err(error) => {
                debug!(
                    provider,
                    provider_account_id,
                    endpoint_kind = endpoint_kind_key,
                    error = %error,
                    "remote browser executor unavailable; falling back to local worker"
                );
                if self.request_time_browser_policy.forbids_local_fallback() {
                    return Err(remote_browser_executor_required_unavailable_error(format!(
                        "Remote browser executor is required because request-time local browser fallback is disabled, but {url} is unavailable: {error}"
                    )));
                }
                return Ok(None);
            }
        };

        let status = response.status().as_u16();
        let body_text = response.text().await.unwrap_or_default();
        if !(200..300).contains(&status) {
            debug!(
                provider,
                provider_account_id,
                endpoint_kind = endpoint_kind_key,
                status,
                body = %body_text,
                "remote browser executor returned non-success status; falling back to local worker"
            );
            if self.request_time_browser_policy.forbids_local_fallback() {
                return Err(remote_browser_executor_required_failed_error(format!(
                    "Remote browser executor is required because request-time local browser fallback is disabled, but {url} returned status {status}: {body_text}"
                )));
            }
            return Ok(None);
        }

        let result = match parse_browser_executor_invocation_response_body(&body_text) {
            Ok(result) => result,
            Err(error) => {
                debug!(
                    provider,
                    provider_account_id,
                    endpoint_kind = endpoint_kind_key,
                    error = %error,
                    body = %body_text,
                    "remote browser executor returned invalid JSON; falling back to local worker"
                );
                if self.request_time_browser_policy.forbids_local_fallback() {
                    return Err(remote_browser_executor_required_failed_error(format!(
                        "Remote browser executor is required because request-time local browser fallback is disabled, but {url} returned invalid JSON: {error}"
                    )));
                }
                return Ok(None);
            }
        };

        if result.ok {
            return Ok(extract_browser_executor_invocation_success(result));
        }

        Err(classify_browser_executor_invocation_failure(
            result, provider,
        ))
    }

    pub async fn execute_browser_executor_service_invocation(
        &self,
        request: BrowserExecutorServiceInvocationRequest,
    ) -> BrowserExecutorServiceInvocationResponse {
        let provider_name = request.provider.trim().to_lowercase();
        match self
            .execute_browser_executor_service_invocation_inner(request)
            .await
        {
            Ok(result) => {
                build_browser_executor_service_invocation_success_response(&provider_name, result)
            }
            Err(error) => {
                build_browser_executor_service_invocation_failure_response(&provider_name, error)
            }
        }
    }

    async fn execute_browser_executor_service_invocation_inner(
        &self,
        request: BrowserExecutorServiceInvocationRequest,
    ) -> Result<Value, GatewayError> {
        let provider_label = request.provider.clone();
        let provider = provider_label.trim().to_lowercase();
        let input = request.input;

        match provider.as_str() {
            "lumalabs" => {
                self.execute_lumalabs_browser_executor_service_invocation(&input)
                    .await
            }
            "producer" => {
                let prepared = prepare_producer_browser_executor_service_input(&input)?;
                execute_producer_browser_worker("producer_compatible", &prepared, false).await
            }
            "suno" => {
                self.execute_suno_browser_executor_service_invocation(&input)
                    .await
            }
            "udio" => {
                self.execute_udio_browser_executor_service_invocation(&input)
                    .await
            }
            "gemini_canvas" => {
                let prepared = gemini_canvas_web_reverse_modular::
                    prepare_gemini_canvas_browser_executor_service_input(&input)?;
                let browser_pool_base_url = self
                    .ensure_gemini_canvas_browser_pool("gemini_canvas_compatible")
                    .await?;
                let result = self
                    .execute_gemini_canvas_browser_request(
                        "gemini_canvas_compatible",
                        &browser_pool_base_url,
                        &prepared.base_url,
                        &prepared.share_id,
                        &prepared.runtime_state_object_key,
                        prepared.browser_cdp_url.as_deref(),
                        prepared.cookie_header.as_deref(),
                        &prepared.operation,
                        &prepared.prompt,
                        &prepared.locale,
                        prepared.timeout,
                    )
                    .await?;
                Ok(
                    gemini_canvas_web_reverse_modular::
                        build_gemini_canvas_browser_executor_service_result(&result),
                )
            }
            _ => Err(unsupported_browser_executor_provider_error(&provider_label)),
        }
    }

    async fn execute_gemini_business_images(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        match req.endpoint_kind {
            EndpointKind::ImagesGenerations | EndpointKind::ImagesEdits => {}
            _ => return Err(gemini_business_unsupported_images_endpoint_error()),
        }

        let provider = "gemini_business_compatible";
        let headers = build_upstream_headers_with(payload, extra_headers);
        let runtime = gemini_business::runtime_from_payload(payload)?;
        let prompt = gemini_business::prompt_from_request(req)?;
        let uploads = gemini_business::extract_uploads_from_request_body(&req.raw_body)?;
        let mut uploaded_file_ids = Vec::with_capacity(uploads.len());

        for upload in uploads {
            let upload_plan = gemini_business::build_context_file_upload_plan(
                payload,
                &runtime,
                &upload,
                req.endpoint_kind,
            );

            let response = self
                .send_plan(&upload_plan, headers.clone())
                .send()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;

            let status = response.status().as_u16();
            if !response.status().is_success() {
                let body_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| String::from("<unreadable body>"));
                return Err(classify_upstream_error(status, &body_text, Some(provider)));
            }

            let upload_json: Value = response
                .json()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;

            let file_id = extract_gemini_business_upload_file_id(&upload_json, provider)?;

            uploaded_file_ids.push(file_id.to_string());
        }

        let assist_plan = gemini_business::build_stream_assist_plan(
            payload,
            req,
            model,
            &runtime,
            &uploaded_file_ids,
        )?;

        let response = self
            .send_plan(&assist_plan, headers.clone())
            .send()
            .await
            .map_err(|e| classify_network_error(&e, Some(provider)))?;

        let status = response.status().as_u16();
        let body_text = response
            .text()
            .await
            .map_err(|e| classify_network_error(&e, Some(provider)))?;

        if !(200..300).contains(&status) {
            return Err(classify_upstream_error(status, &body_text, Some(provider)));
        }

        let response_objects = parse_gemini_business_stream_response_objects(&body_text, provider)?;

        let (session_name, generated_files) =
            gemini_business::extract_generated_files(&response_objects, &runtime.session)?;

        let mut images = Vec::with_capacity(generated_files.len());
        for generated in generated_files {
            let download_url = format!(
                "{}/{}:downloadFile?fileId={}&alt=media",
                payload.base_url.trim_end_matches('/'),
                session_name,
                generated.file_id
            );

            let response = self
                .http
                .request(Method::GET, &download_url)
                .headers(headers.clone())
                .timeout(self.timeout)
                .send()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;

            let status = response.status().as_u16();
            if !response.status().is_success() {
                let body_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| String::from("<unreadable body>"));
                return Err(classify_upstream_error(status, &body_text, Some(provider)));
            }

            let bytes = response
                .bytes()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;
            images.push((generated.mime_type, bytes.to_vec()));
        }

        gemini_business::build_openai_images_response(req, &prompt, &images)
    }

    async fn execute_gemini_canvas_media_direct_http(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        prompt: String,
        timeout: Duration,
    ) -> Result<Value, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let runtime = gemini_canvas::runtime_from_payload(payload)?;
        if operation == gemini_canvas::GeminiCanvasMediaOperation::Video {
            if let Some(continuation) = gemini_canvas_video_continuation_from_request(req)? {
                return self
                    .execute_gemini_canvas_video_continuation(
                        payload,
                        model,
                        &runtime,
                        &prompt,
                        continuation,
                        timeout,
                    )
                    .await;
            }
        }
        if matches!(
            operation,
            gemini_canvas::GeminiCanvasMediaOperation::Music
                | gemini_canvas::GeminiCanvasMediaOperation::Video
        ) {
            if payload.adapter == "gemini_canvas_program_web_reverse_compatible" {
                let program_context = self
                    .prepare_gemini_canvas_program_app_endpoint_api_context(
                        payload, &runtime, timeout,
                    )
                    .await?;
                return self
                    .execute_gemini_canvas_program_app_endpoint_media_direct_http_with_context(
                        req,
                        model,
                        operation,
                        program_context,
                    )
                    .await;
            } else {
                match self
                    .execute_gemini_canvas_runtime_api_media_direct_http(
                        payload, req, model, operation, timeout,
                    )
                    .await
                {
                    Ok(body) => return Ok(body),
                    Err(error) => {
                        debug!(
                            provider,
                            operation = ?operation,
                            error = %summarize_gateway_error(&error),
                            "gemini canvas runtime api media lane failed; falling back to StreamGenerate direct HTTP"
                        );
                    }
                }
            }
        }
        if operation == gemini_canvas::GeminiCanvasMediaOperation::Image {
            let image_direct_http_timeout = timeout.min(Duration::from_secs(75));
            match self
                .execute_gemini_canvas_direct_http_image(
                    payload,
                    req,
                    model,
                    &runtime,
                    prompt.clone(),
                    image_direct_http_timeout,
                )
                .await
            {
                Ok(body) => return Ok(body),
                Err(error) if should_fallback_gemini_canvas_image_to_browser(&error) => {
                    debug!(
                        provider,
                        error = %summarize_gateway_error(&error),
                        "gemini canvas direct HTTP image lane failed; retrying through browser-backed invocation"
                    );
                    let browser_pool_base_url =
                        self.ensure_gemini_canvas_browser_pool(provider).await?;
                    let locale = gemini_canvas::locale_from_payload(payload);
                    let result = self
                        .execute_gemini_canvas_owned_browser_invocation(
                            payload,
                            provider,
                            &browser_pool_base_url,
                            payload.base_url.trim_end_matches('/'),
                            &runtime,
                            None,
                            "image",
                            &prompt,
                            &locale,
                            timeout,
                        )
                        .await?;
                    return gemini_canvas_web_reverse_modular::build_image_generation_response_from_invocation(
                        &self.http,
                        provider,
                        req,
                        &prompt,
                        &result,
                        timeout,
                    )
                    .await;
                }
                Err(error) => return Err(error),
            }
        }
        let mode_index = gemini_canvas::stream_generate_mode_index(operation);
        let request_started_at = SystemTime::now();
        let body_text = self
            .execute_gemini_canvas_direct_http_stream_generate_body(
                payload, model, &runtime, mode_index, &prompt, timeout, true, None, None,
            )
            .await?;

        match operation {
            gemini_canvas::GeminiCanvasMediaOperation::Music => {
                let followup_result = self
                    .extract_gemini_canvas_media_assets_with_followup(
                        payload,
                        model,
                        &runtime,
                        operation,
                        &prompt,
                        &body_text,
                        request_started_at,
                        timeout,
                        false,
                        None,
                    )
                    .await;
                let (assets, resolved_body_text) = match followup_result {
                    Ok(result) => result,
                    Err(error)
                        if gemini_canvas_music_body_indicates_accepted_progress(&body_text) =>
                    {
                        debug!(
                            provider,
                            error = %summarize_gateway_error(&error),
                            "gemini canvas direct HTTP music follow-up did not expose a final asset, but the primary StreamGenerate body already reached accepted progress"
                        );
                        return Ok(build_gemini_canvas_music_accepted_response_from_body(
                            model,
                            &prompt,
                            gemini_canvas::duration_seconds_from_request(req),
                            &body_text,
                            None,
                            None,
                            None,
                        ));
                    }
                    Err(error) => return Err(error),
                };
                if assets.is_empty()
                    && gemini_canvas_music_body_indicates_accepted_progress(&resolved_body_text)
                {
                    return Ok(build_gemini_canvas_music_accepted_response_from_body(
                        model,
                        &prompt,
                        gemini_canvas::duration_seconds_from_request(req),
                        &resolved_body_text,
                        None,
                        None,
                        None,
                    ));
                }
                let asset = select_preferred_gemini_canvas_music_asset(&assets)
                    .expect("assets is non-empty");
                let asset = if asset.body_base64.is_some() {
                    asset.clone()
                } else {
                    self.materialize_gemini_canvas_direct_http_media_asset(
                        payload,
                        &runtime,
                        &asset.url,
                        Some(asset.kind.as_str()),
                        Some(asset.mime_type.as_str()),
                        timeout,
                    )
                    .await?
                };
                Ok(gemini_canvas::build_music_generation_response(
                    model,
                    &prompt,
                    &asset,
                    Some(&resolved_body_text),
                ))
            }
            gemini_canvas::GeminiCanvasMediaOperation::Video => {
                let (assets, resolved_body_text) = match self
                    .extract_gemini_canvas_media_assets_with_followup(
                        payload,
                        model,
                        &runtime,
                        operation,
                        &prompt,
                        &body_text,
                        request_started_at,
                        timeout,
                        false,
                        None,
                    )
                    .await
                {
                    Ok(value) => value,
                    Err(error)
                        if gemini_canvas::response_indicates_video_generation_pending(
                            &body_text,
                        )
                            || gemini_canvas::response_indicates_video_generation_quota_reached(
                                &body_text,
                            ) =>
                    {
                        debug!(
                            provider,
                            error = %summarize_gateway_error(&error),
                            "gemini canvas direct HTTP video follow-up did not expose a final asset, but the primary StreamGenerate body already reached accepted progress"
                        );
                        return Ok(build_gemini_canvas_video_accepted_response_from_body(
                            model, &prompt, &body_text, None, None, None,
                        ));
                    }
                    Err(error) => return Err(error),
                };
                if assets.is_empty()
                    && (gemini_canvas::response_indicates_video_generation_pending(
                        &resolved_body_text,
                    ) || gemini_canvas::response_indicates_video_generation_quota_reached(
                        &resolved_body_text,
                    ))
                {
                    return Ok(build_gemini_canvas_video_accepted_response_from_body(
                        model,
                        &prompt,
                        &resolved_body_text,
                        None,
                        None,
                        None,
                    ));
                }
                let asset = assets.first().ok_or_else(|| {
                    build_gemini_canvas_direct_http_video_missing_asset_error(provider)
                })?;
                if gemini_canvas::video_body_indicates_music_modality_mismatch(&resolved_body_text)
                {
                    return Err(gemini_canvas_video_music_modality_mismatch_error(provider));
                }
                let asset = if asset.body_base64.is_some() {
                    asset.clone()
                } else {
                    self.materialize_gemini_canvas_direct_http_media_asset(
                        payload,
                        &runtime,
                        &asset.url,
                        Some(asset.kind.as_str()),
                        Some(asset.mime_type.as_str()),
                        timeout,
                    )
                    .await?
                };
                Ok(gemini_canvas::build_video_generation_response(
                    model,
                    &prompt,
                    &asset,
                    Some(&resolved_body_text),
                ))
            }
            gemini_canvas::GeminiCanvasMediaOperation::Image => unreachable!(),
        }
    }

    async fn execute_gemini_canvas_video_continuation(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        continuation: GeminiCanvasVideoContinuation,
        timeout: Duration,
    ) -> Result<Value, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let locator = gemini_canvas::GeminiCanvasStreamGenerateLocator {
            conversation_id: continuation.conversation_id.clone(),
            response_id: continuation.response_id.clone(),
            app_path: continuation.app_path.clone(),
        };
        let seed_body = build_gemini_canvas_video_continuation_seed_body(&continuation);
        let followup_body = match self
            .execute_gemini_canvas_direct_http_media_followup_body(
                payload,
                model,
                runtime,
                gemini_canvas::GeminiCanvasMediaOperation::Video,
                prompt,
                &seed_body,
                Some(locator),
                SystemTime::now(),
                timeout,
                false,
                None,
            )
            .await
        {
            Ok(body) => body,
            Err(error) if should_preserve_gemini_canvas_video_continuation_as_pending(&error) => {
                debug!(
                    provider,
                    error = %summarize_gateway_error(&error),
                    "gemini canvas video continuation remains pending"
                );
                return Ok(build_gemini_canvas_video_accepted_response_from_body(
                    model,
                    prompt,
                    &seed_body,
                    Some(&continuation.conversation_id),
                    Some(&continuation.response_id),
                    Some(&continuation.app_path),
                ));
            }
            Err(error) => return Err(error),
        };

        let assets = gemini_canvas::extract_stream_generate_media_assets(
            &followup_body,
            gemini_canvas::GeminiCanvasMediaOperation::Video,
        )
        .or_else(|_| {
            gemini_canvas::extract_page_blob_media_assets(
                &followup_body,
                gemini_canvas::GeminiCanvasMediaOperation::Video,
            )
        });
        let assets = match assets {
            Ok(assets) if !assets.is_empty() => assets,
            _ if gemini_canvas::response_indicates_video_generation_pending(&followup_body)
                || gemini_canvas::response_indicates_video_generation_quota_reached(
                    &followup_body,
                ) =>
            {
                return Ok(build_gemini_canvas_video_accepted_response_from_body(
                    model,
                    prompt,
                    &followup_body,
                    Some(&continuation.conversation_id),
                    Some(&continuation.response_id),
                    Some(&continuation.app_path),
                ));
            }
            _ => {
                return Err(build_gemini_canvas_direct_http_video_missing_asset_error(
                    provider,
                ));
            }
        };
        if gemini_canvas::video_body_indicates_music_modality_mismatch(&followup_body) {
            return Err(gemini_canvas_video_music_modality_mismatch_error(provider));
        }
        let asset = assets
            .first()
            .expect("non-empty continuation assets were checked above");
        let asset = if asset.body_base64.is_some() {
            asset.clone()
        } else {
            self.materialize_gemini_canvas_direct_http_media_asset(
                payload,
                runtime,
                &asset.url,
                Some(asset.kind.as_str()),
                Some(asset.mime_type.as_str()),
                timeout,
            )
            .await?
        };
        Ok(gemini_canvas::build_video_generation_response(
            model,
            prompt,
            &asset,
            Some(&followup_body),
        ))
    }

    async fn execute_gemini_canvas_runtime_api_media_direct_http(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        timeout: Duration,
    ) -> Result<Value, GatewayError> {
        let runtime = gemini_canvas::runtime_from_payload(payload)?;
        let runtime_api = self
            .prepare_gemini_canvas_runtime_api_payload(payload, &runtime, timeout)
            .await?;
        self.execute_gemini_canvas_official_media("", &runtime_api.payload, req, model, None)
            .await
            .map_err(|error| {
                debug!(
                    provider = "gemini_canvas_compatible",
                    operation = ?operation,
                    error = %summarize_gateway_error(&error),
                    "gemini canvas runtime api media lane returned an error"
                );
                error
            })
    }

    async fn execute_gemini_canvas_program_app_endpoint_media_direct_http_with_context(
        &self,
        req: &CanonicalRelayRequest,
        model: &str,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        program_context: GeminiCanvasProgramAppEndpointApiContext,
    ) -> Result<Value, GatewayError> {
        let app_path = program_context
            .relay_config
            .app_endpoint
            .app_path
            .as_deref()
            .unwrap_or("<none>");
        let canvas_base_url = program_context
            .relay_config
            .app_endpoint
            .canvas_program_url
            .as_deref()
            .or(program_context
                .relay_config
                .app_endpoint
                .page_url
                .as_deref())
            .and_then(origin_from_url)
            .unwrap_or_else(|| "https://gemini.google.com".to_string());
        let invoke_contract =
            gemini_canvas_program_web_reverse_modular::preferred_app_endpoint_invoke_contract(
                operation,
                match operation {
                    gemini_canvas::GeminiCanvasMediaOperation::Music => {
                        "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateMusic"
                    }
                    gemini_canvas::GeminiCanvasMediaOperation::Video => {
                        &program_context.invoke_base_url
                    }
                    gemini_canvas::GeminiCanvasMediaOperation::Image => "",
                },
                (operation == gemini_canvas::GeminiCanvasMediaOperation::Video)
                    .then(|| gemini_canvas::resolve_official_video_model(model))
                    .transpose()?,
                &program_context.relay_config,
            );
        let program_model_override =
            gemini_canvas_program_web_reverse_modular::preferred_app_endpoint_model_name(
                &invoke_contract,
            );
        let runtime_api_result = match operation {
            gemini_canvas::GeminiCanvasMediaOperation::Music => {
                if invoke_contract
                    .asset_url
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_some()
                {
                    self.execute_gemini_canvas_program_preview_no_key_music(
                        req,
                        model,
                        &program_context,
                        &invoke_contract,
                    )
                    .await
                } else if matches!(
                    invoke_contract.request_envelope_kind.as_deref(),
                    Some("page_stream_generate_form")
                ) || matches!(
                    invoke_contract.transport_kind.as_deref(),
                    Some("program_music_streamgenerate_candidate")
                ) {
                    self.execute_gemini_canvas_program_preview_no_key_music(
                        req,
                        model,
                        &program_context,
                        &invoke_contract,
                    )
                    .await
                } else if matches!(
                    invoke_contract.request_envelope_kind.as_deref(),
                    Some("canvas_proxy_request")
                ) || matches!(
                    invoke_contract.transport_kind.as_deref(),
                    Some("canvas_program_ws_candidate")
                ) {
                    let prompt = invoke_contract
                        .prompt
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                        .unwrap_or(
                            gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
                                req,
                                gemini_canvas::GeminiCanvasMediaOperation::Music,
                            )?,
                        );
                    let upstream_model = program_model_override
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                        .unwrap_or_else(|| {
                            gemini_canvas::resolve_music_model(model)
                                .unwrap_or(gemini_canvas::GEMINI_CANVAS_MUSIC_PREVIEW_MODEL)
                                .to_string()
                        });
                    let mut request_body = json!({
                        "setup": {
                            "model": format!("models/{upstream_model}")
                        },
                        "client_content": gemini_canvas::build_music_client_content(&prompt),
                        "playback_control": "PLAY"
                    });
                    let music_generation_config = gemini_canvas::build_music_generation_config(req);
                    if music_generation_config
                        .as_object()
                        .map(|config| !config.is_empty())
                        .unwrap_or(false)
                    {
                        request_body["music_generation_config"] = music_generation_config;
                    }
                    let browser_pool_base_url = self
                        .ensure_gemini_canvas_browser_pool("gemini_canvas_compatible")
                        .await?;
                    let invocation = self
                        .execute_gemini_canvas_connected_fetch_invocation_with_program_context(
                            &program_context.runtime_api.payload,
                            "gemini_canvas_compatible",
                            &browser_pool_base_url,
                            &canvas_base_url,
                            &program_context.relay_config,
                            None,
                            None,
                            "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateMusic",
                            Method::POST,
                            Some(&request_body),
                            "canvas_page_music_no_key",
                            self.timeout.max(Duration::from_secs(60)),
                        )
                        .await?;
                    let audio = decode_gemini_canvas_program_music_no_key_audio(
                        invocation.body_base64.as_deref(),
                        invocation.content_type.as_deref(),
                    )?;
                    let (body, content_type) =
                        gemini_canvas::build_audio_binary_response(req, &audio)?;
                    let asset_body_base64 = base64::engine::general_purpose::STANDARD.encode(&body);
                    let asset = gemini_canvas::GeminiCanvasMediaAsset {
                        kind: "audio".to_string(),
                        url: format!("data:{content_type};base64,{asset_body_base64}"),
                        mime_type: content_type,
                        download_token: None,
                        body_base64: Some(asset_body_base64),
                        alt: Some(prompt.clone()),
                        width: None,
                        height: None,
                        duration_seconds: None,
                    };
                    Ok(gemini_canvas::build_music_generation_response(
                        model, &prompt, &asset, None,
                    ))
                } else {
                    let socket_url = invoke_contract
                        .music_ws_url
                        .as_deref()
                        .ok_or_else(gemini_canvas_program_music_invoke_target_missing_error)?;
                    execute_gemini_canvas_official_music(
                        self.timeout,
                        &program_context.runtime_api.payload,
                        req,
                        model,
                        Some(&program_context.runtime_api),
                        Some(&program_context.official_extra_headers),
                        Some(socket_url),
                        invoke_contract.prompt.as_deref(),
                        invoke_contract.duration_seconds,
                        program_model_override.as_deref(),
                    )
                    .await
                }
            }
            gemini_canvas::GeminiCanvasMediaOperation::Video => {
                if matches!(
                    invoke_contract.request_envelope_kind.as_deref(),
                    Some("canvas_proxy_request") | Some("page_stream_generate_form")
                ) || matches!(
                    invoke_contract.transport_kind.as_deref(),
                    Some("canvas_program_ws_candidate")
                        | Some("program_video_streamgenerate_candidate")
                ) {
                    self.execute_gemini_canvas_program_preview_no_key_video(
                        req,
                        model,
                        &canvas_base_url,
                        &program_context,
                        program_model_override.as_deref(),
                        &invoke_contract,
                    )
                    .await
                } else {
                    let request_url = invoke_contract
                        .video_request_url
                        .as_deref()
                        .ok_or_else(gemini_canvas_program_video_invoke_target_missing_error)?;
                    execute_gemini_canvas_official_video(
                        &self.http,
                        self.timeout,
                        &program_context.runtime_api.payload,
                        req,
                        model,
                        Some(&program_context.official_extra_headers),
                        Some(request_url),
                        invoke_contract.prompt.as_deref(),
                        invoke_contract.aspect_ratio.as_deref(),
                        invoke_contract.duration_seconds,
                        program_model_override.as_deref(),
                    )
                    .await
                }
            }
            gemini_canvas::GeminiCanvasMediaOperation::Image => unreachable!(),
        };
        runtime_api_result.map_err(|error| {
            debug!(
                provider = "gemini_canvas_compatible",
                operation = ?operation,
                app_path,
                error = %summarize_gateway_error(&error),
                "gemini canvas program app-endpoint media lane returned an error"
            );
            error
        })
    }

    async fn execute_gemini_canvas_program_preview_no_key_video(
        &self,
        req: &CanonicalRelayRequest,
        model: &str,
        canvas_base_url: &str,
        program_context: &GeminiCanvasProgramAppEndpointApiContext,
        program_model_override: Option<&str>,
        invoke_contract: &gemini_canvas_program_web_reverse_modular::GeminiCanvasProgramAppInvokeContract,
    ) -> Result<Value, GatewayError> {
        let provider = "gemini_canvas_compatible";
        if gemini_canvas::requested_output_count(req) > 1 {
            return Err(gemini_canvas_video_unsupported_count_error(provider));
        }

        let prompt = invoke_contract
            .prompt
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or(
                gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
                    req,
                    gemini_canvas::GeminiCanvasMediaOperation::Video,
                )?,
            );
        let resolved_aspect_ratio =
            if req.raw_body.get("size").is_some() || req.raw_body.get("aspect_ratio").is_some() {
                gemini_canvas::aspect_ratio_from_request(req)
            } else {
                invoke_contract
                    .aspect_ratio
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| gemini_canvas::aspect_ratio_from_request(req))
            };
        let runtime = gemini_canvas::runtime_from_payload(&program_context.runtime_api.payload)?;
        let uses_page_owned_stream_generate = matches!(
            invoke_contract.request_envelope_kind.as_deref(),
            Some("page_stream_generate_form")
        ) || matches!(
            invoke_contract.transport_kind.as_deref(),
            Some("program_video_streamgenerate_candidate")
        ) || invoke_contract
            .request_url
            .as_deref()
            .map(|value| value.contains("/StreamGenerate"))
            .unwrap_or(false);
        if uses_page_owned_stream_generate {
            let mut template =
                gemini_canvas_program_web_reverse_modular::build_program_stream_generate_request_from_invoke_contract(
                    invoke_contract,
                    Some(&prompt),
                )?
                .ok_or_else(|| {
                    gemini_canvas_program_video_no_key_request_contract_missing_error(provider)
                })?;
            template
                .headers
                .insert("accept".to_string(), "*/*".to_string());
            template.headers.insert(
                "accept-language".to_string(),
                format!(
                    "{},zh;q=0.9,en;q=0.8",
                    gemini_canvas::locale_from_payload(&program_context.runtime_api.payload)
                ),
            );
            template.headers.insert(
                "authorization".to_string(),
                gemini_canvas::build_sapisid_authorization(
                    &program_context.runtime_api.session.sapisid,
                    &program_context.runtime_api.page_origin,
                    current_unix_timestamp_i64(),
                )?,
            );
            template.headers.insert(
                "origin".to_string(),
                program_context.runtime_api.page_origin.clone(),
            );
            template.headers.insert(
                "referer".to_string(),
                program_context.runtime_api.page_referer.clone(),
            );
            template.headers.insert(
                "user-agent".to_string(),
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0".to_string(),
            );
            template.headers.insert(
                "x-goog-authuser".to_string(),
                program_context.runtime_api.session.auth_user.clone(),
            );
            template.headers.insert(
                "x-origin".to_string(),
                program_context.runtime_api.page_origin.clone(),
            );
            template
                .headers
                .insert("x-same-domain".to_string(), "1".to_string());
            template
                .headers
                .insert("sec-fetch-site".to_string(), "same-origin".to_string());
            template
                .headers
                .insert("sec-fetch-mode".to_string(), "cors".to_string());
            template
                .headers
                .insert("sec-fetch-dest".to_string(), "empty".to_string());
            template.headers.insert(
                "sec-ch-ua".to_string(),
                "\"Microsoft Edge\";v=\"143\", \"Chromium\";v=\"143\", \"Not_A Brand\";v=\"24\""
                    .to_string(),
            );
            template
                .headers
                .insert("sec-ch-ua-mobile".to_string(), "?0".to_string());
            template
                .headers
                .insert("sec-ch-ua-platform".to_string(), "\"Windows\"".to_string());

            let replay_result = self
                .execute_gemini_canvas_http_replay_worker(
                    provider,
                    &template,
                    &program_context.runtime_api.session,
                    Some("video"),
                    self.timeout.max(Duration::from_secs(45)),
                )
                .await?;
            let body_text = replay_result.body_text;
            let locator_hint = gemini_canvas::extract_stream_generate_locator(&body_text).ok();
            let conversation_id_hint = locator_hint
                .as_ref()
                .map(|locator| locator.conversation_id.as_str())
                .or(program_context
                    .relay_config
                    .app_endpoint
                    .conversation_id
                    .as_deref());
            let response_id_hint = locator_hint
                .as_ref()
                .map(|locator| locator.response_id.as_str())
                .or(program_context
                    .relay_config
                    .app_endpoint
                    .response_id
                    .as_deref());
            let app_path_hint = locator_hint
                .as_ref()
                .map(|locator| locator.app_path.as_str())
                .or(program_context
                    .relay_config
                    .app_endpoint
                    .app_path
                    .as_deref());
            let video_pending_hint = body_text.contains("video_placeholder")
                || gemini_canvas::response_indicates_video_generation_pending(&body_text);
            let busy_hint = gemini_canvas_body_indicates_context_busy(&body_text);
            let job_id_hint = gemini_canvas::extract_video_generation_job_id(&body_text);

            let request_started_at = SystemTime::now();
            let followup_result = self
                .extract_gemini_canvas_media_assets_with_followup(
                    &program_context.runtime_api.payload,
                    model,
                    &runtime,
                    gemini_canvas::GeminiCanvasMediaOperation::Video,
                    &prompt,
                    &body_text,
                    request_started_at,
                    self.timeout.max(Duration::from_secs(120)),
                    false,
                    None,
                )
                .await;
            let (assets, resolved_body_text) = match followup_result {
                Ok(result) => result,
                Err(error) if video_pending_hint || busy_hint => {
                    debug!(
                        provider,
                        app_path = app_path_hint.unwrap_or("<none>"),
                        conversation_id = conversation_id_hint.unwrap_or("<none>"),
                        response_id = response_id_hint.unwrap_or("<none>"),
                        error = %summarize_gateway_error(&error),
                        "gemini canvas page-owned video StreamGenerate reached pending/busy state before exposing a real asset; returning accepted response"
                    );
                    return Ok(gemini_canvas::build_video_generation_accepted_response(
                        model,
                        &prompt,
                        conversation_id_hint,
                        response_id_hint,
                        app_path_hint,
                        job_id_hint.as_deref(),
                        Some(&body_text),
                    ));
                }
                Err(error) => return Err(error),
            };
            let asset = assets.first().ok_or_else(|| {
                build_gemini_canvas_direct_http_video_missing_asset_error(provider)
            })?;
            if gemini_canvas::video_body_indicates_music_modality_mismatch(&resolved_body_text) {
                return Err(gemini_canvas_video_music_modality_mismatch_error(provider));
            }
            if (video_pending_hint || busy_hint)
                && !gemini_canvas_video_body_has_usable_asset(&resolved_body_text)
            {
                return Ok(gemini_canvas::build_video_generation_accepted_response(
                    model,
                    &prompt,
                    conversation_id_hint,
                    response_id_hint,
                    app_path_hint,
                    job_id_hint.as_deref(),
                    Some(&resolved_body_text),
                ));
            }
            return Ok(gemini_canvas::build_video_generation_response(
                model,
                &prompt,
                asset,
                Some(&resolved_body_text),
            ));
        }

        let mut parameters = serde_json::Map::new();
        parameters.insert("aspectRatio".to_string(), json!(resolved_aspect_ratio));
        if let Some(duration_seconds) = invoke_contract.duration_seconds {
            if req.raw_body.get("duration_s").is_none()
                && req.raw_body.get("durationSeconds").is_none()
                && req.raw_body.get("duration").is_none()
            {
                parameters.insert("durationSeconds".to_string(), json!(duration_seconds));
            }
        }
        let request_body = json!({
            "instances": [{
                "prompt": prompt
            }],
            "parameters": parameters
        });

        let browser_pool_base_url = self.ensure_gemini_canvas_browser_pool(provider).await?;
        let preview_model = gemini_canvas::resolve_video_model(model)?;
        let official_model = program_model_override
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| {
                gemini_canvas::resolve_official_video_model(model)
                    .unwrap_or(gemini_canvas::GEMINI_CANVAS_OFFICIAL_VIDEO_MODEL)
                    .to_string()
            });
        let candidate_request_urls = vec![
            format!(
                "{}/models/{}:predictLongRunning",
                gemini_canvas::GEMINI_CANVAS_DIRECT_HTTP_IMAGE_API_BASE_URL.trim_end_matches('/'),
                preview_model
            ),
            format!(
                "{}/models/{}:predictLongRunning",
                program_context
                    .runtime_api
                    .payload
                    .base_url
                    .trim_end_matches('/'),
                official_model
            ),
        ];

        let mut operation_value: Option<Value> = None;
        let mut operation_base_url: Option<String> = None;
        let mut last_error: Option<GatewayError> = None;

        for request_url in candidate_request_urls {
            match self
                .execute_gemini_canvas_connected_fetch_invocation_with_program_context(
                    &program_context.runtime_api.payload,
                    provider,
                    &browser_pool_base_url,
                    canvas_base_url,
                    &program_context.relay_config,
                    None,
                    None,
                    &request_url,
                    Method::POST,
                    Some(&request_body),
                    "canvas_page_no_key",
                    self.timeout.max(Duration::from_secs(30)),
                )
                .await
            {
                Ok(invocation) => {
                    let body = invocation.body_text.as_deref().ok_or_else(|| {
                        gemini_canvas_program_video_no_key_empty_body_error(provider)
                    })?;
                    let body_json: Value = serde_json::from_str(body).map_err(|error| {
                        gemini_canvas_program_video_no_key_invalid_json_error(
                            provider,
                            error.to_string().as_str(),
                        )
                    })?;
                    if (200..300).contains(&invocation.status) {
                        operation_base_url = request_url
                            .split_once("/models/")
                            .map(|(prefix, _)| prefix.to_string())
                            .or_else(|| {
                                origin_from_url(&request_url)
                                    .map(|origin| format!("{origin}/v1beta"))
                            });
                        operation_value = Some(body_json);
                        break;
                    }
                    last_error = Some(classify_upstream_error(
                        invocation.status,
                        body,
                        Some(provider),
                    ));
                }
                Err(error) => {
                    last_error = Some(error);
                }
            }
        }

        let operation_value = match operation_value {
            Some(value) => value,
            None => {
                return Err(last_error.unwrap_or_else(|| {
                    gemini_canvas_program_video_no_key_request_exhausted_error(provider)
                }));
            }
        };
        let operation_name = operation_value
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| gemini_canvas_video_missing_operation_error(provider))?;
        let operation_url =
            if operation_name.starts_with("http://") || operation_name.starts_with("https://") {
                operation_name.to_string()
            } else {
                format!(
                    "{}/{}",
                    operation_base_url.unwrap_or_else(|| {
                        program_context
                            .runtime_api
                            .payload
                            .base_url
                            .trim_end_matches('/')
                            .to_string()
                    }),
                    operation_name.trim_start_matches('/')
                )
            };

        let deadline = std::time::Instant::now() + self.timeout.max(Duration::from_secs(600));
        let mut poll = operation_value.clone();
        while std::time::Instant::now() < deadline {
            if poll.get("done").and_then(Value::as_bool).unwrap_or(false) {
                break;
            }
            sleep(Duration::from_secs(5)).await;
            let invocation = self
                .execute_gemini_canvas_connected_fetch_invocation_with_program_context(
                    &program_context.runtime_api.payload,
                    provider,
                    &browser_pool_base_url,
                    canvas_base_url,
                    &program_context.relay_config,
                    None,
                    None,
                    &operation_url,
                    Method::GET,
                    None,
                    "canvas_page_no_key",
                    self.timeout.max(Duration::from_secs(30)),
                )
                .await?;
            let body = invocation.body_text.as_deref().ok_or_else(|| {
                gemini_canvas_program_video_no_key_poll_empty_body_error(provider)
            })?;
            if !(200..300).contains(&invocation.status) {
                return Err(classify_upstream_error(
                    invocation.status,
                    body,
                    Some(provider),
                ));
            }
            poll = serde_json::from_str(body).map_err(|error| {
                gemini_canvas_program_video_no_key_poll_invalid_json_error(
                    provider,
                    error.to_string().as_str(),
                )
            })?;
        }
        if !poll.get("done").and_then(Value::as_bool).unwrap_or(false) {
            return Err(gemini_canvas_video_operation_timeout_error(provider));
        }
        if let Some(error) = poll.get("error") {
            return Err(classify_upstream_error(
                502,
                &error.to_string(),
                Some(provider),
            ));
        }
        let video_uri = extract_gemini_canvas_video_operation_download_uri(provider, &poll)?;
        let (bytes, content_type) = self
            .execute_gemini_canvas_connected_fetch_get_bytes_with_program_context(
                &program_context.runtime_api.payload,
                provider,
                &browser_pool_base_url,
                canvas_base_url,
                &program_context.relay_config,
                None,
                None,
                video_uri,
                "canvas_page_no_key",
                self.timeout.max(Duration::from_secs(120)),
            )
            .await?;
        let mime_type = content_type.unwrap_or_else(|| "video/mp4".to_string());
        let body_base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let asset = gemini_canvas::GeminiCanvasMediaAsset {
            kind: "video".to_string(),
            url: video_uri.to_string(),
            mime_type,
            download_token: None,
            body_base64: Some(body_base64),
            alt: Some(prompt.clone()),
            width: None,
            height: None,
            duration_seconds: None,
        };
        Ok(gemini_canvas::build_video_generation_response(
            model, &prompt, &asset, None,
        ))
    }

    async fn execute_gemini_canvas_program_preview_no_key_music(
        &self,
        req: &CanonicalRelayRequest,
        model: &str,
        program_context: &GeminiCanvasProgramAppEndpointApiContext,
        invoke_contract: &gemini_canvas_program_web_reverse_modular::GeminiCanvasProgramAppInvokeContract,
    ) -> Result<Value, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let timeout = self.timeout.max(Duration::from_secs(45));
        let request_started_at = SystemTime::now();
        let prompt = invoke_contract
            .prompt
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or(
                gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
                    req,
                    gemini_canvas::GeminiCanvasMediaOperation::Music,
                )?,
            );
        let runtime = gemini_canvas::runtime_from_payload(&program_context.runtime_api.payload)?;
        if let Some(asset_url) = invoke_contract
            .asset_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            match self
                .materialize_gemini_canvas_direct_http_media_asset(
                    &program_context.runtime_api.payload,
                    &runtime,
                    asset_url,
                    invoke_contract.asset_kind.as_deref(),
                    invoke_contract.asset_mime_type.as_deref(),
                    timeout,
                )
                .await
            {
                Ok(asset) => {
                    return Ok(gemini_canvas::build_music_generation_response(
                        model,
                        &prompt,
                        &asset,
                        Some("music_player_ready"),
                    ));
                }
                Err(error) => {
                    debug!(
                        provider,
                        asset_url,
                        error = %summarize_gateway_error(&error),
                        "gemini canvas no-key music invoke contract exposed a final asset target, but direct materialization failed; continuing browserless StreamGenerate replay path"
                    );
                }
            }
        }
        let mut template =
            gemini_canvas_program_web_reverse_modular::build_program_stream_generate_request_from_invoke_contract(
                invoke_contract,
                Some(&prompt),
            )?
            .ok_or_else(|| {
                gemini_canvas_program_music_no_key_request_contract_missing_error(provider)
            })?;
        template
            .headers
            .insert("accept".to_string(), "*/*".to_string());
        template.headers.insert(
            "content-type".to_string(),
            "application/x-www-form-urlencoded;charset=UTF-8".to_string(),
        );
        template.headers.insert(
            "accept-language".to_string(),
            format!(
                "{},zh;q=0.9,en;q=0.8",
                gemini_canvas::locale_from_payload(&program_context.runtime_api.payload)
            ),
        );
        template.headers.insert(
            "authorization".to_string(),
            gemini_canvas::build_sapisid_authorization(
                &program_context.runtime_api.session.sapisid,
                &program_context.runtime_api.page_origin,
                current_unix_timestamp_i64(),
            )?,
        );
        template.headers.insert(
            "origin".to_string(),
            program_context.runtime_api.page_origin.clone(),
        );
        template.headers.insert(
            "referer".to_string(),
            program_context.runtime_api.page_referer.clone(),
        );
        template.headers.insert(
            "user-agent".to_string(),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0".to_string(),
        );
        template.headers.insert(
            "x-goog-authuser".to_string(),
            program_context.runtime_api.session.auth_user.clone(),
        );
        template.headers.insert(
            "x-origin".to_string(),
            program_context.runtime_api.page_origin.clone(),
        );
        template
            .headers
            .insert("x-same-domain".to_string(), "1".to_string());
        template
            .headers
            .insert("sec-fetch-site".to_string(), "same-origin".to_string());
        template
            .headers
            .insert("sec-fetch-mode".to_string(), "cors".to_string());
        template
            .headers
            .insert("sec-fetch-dest".to_string(), "empty".to_string());
        template.headers.insert(
            "sec-ch-ua".to_string(),
            "\"Microsoft Edge\";v=\"143\", \"Chromium\";v=\"143\", \"Not_A Brand\";v=\"24\""
                .to_string(),
        );
        template
            .headers
            .insert("sec-ch-ua-mobile".to_string(), "?0".to_string());
        template
            .headers
            .insert("sec-ch-ua-platform".to_string(), "\"Windows\"".to_string());
        let session_cookie_hash = {
            let digest = <sha1::Sha1 as sha1::Digest>::digest(
                program_context.runtime_api.session.cookie_header.as_bytes(),
            );
            hex::encode(digest)
        };
        debug!(
            provider,
            cookie_header_len = program_context.runtime_api.session.cookie_header.len(),
            cookie_count = program_context
                .runtime_api
                .session
                .cookie_header
                .split(';')
                .filter(|value| !value.trim().is_empty())
                .count(),
            has_1psid = program_context
                .runtime_api
                .session
                .cookie_header
                .contains("__Secure-1PSID="),
            has_1psidts = program_context
                .runtime_api
                .session
                .cookie_header
                .contains("__Secure-1PSIDTS="),
            has_sapisid = program_context
                .runtime_api
                .session
                .cookie_header
                .contains("SAPISID="),
            auth_user = %program_context.runtime_api.session.auth_user,
            cookie_sha1 = %session_cookie_hash,
            page_referer = %program_context.runtime_api.page_referer,
            request_url = %template.url,
            request_query = ?template.query,
            "gemini canvas no-key music replay session prepared"
        );

        let initial_result = self
            .execute_gemini_canvas_http_replay_worker(
                provider,
                &template,
                &program_context.runtime_api.session,
                Some("music"),
                timeout,
            )
            .await?;
        let initial_body = initial_result.body_text;
        debug!(
            provider,
            body_len = initial_body.len(),
            has_mp3 = initial_body.contains(".mp3"),
            has_audio_mpeg = initial_body.contains("audio/mpeg"),
            has_mp4 = initial_body.contains(".mp4"),
            has_1060 = initial_body.contains("[1060]"),
            has_track_details = initial_body.contains("Track Details"),
            "gemini canvas no-key music initial StreamGenerate replay completed"
        );
        if let Ok((assets, resolved_body)) = self
            .extract_gemini_canvas_media_assets_with_followup(
                &program_context.runtime_api.payload,
                model,
                &runtime,
                gemini_canvas::GeminiCanvasMediaOperation::Music,
                &prompt,
                &initial_body,
                SystemTime::now(),
                timeout,
                false,
                None,
            )
            .await
        {
            if let Some(asset) = select_preferred_gemini_canvas_music_asset(&assets) {
                let asset = self
                    .best_effort_materialize_gemini_canvas_direct_http_media_asset(
                        &program_context.runtime_api.payload,
                        &runtime,
                        asset,
                        timeout,
                        provider,
                    )
                    .await;
                return Ok(gemini_canvas::build_music_generation_response(
                    model,
                    &prompt,
                    &asset,
                    Some(&resolved_body),
                ));
            }
            if let Some(result) = self
                .try_resolve_gemini_canvas_program_preview_no_key_music_followup(
                    &program_context.runtime_api.payload,
                    model,
                    &runtime,
                    &prompt,
                    &resolved_body,
                    request_started_at,
                    timeout,
                    program_context
                        .relay_config
                        .app_endpoint
                        .conversation_id
                        .as_deref(),
                    program_context
                        .relay_config
                        .app_endpoint
                        .response_id
                        .as_deref(),
                    program_context
                        .relay_config
                        .app_endpoint
                        .app_path
                        .as_deref(),
                )
                .await?
            {
                return Ok(result);
            }
            debug!(
                provider,
                asset_count = assets.len(),
                "gemini canvas no-key music initial replay extracted assets but none were selected as final"
            );
        } else {
            let extract_error = gemini_canvas::extract_stream_generate_media_assets(
                &initial_body,
                gemini_canvas::GeminiCanvasMediaOperation::Music,
            )
            .err()
            .map(|error| summarize_gateway_error(&error))
            .unwrap_or_else(|| "<unknown>".to_string());
            debug!(
                provider,
                error = %extract_error,
                "gemini canvas no-key music initial replay did not yield direct music assets"
            );
        }
        if let Some(result) = self
            .try_resolve_gemini_canvas_program_preview_no_key_music_followup(
                &program_context.runtime_api.payload,
                model,
                &runtime,
                &prompt,
                &initial_body,
                request_started_at,
                timeout,
                program_context
                    .relay_config
                    .app_endpoint
                    .conversation_id
                    .as_deref(),
                program_context
                    .relay_config
                    .app_endpoint
                    .response_id
                    .as_deref(),
                program_context
                    .relay_config
                    .app_endpoint
                    .app_path
                    .as_deref(),
            )
            .await?
        {
            return Ok(result);
        }

        let followup_context = self
            .prepare_gemini_canvas_direct_http_media_followup_context(
                &program_context.runtime_api.payload,
                &runtime,
                gemini_canvas::GeminiCanvasMediaOperation::Music,
                &initial_body,
                None,
                timeout,
                false,
                None,
            )
            .await;

        if let Ok(GeminiCanvasMediaFollowupContext {
            bootstrap,
            mut session,
            followup_target,
            ..
        }) = followup_context
        {
            let selected_mode_request = gemini_canvas::build_mode_selection_preflight_request(
                &bootstrap,
                &followup_target.source_path,
                gemini_canvas::GEMINI_CANVAS_TEXT_LAST_SELECTED_MODE_ID,
            )?;
            let model_header = gemini_canvas::build_text_batchexecute_model_header(None, None);
            let _ = self
                .send_gemini_canvas_text_batchexecute_request_refreshing_session(
                    &program_context.runtime_api.payload,
                    model,
                    &selected_mode_request,
                    &mut session,
                    &model_header,
                    timeout,
                )
                .await;

            let replay_after_prelude = self
                .execute_gemini_canvas_http_replay_worker(
                    provider,
                    &template,
                    &session,
                    Some("music"),
                    timeout,
                )
                .await?;
            let replay_after_prelude_body = replay_after_prelude.body_text;
            debug!(
                provider,
                body_len = replay_after_prelude_body.len(),
                has_mp3 = replay_after_prelude_body.contains(".mp3"),
                has_audio_mpeg = replay_after_prelude_body.contains("audio/mpeg"),
                has_mp4 = replay_after_prelude_body.contains(".mp4"),
                has_1060 = replay_after_prelude_body.contains("[1060]"),
                has_track_details = replay_after_prelude_body.contains("Track Details"),
                "gemini canvas no-key music replay after minimal prelude completed"
            );
            if let Ok((assets, resolved_body)) = self
                .extract_gemini_canvas_media_assets_with_followup(
                    &program_context.runtime_api.payload,
                    model,
                    &runtime,
                    gemini_canvas::GeminiCanvasMediaOperation::Music,
                    &prompt,
                    &replay_after_prelude_body,
                    SystemTime::now(),
                    timeout,
                    false,
                    None,
                )
                .await
            {
                if let Some(asset) = select_preferred_gemini_canvas_music_asset(&assets) {
                    let asset = self
                        .best_effort_materialize_gemini_canvas_direct_http_media_asset(
                            &program_context.runtime_api.payload,
                            &runtime,
                            asset,
                            timeout,
                            provider,
                        )
                        .await;
                    return Ok(gemini_canvas::build_music_generation_response(
                        model,
                        &prompt,
                        &asset,
                        Some(&resolved_body),
                    ));
                }
                if let Some(result) = self
                    .try_resolve_gemini_canvas_program_preview_no_key_music_followup(
                        &program_context.runtime_api.payload,
                        model,
                        &runtime,
                        &prompt,
                        &resolved_body,
                        request_started_at,
                        timeout,
                        program_context
                            .relay_config
                            .app_endpoint
                            .conversation_id
                            .as_deref(),
                        program_context
                            .relay_config
                            .app_endpoint
                            .response_id
                            .as_deref(),
                        program_context
                            .relay_config
                            .app_endpoint
                            .app_path
                            .as_deref(),
                    )
                    .await?
                {
                    return Ok(result);
                }
                debug!(
                    provider,
                    asset_count = assets.len(),
                    "gemini canvas no-key music replay after prelude extracted assets but none were selected as final"
                );
            } else {
                let extract_error = gemini_canvas::extract_stream_generate_media_assets(
                    &replay_after_prelude_body,
                    gemini_canvas::GeminiCanvasMediaOperation::Music,
                )
                .err()
                .map(|error| summarize_gateway_error(&error))
                .unwrap_or_else(|| "<unknown>".to_string());
                debug!(
                    provider,
                    error = %extract_error,
                    "gemini canvas no-key music replay after prelude did not yield direct music assets"
                );
            }
            if let Some(result) = self
                .try_resolve_gemini_canvas_program_preview_no_key_music_followup(
                    &program_context.runtime_api.payload,
                    model,
                    &runtime,
                    &prompt,
                    &replay_after_prelude_body,
                    request_started_at,
                    timeout,
                    program_context
                        .relay_config
                        .app_endpoint
                        .conversation_id
                        .as_deref(),
                    program_context
                        .relay_config
                        .app_endpoint
                        .response_id
                        .as_deref(),
                    program_context
                        .relay_config
                        .app_endpoint
                        .app_path
                        .as_deref(),
                )
                .await?
            {
                return Ok(result);
            }
            if replay_after_prelude_body.contains("[1060]") {
                return Err(
                    gemini_canvas_program_music_no_key_prelude_stage_incomplete_error(provider),
                );
            }
            return Err(
                gemini_canvas_program_music_no_key_post_1060_contract_missing_error(provider),
            );
        }

        if initial_body.contains("[1060]") {
            return Err(
                gemini_canvas_program_music_no_key_browserless_stage_incomplete_error(provider),
            );
        }
        Err(gemini_canvas_program_music_no_key_contract_missing_error(
            provider,
        ))
    }

    async fn try_resolve_gemini_canvas_program_preview_no_key_music_followup(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        seed_body: &str,
        request_started_at: SystemTime,
        timeout: Duration,
        fallback_conversation_id: Option<&str>,
        fallback_response_id: Option<&str>,
        fallback_app_path: Option<&str>,
    ) -> Result<Option<Value>, GatewayError> {
        if !gemini_canvas_music_body_indicates_accepted_progress(seed_body) {
            return Ok(None);
        }
        let provider = "gemini_canvas_compatible";
        let mut page_payload = payload.clone();
        page_payload.base_url = gemini_canvas_page_base_url(payload);
        let followup_body = match self
            .execute_gemini_canvas_direct_http_media_followup_body(
                &page_payload,
                model,
                runtime,
                gemini_canvas::GeminiCanvasMediaOperation::Music,
                prompt,
                seed_body,
                None,
                request_started_at,
                timeout,
                false,
                None,
            )
            .await
        {
            Ok(body) => body,
            Err(error) => {
                debug!(
                    provider,
                    error = %summarize_gateway_error(&error),
                    "gemini canvas program-owned music accepted progress follow-up did not recover a final asset; preserving pending semantics"
                );
                return Ok(Some(build_gemini_canvas_music_accepted_response_from_body(
                    model,
                    prompt,
                    None,
                    seed_body,
                    fallback_conversation_id,
                    fallback_response_id,
                    fallback_app_path,
                )));
            }
        };
        let extracted_assets = gemini_canvas::extract_stream_generate_media_assets(
            &followup_body,
            gemini_canvas::GeminiCanvasMediaOperation::Music,
        )
        .or_else(|_| {
            gemini_canvas::extract_page_blob_media_assets(
                &followup_body,
                gemini_canvas::GeminiCanvasMediaOperation::Music,
            )
        });
        if let Ok(assets) = extracted_assets {
            if let Some(asset) = select_preferred_gemini_canvas_music_asset(&assets) {
                let asset = self
                    .best_effort_materialize_gemini_canvas_direct_http_media_asset(
                        payload, runtime, asset, timeout, provider,
                    )
                    .await;
                return Ok(Some(gemini_canvas::build_music_generation_response(
                    model,
                    prompt,
                    &asset,
                    Some(&followup_body),
                )));
            }
        }
        Ok(Some(build_gemini_canvas_music_accepted_response_from_body(
            model,
            prompt,
            None,
            &followup_body,
            fallback_conversation_id,
            fallback_response_id,
            fallback_app_path,
        )))
    }

    async fn best_effort_materialize_gemini_canvas_direct_http_media_asset(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        asset: &gemini_canvas::GeminiCanvasMediaAsset,
        timeout: Duration,
        provider: &str,
    ) -> gemini_canvas::GeminiCanvasMediaAsset {
        if asset.body_base64.is_some() {
            return asset.clone();
        }
        match self
            .materialize_gemini_canvas_direct_http_media_asset(
                payload,
                runtime,
                &asset.url,
                Some(asset.kind.as_str()),
                Some(asset.mime_type.as_str()),
                timeout,
            )
            .await
        {
            Ok(materialized) => materialized,
            Err(error) => {
                if payload.adapter == "gemini_canvas_program_web_reverse_compatible" {
                    debug!(
                        provider,
                        error = %summarize_gateway_error(&error),
                        "gemini canvas program-owned no-key direct asset materialization failed; browser-assisted fallback is disabled on this line"
                    );
                    return asset.clone();
                }
                debug!(
                    provider,
                    error = %summarize_gateway_error(&error),
                    "gemini canvas media asset materialization failed; returning URL-only asset"
                );
                asset.clone()
            }
        }
    }

    async fn materialize_gemini_canvas_direct_http_media_asset(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        asset_url: &str,
        asset_kind_hint: Option<&str>,
        asset_mime_hint: Option<&str>,
        timeout: Duration,
    ) -> Result<gemini_canvas::GeminiCanvasMediaAsset, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let base_url = payload.base_url.trim_end_matches('/');
        let page_origin = gemini_canvas_http_origin(payload);
        let page_referer = format!("{}/", page_origin.trim_end_matches('/'));
        let storage_state = gateway_object_storage()?
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
        let explicit_cookie_header = payload
            .extra_body
            .as_ref()
            .and_then(|extra| {
                extra
                    .get("canvasProgramInvokeContract")
                    .and_then(Value::as_object)
                    .and_then(|contract| contract.get("cookieHeader"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| {
                payload
                    .extra_body
                    .as_ref()
                    .and_then(|extra| extra.get("cookieHeader"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| read_json_string(&storage_state, "cookieHeader"));
        let mut current_url =
            normalize_gemini_canvas_direct_http_asset_url(Some(page_referer.as_str()), asset_url)
                .ok_or_else(|| gemini_canvas_media_fetch_bad_asset_url_error(asset_url))?;

        for _hop in 0..4 {
            let session = if let Some(cookie_header) = explicit_cookie_header
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                gemini_canvas::pure_http_session_from_cookie_header(cookie_header, &auth_user)?
            } else {
                gemini_canvas::storage_state_to_pure_http_session(
                    &storage_state,
                    &current_url,
                    base_url,
                    &auth_user,
                )?
            };
            let mut headers = build_gemini_canvas_direct_http_media_fetch_headers(
                payload,
                &current_url,
                &page_origin,
                &page_referer,
                asset_kind_hint,
            );
            if should_forward_gemini_canvas_download_cookies(&current_url) {
                apply_gemini_canvas_cookie_header(&mut headers, &session);
            }

            let response = self
                .http
                .request(Method::GET, &current_url)
                .headers(headers)
                .timeout(timeout.max(Duration::from_secs(120)))
                .redirect(rquest::redirect::Policy::none())
                .send()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            let status = response.status().as_u16();
            let content_type = response
                .headers()
                .get(rquest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .map(str::to_string);
            let redirect_target = response
                .headers()
                .get(rquest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|location| resolve_relative_url(&current_url, location));

            if response.status().is_redirection() {
                let next_url = redirect_target
                    .ok_or_else(gemini_canvas_media_fetch_redirect_missing_location_error)?;
                if next_url.contains("accounts.google.com/ServiceLogin")
                    || next_url.contains("accounts.google.com/CookieMismatch")
                {
                    return Err(gemini_canvas_media_fetch_cookie_mismatch_redirect_error());
                }
                current_url =
                    normalize_gemini_canvas_direct_http_asset_url(Some(&current_url), &next_url)
                        .ok_or_else(|| gemini_canvas_media_fetch_bad_redirect_error(&next_url))?;
                continue;
            }

            if !response.status().is_success() {
                let body_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| String::from("<unreadable body>"));
                return Err(classify_upstream_error(status, &body_text, Some(provider)));
            }

            let bytes = response
                .bytes()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            if content_type
                .as_deref()
                .map(|value| value.starts_with("text/html"))
                .unwrap_or(false)
                || current_url.contains("accounts.google.com/CookieMismatch")
                || current_url.contains("accounts.google.com/ServiceLogin")
            {
                return Err(gemini_canvas_media_fetch_cookie_mismatch_html_error());
            }
            let resolved_mime_type = infer_gemini_canvas_media_mime_type(
                content_type.as_deref(),
                asset_mime_hint,
                &current_url,
                asset_kind_hint,
            );
            let resolved_kind =
                infer_gemini_canvas_media_kind(&resolved_mime_type, asset_kind_hint, &current_url);
            let body_base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            return Ok(gemini_canvas::GeminiCanvasMediaAsset {
                kind: resolved_kind,
                url: current_url,
                mime_type: resolved_mime_type,
                download_token: None,
                body_base64: Some(body_base64),
                alt: None,
                width: None,
                height: None,
                duration_seconds: None,
            });
        }

        Err(gemini_canvas_media_fetch_redirect_exhausted_error())
    }

    async fn execute_gemini_canvas_direct_http_image(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: String,
        timeout: Duration,
    ) -> Result<Value, GatewayError> {
        let mut context = prepare_gemini_canvas_direct_http_image_context(payload, req, &prompt)?;
        let primary_result = self
            .execute_gemini_canvas_direct_http_image_primary_body(
                payload,
                req,
                model,
                runtime,
                &prompt,
                timeout,
                &mut context,
            )
            .await?;
        let body_text = match primary_result {
            GeminiCanvasDirectHttpImagePrimaryResult::StreamBody(body_text) => body_text,
            GeminiCanvasDirectHttpImagePrimaryResult::FinalResponse(body) => return Ok(body),
        };

        self.execute_gemini_canvas_direct_http_image_lane(
            payload,
            req,
            model,
            runtime,
            &prompt,
            &body_text,
            timeout,
            context.mode_index,
            context.request_started_at,
            context.initial_stream_allows_replay_template,
            context.image_edit_uploads.as_deref(),
            context.image_edit_followup_context.as_mut(),
            &context.image_json_policy,
        )
        .await
    }

    async fn extract_gemini_canvas_media_assets_with_followup(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        prompt: &str,
        primary_body: &str,
        request_started_at: SystemTime,
        timeout: Duration,
        force_root_app_followup: bool,
        _image_edit_followup_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
    ) -> Result<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String), GatewayError> {
        match gemini_canvas::extract_stream_generate_media_assets(primary_body, operation) {
            Ok(assets) => Ok((assets, primary_body.to_string())),
            Err(primary_error)
                if primary_error.code.as_deref()
                    == Some("gemini_canvas_image_generation_unavailable") =>
            {
                Err(primary_error)
            }
            Err(primary_error) => {
                self.extract_gemini_canvas_media_assets_after_primary_failure(
                    payload,
                    model,
                    runtime,
                    operation,
                    prompt,
                    primary_body,
                    primary_error,
                    request_started_at,
                    timeout,
                    force_root_app_followup,
                )
                .await
            }
        }
    }

    async fn extract_gemini_canvas_image_assets_with_followup(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        primary_body: &str,
        request_started_at: SystemTime,
        timeout: Duration,
        recovery_mode: GeminiCanvasImageRecoveryMode,
        image_edit_followup_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
    ) -> Result<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String), GatewayError> {
        match gemini_canvas::extract_stream_generate_media_assets(
            primary_body,
            gemini_canvas::GeminiCanvasMediaOperation::Image,
        ) {
            Ok(assets) => Ok((assets, primary_body.to_string())),
            Err(primary_error)
                if primary_error.code.as_deref()
                    == Some("gemini_canvas_image_generation_unavailable") =>
            {
                Err(primary_error)
            }
            Err(primary_error) if recovery_mode == GeminiCanvasImageRecoveryMode::EditAsync => {
                self.extract_gemini_canvas_image_edit_assets_with_followup(
                    payload,
                    model,
                    runtime,
                    primary_body,
                    primary_error,
                    timeout,
                    image_edit_followup_context,
                )
                .await
            }
            Err(primary_error) => {
                self.extract_gemini_canvas_image_generation_assets_with_followup(
                    payload,
                    model,
                    runtime,
                    prompt,
                    primary_body,
                    primary_error,
                    request_started_at,
                    timeout,
                )
                .await
            }
        }
    }

    async fn extract_gemini_canvas_image_generation_assets_with_followup(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        primary_body: &str,
        primary_error: GatewayError,
        request_started_at: SystemTime,
        timeout: Duration,
    ) -> Result<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String), GatewayError> {
        self.extract_gemini_canvas_media_assets_after_primary_failure(
            payload,
            model,
            runtime,
            gemini_canvas::GeminiCanvasMediaOperation::Image,
            prompt,
            primary_body,
            primary_error,
            request_started_at,
            timeout,
            false,
        )
        .await
    }

    async fn extract_gemini_canvas_image_edit_assets_with_followup(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        primary_body: &str,
        mut primary_error: GatewayError,
        timeout: Duration,
        image_edit_followup_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
    ) -> Result<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String), GatewayError> {
        if let Some(result) = self
            .try_resolve_gemini_canvas_image_edit_async_followups(
                payload,
                model,
                runtime,
                primary_body,
                timeout,
                image_edit_followup_context,
                &mut primary_error,
            )
            .await?
        {
            return Ok(result);
        }
        Err(primary_error)
    }

    async fn extract_gemini_canvas_media_assets_after_primary_failure(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        prompt: &str,
        primary_body: &str,
        mut primary_error: GatewayError,
        request_started_at: SystemTime,
        timeout: Duration,
        force_root_app_followup: bool,
    ) -> Result<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String), GatewayError> {
        let followup_body = match self
            .execute_gemini_canvas_direct_http_media_followup_body(
                payload,
                model,
                runtime,
                operation,
                prompt,
                primary_body,
                None,
                request_started_at,
                timeout,
                force_root_app_followup,
                None,
            )
            .await
        {
            Ok(body) => body,
            Err(followup_error)
                if followup_error.code.as_deref()
                    == Some("gemini_canvas_image_generation_unavailable") =>
            {
                return Err(followup_error);
            }
            Err(followup_error) => {
                if matches!(
                    operation,
                    gemini_canvas::GeminiCanvasMediaOperation::Image
                        | gemini_canvas::GeminiCanvasMediaOperation::Video
                ) && matches!(
                    followup_error.code.as_deref(),
                    Some("gemini_canvas_stream_generate_missing_response_id")
                        | Some("gemini_canvas_stream_generate_missing_conversation_id")
                ) {
                    match self
                        .poll_gemini_canvas_media_assets_from_conversation_page(
                            payload,
                            model,
                            runtime,
                            operation,
                            primary_body,
                            timeout,
                            force_root_app_followup,
                            None,
                            None,
                            None,
                        )
                        .await
                    {
                        Ok((assets, page_body)) => return Ok((assets, page_body)),
                        Err(page_poll_error) => {
                            primary_error.message = format!(
                                "{}; media_followup_failure={}; media_page_poll_failure={}",
                                primary_error.message,
                                summarize_gateway_error(&followup_error),
                                summarize_gateway_error(&page_poll_error)
                            );
                            return Err(primary_error);
                        }
                    }
                }
                primary_error.message = format!(
                    "{}; media_followup_failure={}",
                    primary_error.message,
                    summarize_gateway_error(&followup_error)
                );
                return Err(primary_error);
            }
        };

        match gemini_canvas::extract_stream_generate_media_assets(&followup_body, operation) {
            Ok(assets) => Ok((assets, followup_body)),
            Err(followup_extract_error)
                if followup_extract_error.code.as_deref()
                    == Some("gemini_canvas_image_generation_unavailable") =>
            {
                Err(followup_extract_error)
            }
            Err(followup_extract_error) => {
                let page_blob_extract_error = match gemini_canvas::extract_page_blob_media_assets(
                    &followup_body,
                    operation,
                ) {
                    Ok(assets) => return Ok((assets, followup_body)),
                    Err(error) => error,
                };
                let music_followup_pending = operation
                    == gemini_canvas::GeminiCanvasMediaOperation::Music
                    && gemini_canvas_music_body_indicates_accepted_progress(&followup_body);
                let video_followup_pending = operation
                    == gemini_canvas::GeminiCanvasMediaOperation::Video
                    && (gemini_canvas::response_indicates_video_generation_pending(&followup_body)
                        || gemini_canvas::response_indicates_video_generation_quota_reached(
                            &followup_body,
                        ));
                match self
                    .poll_gemini_canvas_media_assets_from_conversation_page(
                        payload,
                        model,
                        runtime,
                        operation,
                        &followup_body,
                        timeout,
                        force_root_app_followup,
                        None,
                        None,
                        None,
                    )
                    .await
                {
                    Ok((assets, page_body)) => Ok((assets, page_body)),
                    Err(page_poll_error) => {
                        if music_followup_pending || video_followup_pending {
                            return Ok((Vec::new(), followup_body));
                        }
                        primary_error.message = format!(
                            "{}; media_followup_extract_failure={}; media_followup_blob_failure={}; media_page_poll_failure={}; media_followup_preview={}",
                            primary_error.message,
                            summarize_gateway_error(&followup_extract_error),
                            summarize_gateway_error(&page_blob_extract_error),
                            summarize_gateway_error(&page_poll_error),
                            compact_response_preview(&followup_body, 320)
                        );
                        Err(primary_error)
                    }
                }
            }
        }
    }

    async fn try_resolve_gemini_canvas_image_edit_async_followups(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        primary_body: &str,
        timeout: Duration,
        mut image_edit_followup_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
        primary_error: &mut GatewayError,
    ) -> Result<Option<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String)>, GatewayError> {
        append_gemini_canvas_image_edit_trace(
            "followup.signaler.start",
            compact_response_preview(primary_body, 180),
        );
        if let Some(context) = image_edit_followup_context.as_deref_mut() {
            if context.signaler_response_id.is_none() {
                if let Some(response_id) =
                    gemini_canvas::extract_stream_generate_response_id(primary_body).ok()
                {
                    context.signaler_response_id = Some(response_id);
                }
            }
            if context.signaler_conversation_id.is_none() {
                if let Some(locator) =
                    gemini_canvas::extract_stream_generate_locator(primary_body).ok()
                {
                    context.signaler_conversation_id = Some(locator.conversation_id);
                    if context.signaler_response_id.is_none() {
                        context.signaler_response_id = Some(locator.response_id);
                    }
                }
            }
        }
        let image_edit_locale_hint = image_edit_followup_context
            .as_ref()
            .and_then(|context| context.locale_hint.clone());
        match self
            .poll_gemini_canvas_image_edit_signaler_assets(
                payload,
                model,
                runtime,
                timeout,
                image_edit_followup_context.as_deref_mut(),
            )
            .await
        {
            Ok((assets, body)) => {
                append_gemini_canvas_image_edit_trace(
                    "followup.signaler.ok",
                    compact_response_preview(&body, 220),
                );
                return Ok(Some((assets, body)));
            }
            Err(signaler_error) => {
                append_gemini_canvas_image_edit_trace(
                    "followup.signaler.err",
                    summarize_gateway_error(&signaler_error),
                );
                primary_error.message = format!(
                    "{}; signaler_followup_failure={}",
                    primary_error.message,
                    summarize_gateway_error(&signaler_error)
                );
            }
        }

        if let Some(edit_context) = image_edit_followup_context.as_deref_mut() {
            match self
                .execute_gemini_canvas_image_edit_post_ack_followups(
                    payload,
                    model,
                    runtime,
                    primary_body,
                    timeout,
                    edit_context,
                )
                .await
            {
                Ok(post_ack_result) => {
                    append_gemini_canvas_image_edit_trace(
                        "followup.post-ack.ok",
                        compact_response_preview(&post_ack_result.preview, 220),
                    );
                    primary_error.message = format!(
                        "{}; post_ack_followup_preview={}",
                        primary_error.message,
                        compact_response_preview(&post_ack_result.preview, 220)
                    );
                    if let Some(page_body) = post_ack_result.page_body {
                        if let Ok(assets) = gemini_canvas::extract_page_blob_media_assets(
                            &page_body,
                            gemini_canvas::GeminiCanvasMediaOperation::Image,
                        ) {
                            return Ok(Some((assets, page_body)));
                        }
                    }
                }
                Err(post_ack_error) => {
                    append_gemini_canvas_image_edit_trace(
                        "followup.post-ack.err",
                        summarize_gateway_error(&post_ack_error),
                    );
                    primary_error.message = format!(
                        "{}; post_ack_followup_failure={}",
                        primary_error.message,
                        summarize_gateway_error(&post_ack_error)
                    );
                }
            }
            append_gemini_canvas_image_edit_trace(
                "followup.conversation.start",
                edit_context
                    .signaler_app_url
                    .clone()
                    .unwrap_or_else(|| "<none>".to_string()),
            );
            match self
                .poll_gemini_canvas_image_edit_conversation_assets(
                    payload,
                    model,
                    runtime,
                    timeout,
                    edit_context,
                )
                .await
            {
                Ok((assets, body)) => {
                    append_gemini_canvas_image_edit_trace(
                        "followup.conversation.ok",
                        compact_response_preview(&body, 220),
                    );
                    return Ok(Some((assets, body)));
                }
                Err(conversation_error) => {
                    append_gemini_canvas_image_edit_trace(
                        "followup.conversation.err",
                        summarize_gateway_error(&conversation_error),
                    );
                    primary_error.message = format!(
                        "{}; conversation_followup_failure={}",
                        primary_error.message,
                        summarize_gateway_error(&conversation_error)
                    );
                }
            }
        }

        let signaler_session = image_edit_followup_context
            .as_ref()
            .and_then(|context| context.signaler_session.as_ref());
        let signaler_app_url = image_edit_followup_context
            .as_ref()
            .and_then(|context| context.signaler_app_url.as_deref());
        match self
            .poll_gemini_canvas_media_assets_from_conversation_page(
                payload,
                model,
                runtime,
                gemini_canvas::GeminiCanvasMediaOperation::Image,
                primary_body,
                timeout,
                true,
                image_edit_locale_hint.as_deref(),
                signaler_session,
                signaler_app_url,
            )
            .await
        {
            Ok((assets, page_body)) => {
                append_gemini_canvas_image_edit_trace(
                    "followup.page.ok",
                    compact_response_preview(&page_body, 220),
                );
                Ok(Some((assets, page_body)))
            }
            Err(page_poll_error) => {
                append_gemini_canvas_image_edit_trace(
                    "followup.page.err",
                    summarize_gateway_error(&page_poll_error),
                );
                primary_error.message = format!(
                    "{}; early_media_page_poll_failure={}",
                    primary_error.message,
                    summarize_gateway_error(&page_poll_error)
                );
                Ok(None)
            }
        }
    }

    async fn execute_gemini_canvas_media_followup_preflight_contract(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        followup_target: &GeminiCanvasFollowupTarget,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        is_image_edit_request: bool,
        timeout: Duration,
    ) -> Result<GeminiCanvasMediaFollowupPreflightOutcome, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let preflight_plan = build_gemini_canvas_media_followup_preflight_plan(
            bootstrap,
            &followup_target.source_path,
            operation,
        )?;

        let parity_followup = match self
            .execute_gemini_canvas_media_capture_parity_preflight_sequence(
                payload,
                model,
                bootstrap,
                &followup_target.source_path,
                session,
                preflight_plan.mode_index,
                preflight_plan.batchexecute_header_id.as_deref(),
                is_image_edit_request,
                timeout,
            )
            .await
        {
            Ok(result) => Some(result),
            Err(error) => {
                debug!(
                    provider,
                    app_path = %followup_target.source_path,
                    mode_index = preflight_plan.mode_index,
                    error = %summarize_gateway_error(&error),
                    "gemini canvas media parity follow-up preflight failed; falling back to legacy follow-up chain"
                );
                None
            }
        };

        if let Some(parity_followup) = parity_followup {
            let selected_bootstrap_body = parity_followup.selected_bootstrap_body;
            if gemini_canvas::extract_stream_generate_media_assets(
                &selected_bootstrap_body,
                operation,
            )
            .is_ok()
            {
                return Ok(GeminiCanvasMediaFollowupPreflightOutcome::Completed(
                    selected_bootstrap_body,
                ));
            }

            self.execute_gemini_canvas_media_legacy_preflight_sequence(
                payload,
                model,
                bootstrap,
                &followup_target.source_path,
                session,
                preflight_plan.mode_index,
                preflight_plan.batchexecute_header_id.as_deref(),
                timeout,
            )
            .await?;

            return Ok(GeminiCanvasMediaFollowupPreflightOutcome::Ready(
                GeminiCanvasMediaFollowupPreflightContract {
                    preflight_plan,
                    strategy: GeminiCanvasMediaFollowupPreflightStrategy::ParityThenLegacy,
                    selected_bootstrap_body: Some(selected_bootstrap_body),
                },
            ));
        }

        self.execute_gemini_canvas_media_legacy_preflight_sequence(
            payload,
            model,
            bootstrap,
            &followup_target.source_path,
            session,
            preflight_plan.mode_index,
            preflight_plan.batchexecute_header_id.as_deref(),
            timeout,
        )
        .await?;

        Ok(GeminiCanvasMediaFollowupPreflightOutcome::Ready(
            GeminiCanvasMediaFollowupPreflightContract {
                preflight_plan,
                strategy: GeminiCanvasMediaFollowupPreflightStrategy::LegacyOnly,
                selected_bootstrap_body: None,
            },
        ))
    }

    async fn execute_gemini_canvas_media_followup_attempts(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        followup_target: &GeminiCanvasFollowupTarget,
        preflight_plan: &GeminiCanvasMediaFollowupPreflightPlan,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        bootstrap_page_url: &str,
        timeout: Duration,
    ) -> GeminiCanvasMediaFollowupAttemptOutcome {
        let provider = "gemini_canvas_compatible";
        let response_id_log = followup_target.response_id();
        let conversation_id_log = followup_target.conversation_id();
        let followup_attempts = if operation == gemini_canvas::GeminiCanvasMediaOperation::Music {
            3usize
        } else {
            1usize
        };
        let mut last_body: Option<String> = None;
        let mut last_error: Option<GatewayError> = None;

        for attempt in 0..followup_attempts {
            if operation == gemini_canvas::GeminiCanvasMediaOperation::Music {
                if let Err(error) = self
                    .send_gemini_canvas_text_batchexecute_request(
                        payload,
                        model,
                        &preflight_plan.activity_request,
                        session,
                        &preflight_plan.followup_model_header,
                        timeout,
                    )
                    .await
                {
                    debug!(
                        provider,
                        app_path = %followup_target.source_path,
                        response_id = response_id_log,
                        conversation_id = conversation_id_log,
                        attempt = attempt + 1,
                        error = %summarize_gateway_error(&error),
                        "gemini canvas media ESY5D follow-up preflight failed"
                    );
                }
            }

            match self
                .send_gemini_canvas_text_batchexecute_request(
                    payload,
                    model,
                    &preflight_plan.followup_request,
                    session,
                    &preflight_plan.followup_model_header,
                    timeout,
                )
                .await
            {
                Ok(body) => {
                    if gemini_canvas::extract_stream_generate_media_assets(&body, operation).is_ok()
                    {
                        return GeminiCanvasMediaFollowupAttemptOutcome::Completed(body);
                    }
                    last_error = Some(build_gemini_canvas_media_followup_missing_asset_error(
                        provider,
                        operation,
                        followup_target,
                        bootstrap_page_url,
                        attempt + 1,
                        &body,
                    ));
                    last_body = Some(body);
                }
                Err(error) => {
                    let mut wrapped = gemini_canvas_media_followup_failed_error(
                        provider,
                        followup_target,
                        bootstrap_page_url,
                        summarize_gateway_error(&error).as_str(),
                    );
                    wrapped.http_status = error.http_status;
                    last_error = Some(wrapped);
                }
            }

            if attempt + 1 < followup_attempts {
                sleep(Duration::from_millis(750)).await;
            }
        }

        GeminiCanvasMediaFollowupAttemptOutcome::Incomplete(GeminiCanvasMediaFollowupAttemptState {
            last_body,
            last_error,
        })
    }

    async fn finalize_gemini_canvas_video_followup_result(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        followup_target: &GeminiCanvasFollowupTarget,
        preflight_plan: &GeminiCanvasMediaFollowupPreflightPlan,
        stream_body: &str,
        timeout: Duration,
        force_root_app_followup: bool,
        locale_override: Option<&str>,
        bootstrap_page_url: &str,
        state: GeminiCanvasMediaFollowupAttemptState,
    ) -> Result<String, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let GeminiCanvasMediaFollowupAttemptState {
            last_body,
            mut last_error,
        } = state;

        let Some(locator) = followup_target.locator.as_ref() else {
            if last_error.is_none() {
                last_error = Some(gemini_canvas_video_followup_missing_locator_error(
                    provider,
                    followup_target.source_path.as_str(),
                    bootstrap_page_url,
                ));
            }
            return finalize_gemini_canvas_media_followup_attempt_state(
                provider,
                followup_target,
                bootstrap_page_url,
                GeminiCanvasMediaFollowupAttemptState {
                    last_body,
                    last_error,
                },
            );
        };

        match self
            .recover_gemini_canvas_video_completion_or_page_body(
                payload,
                model,
                runtime,
                bootstrap,
                session,
                followup_target,
                locator,
                &preflight_plan.followup_model_header,
                stream_body,
                timeout,
                force_root_app_followup,
                locale_override,
            )
            .await
        {
            Ok(body) => Ok(body),
            Err(error) => {
                last_error = Some(error);
                finalize_gemini_canvas_media_followup_attempt_state(
                    provider,
                    followup_target,
                    bootstrap_page_url,
                    GeminiCanvasMediaFollowupAttemptState {
                        last_body,
                        last_error,
                    },
                )
            }
        }
    }

    async fn finalize_gemini_canvas_music_followup_result(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        prompt: &str,
        stream_body: &str,
        request_started_at: SystemTime,
        timeout: Duration,
        force_root_app_followup: bool,
        locale_override: Option<&str>,
        followup_target: &GeminiCanvasFollowupTarget,
        preflight_plan: &GeminiCanvasMediaFollowupPreflightPlan,
        bootstrap_page_url: &str,
        state: GeminiCanvasMediaFollowupAttemptState,
    ) -> Result<String, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let GeminiCanvasMediaFollowupAttemptState {
            last_body,
            last_error: _,
        } = state;
        let mut page_seed_body = last_body.clone().unwrap_or_else(|| stream_body.to_string());

        if let Ok(response_id) = gemini_canvas::extract_stream_generate_response_id(&page_seed_body)
            .or_else(|_| gemini_canvas::extract_stream_generate_response_id(stream_body))
        {
            let mut trigger_session = session.clone();
            let trigger_source_paths =
                if followup_target.source_path == gemini_web::GEMINI_WEB_DEFAULT_APP_PATH {
                    vec![followup_target.source_path.clone()]
                } else {
                    vec![
                        followup_target.source_path.clone(),
                        gemini_web::GEMINI_WEB_DEFAULT_APP_PATH.to_string(),
                    ]
                };

            for trigger_source_path in trigger_source_paths {
                let trigger_request = gemini_canvas::build_music_trigger_request(
                    &response_id,
                    bootstrap,
                    &trigger_source_path,
                )?;
                match self
                    .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                        payload,
                        model,
                        &trigger_request,
                        &mut trigger_session,
                        &preflight_plan.followup_model_header,
                        timeout.min(Duration::from_secs(20)).max(Duration::from_secs(8)),
                    )
                    .await
                {
                    Ok(trigger_body) => {
                        if gemini_canvas::extract_stream_generate_media_assets(
                            &trigger_body,
                            gemini_canvas::GeminiCanvasMediaOperation::Music,
                        )
                        .is_ok()
                            || gemini_canvas::extract_page_blob_media_assets(
                                &trigger_body,
                                gemini_canvas::GeminiCanvasMediaOperation::Music,
                            )
                            .is_ok()
                        {
                            return Ok(trigger_body);
                        }
                        page_seed_body = trigger_body;
                        break;
                    }
                    Err(_error) => {}
                }
            }
        }

        let mut last_error = match self
            .poll_gemini_canvas_media_assets_from_conversation_page(
                payload,
                model,
                runtime,
                gemini_canvas::GeminiCanvasMediaOperation::Music,
                &page_seed_body,
                timeout,
                force_root_app_followup,
                locale_override,
                Some(session),
                None,
            )
            .await
        {
            Ok((_assets, page_body)) => return Ok(page_body),
            Err(error) => {
                let upstream_summary = summarize_gateway_error(&error);
                Some(gemini_canvas_program_music_page_poll_failed_error(
                    provider,
                    bootstrap_page_url,
                    upstream_summary.as_str(),
                ))
            }
        };

        let recovered_page_url = self
            .try_recover_gemini_canvas_recent_conversation_page_url(
                payload,
                model,
                bootstrap,
                session,
                &followup_target.source_path,
                &preflight_plan.followup_model_header,
                prompt,
                request_started_at,
                timeout,
            )
            .await?;

        if let Some(app_page_url) = recovered_page_url.as_deref() {
            match self
                .poll_gemini_canvas_media_assets_from_conversation_page(
                    payload,
                    model,
                    runtime,
                    gemini_canvas::GeminiCanvasMediaOperation::Music,
                    &page_seed_body,
                    timeout,
                    force_root_app_followup,
                    locale_override,
                    Some(session),
                    Some(app_page_url),
                )
                .await
            {
                Ok((_assets, page_body)) => return Ok(page_body),
                Err(error) => {
                    let upstream_summary = summarize_gateway_error(&error);
                    last_error = Some(gemini_canvas_program_music_recent_page_poll_failed_error(
                        provider,
                        bootstrap_page_url,
                        app_page_url,
                        upstream_summary.as_str(),
                    ));
                }
            }
        }

        finalize_gemini_canvas_media_followup_attempt_state(
            provider,
            followup_target,
            bootstrap_page_url,
            GeminiCanvasMediaFollowupAttemptState {
                last_body,
                last_error,
            },
        )
    }

    async fn prepare_gemini_canvas_direct_http_media_followup_context(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        stream_body: &str,
        locator_override: Option<gemini_canvas::GeminiCanvasStreamGenerateLocator>,
        timeout: Duration,
        force_root_app_followup: bool,
        locale_override: Option<&str>,
    ) -> Result<GeminiCanvasMediaFollowupContext, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let mut page_seed = prepare_gemini_canvas_page_seed(
            payload,
            runtime,
            operation,
            stream_body,
            force_root_app_followup,
            None,
            false,
            provider,
            "gemini canvas media follow-up",
        )?;
        if let Some(locator) = locator_override {
            let page_base_url = gemini_canvas_page_base_url(payload);
            page_seed.conversation_page_url = Some(format!(
                "{}{}",
                page_base_url.trim_end_matches('/'),
                locator.app_path
            ));
            page_seed.prefer_root_app_path = false;
            page_seed.locator = Some(locator);
        }
        let locator = page_seed.locator;
        let initial_page_target_mode = classify_gemini_canvas_page_target_mode(
            force_root_app_followup,
            false,
            locator.is_some(),
            false,
        );
        let force_root_app_path = page_seed.prefer_root_app_path;
        let base_url = payload.base_url.trim_end_matches('/');
        let app_bootstrap_url = page_seed.app_bootstrap_url;
        let share_bootstrap_url = page_seed.share_bootstrap_url;
        let conversation_url = page_seed.conversation_page_url;
        let storage_state = gateway_object_storage()?
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
        let session = gemini_canvas_pure_http_session_from_payload_or_storage(
            payload,
            &storage_state,
            conversation_url
                .as_deref()
                .unwrap_or(app_bootstrap_url.as_str()),
            base_url,
            &auth_user,
        )?;
        let bootstrap_candidates = build_gemini_canvas_followup_bootstrap_candidates(
            force_root_app_path,
            conversation_url.as_deref(),
            &app_bootstrap_url,
            &share_bootstrap_url,
        );
        let mut bootstrap_failures = Vec::new();
        let mut selected_bootstrap: Option<(String, String)> = None;

        for candidate_url in &bootstrap_candidates {
            match self
                .fetch_gemini_canvas_direct_http_page_html_with_locale(
                    payload,
                    &session,
                    candidate_url.as_str(),
                    timeout,
                    locale_override,
                )
                .await
            {
                Ok(body) => {
                    selected_bootstrap = Some((body, candidate_url.clone()));
                    break;
                }
                Err(error) => bootstrap_failures.push(format!(
                    "{}: {}",
                    candidate_url,
                    summarize_gateway_error(&error)
                )),
            }
        }

        let (bootstrap_body, bootstrap_page_url) = selected_bootstrap.ok_or_else(|| {
            gemini_canvas_media_followup_bootstrap_failed_error(
                initial_page_target_mode,
                locator
                    .as_ref()
                    .map(|locator| locator.app_path.as_str())
                    .unwrap_or(gemini_web::GEMINI_WEB_DEFAULT_APP_PATH),
                bootstrap_candidates.join(",").as_str(),
                bootstrap_failures.join(" | ").as_str(),
            )
        })?;

        let fallback_bootstrap =
            gemini_web::bootstrap_from_payload_cache(payload.extra_body.as_ref());
        let mut bootstrap =
            gemini_web::parse_bootstrap_from_app_html(&bootstrap_body, Some(&bootstrap_page_url))
                .or_else(|primary_error| fallback_bootstrap.clone().ok_or(primary_error))?;
        bootstrap =
            gemini_web::merge_bootstrap_from_fallback(bootstrap, fallback_bootstrap.as_ref());
        if force_root_app_followup && operation == gemini_canvas::GeminiCanvasMediaOperation::Image
        {
            if let Some(locale) = gemini_canvas::harvest_image_edit_template_locale(&storage_state)
            {
                bootstrap.language = locale;
            }
        }

        let followup_target = GeminiCanvasFollowupTarget::from_bootstrap(
            force_root_app_path,
            bootstrap.app_page_path.as_deref(),
            locator,
            initial_page_target_mode,
        );

        Ok(GeminiCanvasMediaFollowupContext {
            bootstrap,
            bootstrap_page_url,
            session,
            followup_target,
        })
    }

    async fn execute_gemini_canvas_direct_http_image_primary_body(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        timeout: Duration,
        context: &mut GeminiCanvasDirectHttpImageContext,
    ) -> Result<GeminiCanvasDirectHttpImagePrimaryResult, GatewayError> {
        let provider = "gemini_canvas_compatible";
        if context.image_json_policy.initial_action()
            == GeminiCanvasDirectHttpImageJsonAction::TryJson
        {
            match self
                .execute_gemini_canvas_direct_http_image_json(
                    payload, req, model, runtime, prompt, timeout,
                )
                .await
            {
                Ok(body) => {
                    return Ok(GeminiCanvasDirectHttpImagePrimaryResult::FinalResponse(
                        body,
                    ));
                }
                Err(image_json_error) => {
                    let image_json_summary = summarize_gateway_error(&image_json_error);
                    debug!(
                        provider,
                        error = %image_json_summary,
                        "gemini canvas direct HTTP image inline JSON attempt failed; falling back to StreamGenerate asset replay for non-url response"
                    );
                    context
                        .image_json_policy
                        .note_prefill_failure(image_json_summary);
                }
            }
        }
        if req.endpoint_kind == EndpointKind::ImagesEdits
            && !context.initial_stream_allows_replay_template
        {
            append_gemini_canvas_image_edit_trace(
                "stream.force-heavy-only",
                "Skipping replay-template-first path for focused image-edit probe.",
            );
        }
        match self
            .execute_gemini_canvas_direct_http_stream_generate_body(
                payload,
                model,
                runtime,
                context.mode_index,
                prompt,
                timeout,
                context.initial_stream_allows_replay_template,
                context.image_edit_uploads.as_deref(),
                context.image_edit_followup_context.as_mut(),
            )
            .await
        {
            Ok(body_text) => Ok(GeminiCanvasDirectHttpImagePrimaryResult::StreamBody(
                body_text,
            )),
            Err(stream_error) => match context.image_json_policy.on_stream_failure() {
                GeminiCanvasDirectHttpImageJsonAction::ReturnOriginal => Err(stream_error),
                GeminiCanvasDirectHttpImageJsonAction::ReturnOriginalWithSummary {
                    context_key,
                    summary,
                } => Err(append_gateway_error_summary(
                    stream_error,
                    context_key,
                    Some(&summary),
                )),
                GeminiCanvasDirectHttpImageJsonAction::TryJsonWithErrorContext { context_key } => {
                    let stream_summary = summarize_gateway_error(&stream_error);
                    debug!(
                        provider,
                        error = %stream_summary,
                        "gemini canvas direct HTTP image StreamGenerate replay failed; trying legacy JSON fallback because image_json_fallback_enabled=true"
                    );
                    match self
                        .execute_gemini_canvas_direct_http_image_json(
                            payload, req, model, runtime, prompt, timeout,
                        )
                        .await
                    {
                        Ok(body) => Ok(GeminiCanvasDirectHttpImagePrimaryResult::FinalResponse(
                            body,
                        )),
                        Err(image_json_error) => Err(append_gateway_error_summary(
                            image_json_error,
                            context_key,
                            Some(&stream_summary),
                        )),
                    }
                }
                GeminiCanvasDirectHttpImageJsonAction::Skip
                | GeminiCanvasDirectHttpImageJsonAction::TryJson => Err(stream_error),
            },
        }
    }

    async fn execute_gemini_canvas_direct_http_media_followup_body(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        prompt: &str,
        stream_body: &str,
        locator_override: Option<gemini_canvas::GeminiCanvasStreamGenerateLocator>,
        request_started_at: SystemTime,
        timeout: Duration,
        force_root_app_followup: bool,
        locale_override: Option<&str>,
    ) -> Result<String, GatewayError> {
        let GeminiCanvasMediaFollowupContext {
            bootstrap,
            bootstrap_page_url,
            mut session,
            mut followup_target,
        } = self
            .prepare_gemini_canvas_direct_http_media_followup_context(
                payload,
                runtime,
                operation,
                stream_body,
                locator_override,
                timeout,
                force_root_app_followup,
                locale_override,
            )
            .await?;
        let preflight_contract = match self
            .execute_gemini_canvas_media_followup_preflight_contract(
                payload,
                model,
                &bootstrap,
                &followup_target,
                &mut session,
                operation,
                force_root_app_followup,
                timeout,
            )
            .await?
        {
            GeminiCanvasMediaFollowupPreflightOutcome::Completed(body) => return Ok(body),
            GeminiCanvasMediaFollowupPreflightOutcome::Ready(contract) => contract,
        };
        let GeminiCanvasMediaFollowupPreflightContract {
            preflight_plan,
            strategy: preflight_strategy,
            selected_bootstrap_body,
        } = preflight_contract;

        self.execute_gemini_canvas_media_followup_after_preflight(
            payload,
            model,
            runtime,
            operation,
            prompt,
            stream_body,
            request_started_at,
            timeout,
            force_root_app_followup,
            locale_override,
            &bootstrap,
            &session,
            &mut followup_target,
            &preflight_plan,
            preflight_strategy,
            selected_bootstrap_body.as_deref(),
            &bootstrap_page_url,
        )
        .await
    }

    async fn execute_gemini_canvas_media_followup_after_preflight(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        prompt: &str,
        stream_body: &str,
        request_started_at: SystemTime,
        timeout: Duration,
        force_root_app_followup: bool,
        locale_override: Option<&str>,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        followup_target: &mut GeminiCanvasFollowupTarget,
        preflight_plan: &GeminiCanvasMediaFollowupPreflightPlan,
        preflight_strategy: GeminiCanvasMediaFollowupPreflightStrategy,
        selected_bootstrap_body: Option<&str>,
        bootstrap_page_url: &str,
    ) -> Result<String, GatewayError> {
        if operation == gemini_canvas::GeminiCanvasMediaOperation::Video {
            return self
                .execute_gemini_canvas_video_post_preflight_followup(
                    payload,
                    model,
                    runtime,
                    prompt,
                    stream_body,
                    request_started_at,
                    timeout,
                    force_root_app_followup,
                    locale_override,
                    bootstrap,
                    session,
                    followup_target,
                    preflight_plan,
                    preflight_strategy,
                    selected_bootstrap_body,
                    bootstrap_page_url,
                )
                .await;
        }
        if operation == gemini_canvas::GeminiCanvasMediaOperation::Music {
            return self
                .execute_gemini_canvas_music_post_preflight_followup(
                    payload,
                    model,
                    runtime,
                    prompt,
                    stream_body,
                    request_started_at,
                    timeout,
                    force_root_app_followup,
                    locale_override,
                    bootstrap,
                    session,
                    followup_target,
                    preflight_plan,
                    preflight_strategy,
                    selected_bootstrap_body,
                    bootstrap_page_url,
                )
                .await;
        }

        let provider = "gemini_canvas_compatible";
        let response_id_log = followup_target.response_id();
        let conversation_id_log = followup_target.conversation_id();
        debug!(
            provider,
            app_path = %followup_target.source_path,
            response_id = response_id_log,
            conversation_id = conversation_id_log,
            bootstrap_page = %bootstrap_page_url,
            preflight_strategy = preflight_strategy.as_str(),
            selected_bootstrap_preview = %selected_bootstrap_body
                .map(|body| compact_response_preview(body, 180))
                .unwrap_or_else(|| "<none>".to_string()),
            "sending gemini canvas pure HTTP media aPya6c follow-up request"
        );
        let followup_attempt_state = match self
            .execute_gemini_canvas_media_followup_attempts(
                payload,
                model,
                operation,
                followup_target,
                preflight_plan,
                session,
                bootstrap_page_url,
                timeout,
            )
            .await
        {
            GeminiCanvasMediaFollowupAttemptOutcome::Completed(body) => return Ok(body),
            GeminiCanvasMediaFollowupAttemptOutcome::Incomplete(state) => state,
        };

        finalize_gemini_canvas_media_followup_attempt_state(
            provider,
            followup_target,
            bootstrap_page_url,
            followup_attempt_state,
        )
    }

    async fn execute_gemini_canvas_video_post_preflight_followup(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        stream_body: &str,
        request_started_at: SystemTime,
        timeout: Duration,
        force_root_app_followup: bool,
        locale_override: Option<&str>,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        followup_target: &mut GeminiCanvasFollowupTarget,
        preflight_plan: &GeminiCanvasMediaFollowupPreflightPlan,
        preflight_strategy: GeminiCanvasMediaFollowupPreflightStrategy,
        selected_bootstrap_body: Option<&str>,
        bootstrap_page_url: &str,
    ) -> Result<String, GatewayError> {
        let provider = "gemini_canvas_compatible";
        self.try_ensure_gemini_canvas_video_followup_locator(
            payload,
            model,
            bootstrap,
            session,
            &preflight_plan.followup_model_header,
            prompt,
            request_started_at,
            stream_body,
            timeout,
            followup_target,
        )
        .await;
        let response_id_log = followup_target.response_id();
        let conversation_id_log = followup_target.conversation_id();
        debug!(
            provider,
            app_path = %followup_target.source_path,
            response_id = response_id_log,
            conversation_id = conversation_id_log,
            bootstrap_page = %bootstrap_page_url,
            preflight_strategy = preflight_strategy.as_str(),
            selected_bootstrap_preview = %selected_bootstrap_body
                .map(|body| compact_response_preview(body, 180))
                .unwrap_or_else(|| "<none>".to_string()),
            "sending gemini canvas pure HTTP video aPya6c follow-up request"
        );
        let followup_attempt_state = match self
            .execute_gemini_canvas_media_followup_attempts(
                payload,
                model,
                gemini_canvas::GeminiCanvasMediaOperation::Video,
                followup_target,
                preflight_plan,
                session,
                bootstrap_page_url,
                timeout,
            )
            .await
        {
            GeminiCanvasMediaFollowupAttemptOutcome::Completed(body) => return Ok(body),
            GeminiCanvasMediaFollowupAttemptOutcome::Incomplete(state) => state,
        };

        self.finalize_gemini_canvas_video_followup_result(
            payload,
            model,
            runtime,
            bootstrap,
            session,
            followup_target,
            preflight_plan,
            stream_body,
            timeout,
            force_root_app_followup,
            locale_override,
            bootstrap_page_url,
            followup_attempt_state,
        )
        .await
    }

    async fn execute_gemini_canvas_music_post_preflight_followup(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        stream_body: &str,
        request_started_at: SystemTime,
        timeout: Duration,
        force_root_app_followup: bool,
        locale_override: Option<&str>,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        followup_target: &mut GeminiCanvasFollowupTarget,
        preflight_plan: &GeminiCanvasMediaFollowupPreflightPlan,
        preflight_strategy: GeminiCanvasMediaFollowupPreflightStrategy,
        selected_bootstrap_body: Option<&str>,
        bootstrap_page_url: &str,
    ) -> Result<String, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let response_id_log = followup_target.response_id();
        let conversation_id_log = followup_target.conversation_id();
        debug!(
            provider,
            app_path = %followup_target.source_path,
            response_id = response_id_log,
            conversation_id = conversation_id_log,
            bootstrap_page = %bootstrap_page_url,
            preflight_strategy = preflight_strategy.as_str(),
            selected_bootstrap_preview = %selected_bootstrap_body
                .map(|body| compact_response_preview(body, 180))
                .unwrap_or_else(|| "<none>".to_string()),
            "sending gemini canvas pure HTTP music aPya6c follow-up request"
        );
        let followup_attempt_state = match self
            .execute_gemini_canvas_media_followup_attempts(
                payload,
                model,
                gemini_canvas::GeminiCanvasMediaOperation::Music,
                followup_target,
                preflight_plan,
                session,
                bootstrap_page_url,
                timeout,
            )
            .await
        {
            GeminiCanvasMediaFollowupAttemptOutcome::Completed(body) => return Ok(body),
            GeminiCanvasMediaFollowupAttemptOutcome::Incomplete(state) => state,
        };

        self.finalize_gemini_canvas_music_followup_result(
            payload,
            model,
            runtime,
            bootstrap,
            session,
            prompt,
            stream_body,
            request_started_at,
            timeout,
            force_root_app_followup,
            locale_override,
            followup_target,
            preflight_plan,
            bootstrap_page_url,
            followup_attempt_state,
        )
        .await
    }

    async fn try_ensure_gemini_canvas_video_followup_locator(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        model_header: &str,
        prompt: &str,
        request_started_at: SystemTime,
        stream_body: &str,
        timeout: Duration,
        followup_target: &mut GeminiCanvasFollowupTarget,
    ) {
        let provider = "gemini_canvas_compatible";
        if followup_target.locator.is_some() {
            return;
        }
        let response_id_hint = gemini_canvas::extract_stream_generate_response_id(stream_body).ok();
        match self
            .try_recover_gemini_canvas_video_locator_from_conversation_list(
                payload,
                model,
                bootstrap,
                session,
                &followup_target.source_path,
                model_header,
                prompt,
                request_started_at,
                response_id_hint.as_deref(),
                timeout,
            )
            .await
        {
            Ok(Some(recovered_locator)) => {
                followup_target.adopt_video_recovered_locator(recovered_locator);
            }
            Ok(None) => {}
            Err(error) => {
                debug!(
                    provider,
                    error = %summarize_gateway_error(&error),
                    "gemini canvas video conversation-list locator recovery failed"
                );
            }
        }
    }

    async fn recover_gemini_canvas_video_completion_or_page_body(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        followup_target: &GeminiCanvasFollowupTarget,
        locator: &gemini_canvas::GeminiCanvasStreamGenerateLocator,
        model_header: &str,
        stream_body: &str,
        timeout: Duration,
        force_root_app_followup: bool,
        locale_override: Option<&str>,
    ) -> Result<String, GatewayError> {
        let provider = "gemini_canvas_compatible";
        match self
            .poll_gemini_canvas_direct_http_video_completion_body(
                payload,
                model,
                bootstrap,
                session,
                &followup_target.source_path,
                &locator.conversation_id,
                &locator.response_id,
                model_header,
                stream_body,
                timeout,
            )
            .await
        {
            Ok(body) => Ok(body),
            Err(error) => {
                debug!(
                    provider,
                    app_path = %followup_target.source_path,
                    response_id = followup_target.response_id(),
                    conversation_id = followup_target.conversation_id(),
                    error = %summarize_gateway_error(&error),
                    "gemini canvas media video completion follow-up failed; trying concrete page fallback"
                );
                let base_url = payload.base_url.trim_end_matches('/');
                let concrete_page_url = format!("{base_url}{}", locator.app_path);
                match self
                    .poll_gemini_canvas_media_assets_from_conversation_page(
                        payload,
                        model,
                        runtime,
                        gemini_canvas::GeminiCanvasMediaOperation::Video,
                        stream_body,
                        timeout
                            .min(Duration::from_secs(150))
                            .max(Duration::from_secs(45)),
                        force_root_app_followup,
                        locale_override,
                        Some(session),
                        Some(concrete_page_url.as_str()),
                    )
                    .await
                {
                    Ok((_assets, page_body)) => Ok(page_body),
                    Err(page_error) => {
                        debug!(
                            provider,
                            app_path = %locator.app_path,
                            response_id = followup_target.response_id(),
                            conversation_id = followup_target.conversation_id(),
                            error = %summarize_gateway_error(&page_error),
                            "gemini canvas media concrete page poll after video completion failure also missed asset"
                        );
                        let completion_summary = summarize_gateway_error(&error);
                        Err(append_gateway_error_summary(
                            page_error,
                            "video_completion_followup_failure",
                            Some(&completion_summary),
                        ))
                    }
                }
            }
        }
    }

    async fn execute_gemini_canvas_video_completion_attempt(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        attempt: usize,
        source_path: &str,
        conversation_id: &str,
        response_id: &str,
        requests: &GeminiCanvasVideoCompletionRequests,
        model_header: &str,
        remaining: Duration,
    ) -> Result<GeminiCanvasVideoCompletionAttemptOutcome, GatewayError> {
        let completion_body = self
            .send_gemini_canvas_text_batchexecute_request(
                payload,
                model,
                &requests.completion_request,
                session,
                model_header,
                remaining.min(Duration::from_secs(60)),
            )
            .await?;

        let completion = match classify_gemini_canvas_video_stage_body(completion_body) {
            Ok(body) => return Ok(GeminiCanvasVideoCompletionAttemptOutcome::Completed(body)),
            Err(progress) => progress,
        };
        let metadata_body = self
            .try_fetch_gemini_canvas_video_metadata_body(
                payload,
                model,
                session,
                attempt,
                source_path,
                conversation_id,
                response_id,
                &requests.metadata_request,
                model_header,
                remaining,
            )
            .await;
        if let Some(metadata_body) = metadata_body {
            let metadata = match classify_gemini_canvas_video_stage_body(metadata_body) {
                Ok(body) => return Ok(GeminiCanvasVideoCompletionAttemptOutcome::Completed(body)),
                Err(progress) => progress,
            };
            Ok(GeminiCanvasVideoCompletionAttemptOutcome::Continue(
                GeminiCanvasVideoCompletionAttemptState {
                    completion,
                    metadata: Some(metadata),
                },
            ))
        } else {
            Ok(GeminiCanvasVideoCompletionAttemptOutcome::Continue(
                GeminiCanvasVideoCompletionAttemptState {
                    completion,
                    metadata: None,
                },
            ))
        }
    }

    async fn try_fetch_gemini_canvas_video_metadata_body(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        attempt: usize,
        source_path: &str,
        conversation_id: &str,
        response_id: &str,
        metadata_request: &gemini_web::GeminiWebRequest,
        model_header: &str,
        remaining: Duration,
    ) -> Option<String> {
        let provider = "gemini_canvas_compatible";
        match self
            .send_gemini_canvas_text_batchexecute_request(
                payload,
                model,
                metadata_request,
                session,
                model_header,
                remaining.min(Duration::from_secs(20)),
            )
            .await
        {
            Ok(body) => Some(body),
            Err(error) => {
                debug!(
                    provider,
                    attempt,
                    source_path,
                    conversation_id,
                    response_id,
                    error = %summarize_gateway_error(&error),
                    "gemini canvas video MUAZcd metadata follow-up failed"
                );
                None
            }
        }
    }

    async fn try_send_gemini_canvas_video_job_poll(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        attempt: usize,
        source_path: &str,
        conversation_id: &str,
        model_header: &str,
        remaining: Duration,
        job_poll_request: Option<&Result<gemini_web::GeminiWebRequest, GatewayError>>,
    ) -> Option<String> {
        let provider = "gemini_canvas_compatible";
        let Some(job_poll_request) = job_poll_request else {
            return None;
        };
        match job_poll_request {
            Ok(request) => match self
                .send_gemini_canvas_text_batchexecute_request(
                    payload,
                    model,
                    request,
                    session,
                    model_header,
                    remaining.min(Duration::from_secs(30)),
                )
                .await
            {
                Ok(body) => Some(body),
                Err(error) => {
                    debug!(
                        provider,
                        attempt,
                        source_path,
                        conversation_id,
                        error = %summarize_gateway_error(&error),
                        "gemini canvas video kwDCne job poll failed"
                    );
                    None
                }
            },
            Err(error) => {
                debug!(
                    provider,
                    attempt,
                    source_path,
                    conversation_id,
                    error = %summarize_gateway_error(error),
                    "gemini canvas video kwDCne request build failed"
                );
                None
            }
        }
    }

    async fn try_recover_gemini_canvas_video_locator_from_conversation_list(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        source_path: &str,
        model_header: &str,
        prompt: &str,
        request_started_at: SystemTime,
        response_id_hint: Option<&str>,
        timeout: Duration,
    ) -> Result<Option<gemini_canvas::GeminiCanvasStreamGenerateLocator>, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let response_id = response_id_hint
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if response_id.is_none() {
            return Ok(None);
        }

        let probe_request =
            gemini_canvas::build_conversation_list_probe_request(bootstrap, source_path)?;
        let full_request =
            gemini_canvas::build_conversation_list_full_request(bootstrap, source_path)?;
        let mut session = session.clone();
        let mut bodies = Vec::new();

        for (label, request) in [("probe", &probe_request), ("full", &full_request)] {
            match self
                .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                    payload,
                    model,
                    request,
                    &mut session,
                    model_header,
                    timeout
                        .min(Duration::from_secs(20))
                        .max(Duration::from_secs(8)),
                )
                .await
            {
                Ok(body) => bodies.push((label, body)),
                Err(error) => {
                    debug!(
                        provider,
                        rpc = label,
                        app_path = %source_path,
                        error = %summarize_gateway_error(&error),
                        "gemini canvas video conversation-list locator recovery request failed"
                    );
                }
            }
        }

        let mut entries = Vec::new();
        for (_, body) in &bodies {
            entries.extend(gemini_canvas::extract_conversation_list_entries(body));
        }
        if entries.is_empty() {
            return Ok(None);
        }

        let Some(selected_entry) =
            select_gemini_canvas_recent_conversation_entry(&entries, prompt, request_started_at)
        else {
            return Ok(None);
        };
        let Some(app_path) = selected_entry.app_path() else {
            return Ok(None);
        };

        debug!(
            provider,
            app_path = %app_path,
            conversation_id = %selected_entry.conversation_id,
            response_id = %response_id.as_deref().unwrap_or("<none>"),
            entries = %preview_gemini_canvas_conversation_entries(&entries, 5),
            "recovered gemini canvas video locator from conversation list"
        );

        Ok(Some(gemini_canvas::GeminiCanvasStreamGenerateLocator {
            response_id: response_id.unwrap(),
            conversation_id: selected_entry.conversation_id.clone(),
            app_path,
        }))
    }

    async fn try_recover_gemini_canvas_recent_conversation_page_url(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        source_path: &str,
        model_header: &str,
        prompt: &str,
        request_started_at: SystemTime,
        timeout: Duration,
    ) -> Result<Option<String>, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let probe_request =
            gemini_canvas::build_conversation_list_probe_request(bootstrap, source_path)?;
        let full_request =
            gemini_canvas::build_conversation_list_full_request(bootstrap, source_path)?;
        let mut session = session.clone();
        let mut bodies = Vec::new();

        for (label, request) in [("probe", &probe_request), ("full", &full_request)] {
            match self
                .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                    payload,
                    model,
                    request,
                    &mut session,
                    model_header,
                    timeout
                        .min(Duration::from_secs(20))
                        .max(Duration::from_secs(8)),
                )
                .await
            {
                Ok(body) => bodies.push((label, body)),
                Err(error) => {
                    debug!(
                        provider,
                        rpc = label,
                        app_path = %source_path,
                        error = %summarize_gateway_error(&error),
                        "gemini canvas music conversation-list recovery request failed"
                    );
                }
            }
        }

        let mut entries = Vec::new();
        for (_, body) in &bodies {
            entries.extend(gemini_canvas::extract_conversation_list_entries(body));
        }
        if entries.is_empty() {
            return Ok(None);
        }

        let Some(selected_entry) =
            select_gemini_canvas_recent_conversation_entry(&entries, prompt, request_started_at)
        else {
            return Ok(None);
        };
        let Some(app_path) = selected_entry.app_path() else {
            return Ok(None);
        };
        let app_url = format!("{}{}", payload.base_url.trim_end_matches('/'), app_path);

        debug!(
            provider,
            app_url = %app_url,
            conversation_id = %selected_entry.conversation_id,
            response_id = %selected_entry.response_id.as_deref().unwrap_or("<none>"),
            entries = %preview_gemini_canvas_conversation_entries(&entries, 5),
            "recovered gemini canvas recent conversation page for music asset follow-up"
        );

        Ok(Some(app_url))
    }

    async fn execute_gemini_canvas_image_edit_post_ack_followups(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        stream_body: &str,
        timeout: Duration,
        edit_context: &mut GeminiCanvasImageEditFollowupContext,
    ) -> Result<GeminiCanvasImageEditPostAckFollowupResult, GatewayError> {
        let app_url = edit_context
            .signaler_app_url
            .clone()
            .ok_or_else(gemini_canvas_image_edit_post_ack_missing_app_url_error)?;
        let response_id = edit_context
            .signaler_response_id
            .clone()
            .or_else(|| gemini_canvas::extract_stream_generate_response_id(stream_body).ok());
        let base_url = payload.base_url.trim_end_matches('/');
        let bootstrap_page_url = format!("{base_url}{}", gemini_web::GEMINI_WEB_DEFAULT_APP_PATH);
        let source_path = gemini_web::GEMINI_WEB_DEFAULT_APP_PATH.to_string();
        let storage_state = gateway_object_storage()?
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
        let mut session = if let Some(existing_session) = edit_context.signaler_session.clone() {
            existing_session
        } else {
            gemini_canvas::storage_state_to_pure_http_session(
                &storage_state,
                &app_url,
                base_url,
                &auth_user,
            )?
        };
        let fallback_bootstrap =
            gemini_web::bootstrap_from_payload_cache(payload.extra_body.as_ref());
        let bootstrap_html = self
            .fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale(
                payload,
                &mut session,
                &bootstrap_page_url,
                timeout
                    .min(Duration::from_secs(20))
                    .max(Duration::from_secs(8)),
                edit_context.locale_hint.as_deref(),
            )
            .await
            .ok();
        let mut bootstrap = if let Some(html) = bootstrap_html.as_deref() {
            gemini_web::parse_bootstrap_from_app_html(html, Some("en"))
                .or_else(|primary_error| fallback_bootstrap.clone().ok_or(primary_error))?
        } else {
            fallback_bootstrap
                .clone()
                .ok_or_else(gemini_canvas_image_edit_post_ack_bootstrap_missing_error)?
        };
        bootstrap =
            gemini_web::merge_bootstrap_from_fallback(bootstrap, fallback_bootstrap.as_ref());
        if let Some(locale) = gemini_canvas::harvest_image_edit_template_locale(&storage_state) {
            bootstrap.language = locale;
        }

        let batchexecute_header_id = Some(gemini_canvas::new_batchexecute_header_id());
        let trigger_model_header = gemini_canvas::build_text_batchexecute_model_header(
            None,
            batchexecute_header_id.as_deref(),
        );
        let followup_model_header = gemini_canvas::build_text_batchexecute_model_header(
            Some(gemini_canvas::GEMINI_CANVAS_TEXT_BOOTSTRAP_MODEL_ID),
            batchexecute_header_id.as_deref(),
        );

        let trigger_body = if let Some(response_id) = response_id.as_deref() {
            let trigger_request =
                gemini_canvas::build_tts_trigger_request(response_id, &bootstrap, &source_path)?;
            self.send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                payload,
                model,
                &trigger_request,
                &mut session,
                &trigger_model_header,
                timeout
                    .min(Duration::from_secs(15))
                    .max(Duration::from_secs(6)),
            )
            .await
            .map_err(|error| {
                append_gateway_error_summary(error, "image_edit_post_ack_stage", Some("PCck7e"))
            })?
        } else {
            "<skipped: missing response id>".to_string()
        };

        let followup_request =
            gemini_canvas::build_text_bootstrap_preflight_request(&bootstrap, &source_path)?;
        let first_followup_body = self
            .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                payload,
                model,
                &followup_request,
                &mut session,
                &followup_model_header,
                timeout
                    .min(Duration::from_secs(15))
                    .max(Duration::from_secs(6)),
            )
            .await
            .map_err(|error| {
                append_gateway_error_summary(
                    error,
                    "image_edit_post_ack_stage",
                    Some("aPya6c:first"),
                )
            })?;

        sleep(Duration::from_secs(16)).await;

        let activity_request = gemini_canvas::build_text_batchexecute_request(
            "ESY5D",
            json!([[["bard_activity_enabled"]]]),
            &bootstrap,
            &source_path,
        )?;
        let activity_body = self
            .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                payload,
                model,
                &activity_request,
                &mut session,
                &trigger_model_header,
                timeout
                    .min(Duration::from_secs(20))
                    .max(Duration::from_secs(8)),
            )
            .await
            .map_err(|error| {
                append_gateway_error_summary(error, "image_edit_post_ack_stage", Some("ESY5D"))
            })?;

        let second_followup_body = self
            .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                payload,
                model,
                &followup_request,
                &mut session,
                &followup_model_header,
                timeout
                    .min(Duration::from_secs(20))
                    .max(Duration::from_secs(8)),
            )
            .await
            .map_err(|error| {
                append_gateway_error_summary(
                    error,
                    "image_edit_post_ack_stage",
                    Some("aPya6c:second"),
                )
            })?;

        let mut candidate_page_urls = vec![app_url.clone()];
        for candidate in &edit_context.signaler_app_urls {
            if !candidate_page_urls.iter().any(|value| value == candidate) {
                candidate_page_urls.push(candidate.clone());
            }
        }
        let mut page_body = None;
        let mut page_previews = Vec::new();
        for candidate_page_url in &candidate_page_urls {
            match self
                .fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale(
                    payload,
                    &mut session,
                    candidate_page_url,
                    timeout
                        .min(Duration::from_secs(20))
                        .max(Duration::from_secs(8)),
                    edit_context.locale_hint.as_deref(),
                )
                .await
            {
                Ok(body) => {
                    let preview = compact_response_preview(&body, 120);
                    page_previews.push(format!("{candidate_page_url}={preview}"));
                    if gemini_canvas::extract_page_blob_media_assets(
                        &body,
                        gemini_canvas::GeminiCanvasMediaOperation::Image,
                    )
                    .is_ok()
                    {
                        page_body = Some(body);
                        break;
                    }
                }
                Err(error) => {
                    page_previews.push(format!(
                        "{candidate_page_url}=<fetch failed: {}>",
                        summarize_gateway_error(&error)
                    ));
                }
            }
        }
        let page_preview = if page_previews.is_empty() {
            "<no page fetch>".to_string()
        } else {
            page_previews.join(" || ")
        };

        edit_context.signaler_session = Some(session);

        Ok(GeminiCanvasImageEditPostAckFollowupResult {
            preview: format!(
                "PCck7e={} | aPya6c.first={} | ESY5D={} | aPya6c.second={} | page={}",
                compact_response_preview(&trigger_body, 160),
                compact_response_preview(&first_followup_body, 160),
                compact_response_preview(&activity_body, 160),
                compact_response_preview(&second_followup_body, 160),
                page_preview,
            ),
            page_body,
        })
    }

    async fn poll_gemini_canvas_image_edit_conversation_assets(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        timeout: Duration,
        edit_context: &mut GeminiCanvasImageEditFollowupContext,
    ) -> Result<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String), GatewayError> {
        let base_url = payload.base_url.trim_end_matches('/');
        let app_url = format!("{base_url}{}", gemini_web::GEMINI_WEB_DEFAULT_APP_PATH);
        let bootstrap_page_url = app_url.clone();
        let has_concrete_signaler_page = edit_context.signaler_app_url.is_some();
        append_gemini_canvas_image_edit_trace(
            "conversation.bootstrap",
            bootstrap_page_url.as_str(),
        );
        let storage_state = gateway_object_storage()?
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
        // Image-edit completion appears to depend on the same page-owned
        // lifecycle that the signaler lane has already advanced. Reuse that
        // refreshed session when available instead of rebooting follow-up from
        // the original imported storage state.
        let mut session = if let Some(existing_session) = edit_context.signaler_session.clone() {
            existing_session
        } else {
            gemini_canvas::storage_state_to_pure_http_session(
                &storage_state,
                &bootstrap_page_url,
                base_url,
                &auth_user,
            )?
        };
        let fallback_bootstrap =
            gemini_web::bootstrap_from_payload_cache(payload.extra_body.as_ref());
        let bootstrap_html = self
            .fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale(
                payload,
                &mut session,
                &bootstrap_page_url,
                timeout,
                edit_context.locale_hint.as_deref(),
            )
            .await
            .ok();
        let bootstrap_page = if bootstrap_html.is_some() {
            bootstrap_page_url.as_str()
        } else {
            "<payload-cache>"
        };
        let mut bootstrap = if let Some(html) = bootstrap_html.as_deref() {
            gemini_web::parse_bootstrap_from_app_html(html, Some("en"))
                .or_else(|primary_error| fallback_bootstrap.clone().ok_or(primary_error))?
        } else {
            fallback_bootstrap
                .clone()
                .ok_or_else(gemini_canvas_image_edit_conversation_bootstrap_missing_error)?
        };
        bootstrap =
            gemini_web::merge_bootstrap_from_fallback(bootstrap, fallback_bootstrap.as_ref());
        if let Some(locale) = gemini_canvas::harvest_image_edit_template_locale(&storage_state) {
            bootstrap.language = locale;
        }

        // Concrete signaler pages are useful for direct page fetches and
        // hNvQHb completion requests, but the conversation-list probes in the
        // successful /app capture shape are rooted at the generic app surface.
        // Reusing /app/<id> as source-path for MaZiqc/aPya6c/L5adhe has been
        // yielding only generic [7] frames with no recoverable entries.
        let conversation_list_source_path = gemini_web::GEMINI_WEB_DEFAULT_APP_PATH.to_string();
        let mode_index = gemini_canvas::stream_generate_mode_index(
            gemini_canvas::GeminiCanvasMediaOperation::Image,
        );
        // Successful image-edit follow-up captures use a fresh batchexecute header id
        // instead of reusing the stale id embedded in the imported StreamGenerate template.
        let batchexecute_header_id = Some(gemini_canvas::new_batchexecute_header_id());
        let neutral_header = gemini_canvas::build_text_batchexecute_model_header_variant(
            None,
            batchexecute_header_id.as_deref(),
            false,
            false,
        );
        let empty_model_header = gemini_canvas::build_text_batchexecute_model_header_variant(
            None,
            batchexecute_header_id.as_deref(),
            false,
            true,
        );
        let completion_model_header = gemini_canvas::build_text_batchexecute_model_header_variant(
            Some(gemini_canvas::GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID),
            batchexecute_header_id.as_deref(),
            true,
            false,
        );
        let probe_request = gemini_canvas::build_conversation_list_probe_request(
            &bootstrap,
            &conversation_list_source_path,
        )?;
        let full_request = gemini_canvas::build_conversation_list_full_request(
            &bootstrap,
            &conversation_list_source_path,
        )?;
        let bootstrap_request = gemini_canvas::build_text_bootstrap_preflight_request(
            &bootstrap,
            &conversation_list_source_path,
        )?;
        let mode_selection_request = gemini_canvas::build_text_mode_selection_preflight_request(
            &bootstrap,
            &conversation_list_source_path,
        )?;

        let started_at = Instant::now();
        let total_budget = if has_concrete_signaler_page {
            // Successful browser captures settle image-edit assets in the primary
            // StreamGenerate response within seconds, not minutes. Once we have
            // already fallen back to generic conversation probing, keeping this
            // lane alive for the old ~90s budget only pushes caller-visible HTTP
            // requests into the outer transport timeout without surfacing any
            // better signal than repeated generic [7] frames.
            timeout
                .min(Duration::from_secs(60))
                .max(Duration::from_secs(30))
        } else {
            timeout
                .min(Duration::from_secs(30))
                .max(Duration::from_secs(12))
        };
        let mut attempt = 0usize;
        let mut failures = Vec::new();
        let mut last_entries_preview = "<none>".to_string();
        let mut last_completion_preview = "<none>".to_string();
        let mut last_probe_preview = "<none>".to_string();
        let mut last_full_preview = "<none>".to_string();
        let mut parity_probe_preview = "<none>".to_string();
        let mut parity_full_preview = "<none>".to_string();
        let mut parity_o30_preview = "<none>".to_string();
        let mut parity_k4_preview = "<none>".to_string();

        loop {
            let Some(remaining) = total_budget.checked_sub(started_at.elapsed()) else {
                break;
            };
            if remaining <= Duration::from_secs(1) {
                break;
            }
            attempt += 1;

            if attempt == 1 {
                if has_concrete_signaler_page {
                    if let Err(error) = self
                        .execute_gemini_canvas_page_init_preflight_sequence(
                            payload,
                            model,
                            &bootstrap,
                            &conversation_list_source_path,
                            &session,
                            remaining.min(Duration::from_secs(20)),
                            true,
                        )
                        .await
                    {
                        failures.push(format!(
                            "attempt={} page_init={} source_path={}",
                            attempt,
                            summarize_gateway_error(&error),
                            conversation_list_source_path
                        ));
                    }
                    let capture_aligned_steps = [
                        ("fast_full", &full_request, neutral_header.as_str()),
                        (
                            "fast_bootstrap",
                            &bootstrap_request,
                            empty_model_header.as_str(),
                        ),
                        (
                            "fast_mode_selection",
                            &mode_selection_request,
                            neutral_header.as_str(),
                        ),
                        ("fast_probe", &probe_request, neutral_header.as_str()),
                    ];
                    let mut fast_full_body: Option<String> = None;
                    let mut fast_probe_body: Option<String> = None;
                    for (label, request, header) in capture_aligned_steps {
                        match self
                            .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                                payload,
                                model,
                                request,
                                &mut session,
                                header,
                                remaining.min(Duration::from_secs(12)),
                            )
                            .await
                        {
                            Ok(body) => {
                                let preview = compact_response_preview(&body, 240);
                                match label {
                                    "fast_full" => {
                                        last_full_preview = preview.clone();
                                        fast_full_body = Some(body);
                                    }
                                    "fast_probe" => {
                                        last_probe_preview = preview.clone();
                                        fast_probe_body = Some(body);
                                    }
                                    _ => {
                                        failures.push(format!(
                                            "attempt={} {}={}",
                                            attempt, label, preview
                                        ));
                                    }
                                }
                            }
                            Err(error) => failures.push(format!(
                                "attempt={} {}={}",
                                attempt,
                                label,
                                summarize_gateway_error(&error)
                            )),
                        }
                    }

                    let fast_entries = fast_full_body
                        .as_deref()
                        .map(gemini_canvas::extract_conversation_list_entries)
                        .filter(|entries| !entries.is_empty())
                        .or_else(|| {
                            fast_probe_body
                                .as_deref()
                                .map(gemini_canvas::extract_conversation_list_entries)
                                .filter(|entries| !entries.is_empty())
                        })
                        .unwrap_or_default();
                    if !fast_entries.is_empty() {
                        last_entries_preview =
                            preview_gemini_canvas_conversation_entries(&fast_entries, 5);
                        if let Some(selected_entry) = select_gemini_canvas_recent_conversation_entry(
                            &fast_entries,
                            &edit_context.prompt,
                            edit_context.request_started_at,
                        ) {
                            let app_path = selected_entry.app_path().unwrap_or_else(|| {
                                format!(
                                    "/app/{}",
                                    selected_entry.conversation_id.trim_start_matches("c_")
                                )
                            });
                            let completion_request =
                                gemini_canvas::build_video_completion_followup_request(
                                    &bootstrap,
                                    &app_path,
                                    &selected_entry.conversation_id,
                                )?;
                            match self
                                .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                                    payload,
                                    model,
                                    &completion_request,
                                    &mut session,
                                    &completion_model_header,
                                    remaining.min(Duration::from_secs(20)),
                                )
                                .await
                            {
                                Ok(body) => {
                                    last_completion_preview =
                                        compact_response_preview(&body, 320);
                                    if let Ok(assets) =
                                        gemini_canvas::extract_stream_generate_media_assets(
                                            &body,
                                            gemini_canvas::GeminiCanvasMediaOperation::Image,
                                        )
                                    {
                                        return Ok((assets, body));
                                    }
                                    if let Ok(assets) =
                                        gemini_canvas::extract_page_blob_media_assets(
                                            &body,
                                            gemini_canvas::GeminiCanvasMediaOperation::Image,
                                        )
                                    {
                                        return Ok((assets, body));
                                    }
                                }
                                Err(error) => failures.push(format!(
                                    "attempt={} fast_completion={}",
                                    attempt,
                                    summarize_gateway_error(&error)
                                )),
                            }
                        }
                    }
                }

                match self
                    .execute_gemini_canvas_media_capture_parity_preflight_sequence(
                        payload,
                        model,
                        &bootstrap,
                        &conversation_list_source_path,
                        &mut session,
                        mode_index,
                        batchexecute_header_id.as_deref(),
                        true,
                        remaining.min(Duration::from_secs(30)),
                    )
                    .await
                {
                    Ok(parity) => {
                        if let Some(body) = parity.maziqc_probe_body.as_deref() {
                            parity_probe_preview = compact_response_preview(body, 240);
                            last_probe_preview = parity_probe_preview.clone();
                        }
                        if let Some(body) = parity.maziqc_full_body.as_deref() {
                            parity_full_preview = compact_response_preview(body, 240);
                            last_full_preview = parity_full_preview.clone();
                        }
                        if let Some(body) = parity.o30o0e_body.as_deref() {
                            parity_o30_preview = compact_response_preview(body, 240);
                        }
                        if let Some(body) = parity.k4wwud_body.as_deref() {
                            parity_k4_preview = compact_response_preview(body, 240);
                        }

                        let parity_entries = parity
                            .maziqc_full_body
                            .as_deref()
                            .map(gemini_canvas::extract_conversation_list_entries)
                            .filter(|entries| !entries.is_empty())
                            .or_else(|| {
                                parity
                                    .maziqc_probe_body
                                    .as_deref()
                                    .map(gemini_canvas::extract_conversation_list_entries)
                                    .filter(|entries| !entries.is_empty())
                            })
                            .unwrap_or_default();
                        if !parity_entries.is_empty() {
                            last_entries_preview =
                                preview_gemini_canvas_conversation_entries(&parity_entries, 5);
                            if let Some(selected_entry) =
                                select_gemini_canvas_recent_conversation_entry(
                                    &parity_entries,
                                    &edit_context.prompt,
                                    edit_context.request_started_at,
                                )
                            {
                                let app_path = selected_entry.app_path().unwrap_or_else(|| {
                                    format!(
                                        "/app/{}",
                                        selected_entry.conversation_id.trim_start_matches("c_")
                                    )
                                });
                                let completion_request =
                                    gemini_canvas::build_video_completion_followup_request(
                                        &bootstrap,
                                        &app_path,
                                        &selected_entry.conversation_id,
                                    )?;
                                match self
                                    .send_gemini_canvas_text_batchexecute_request_capture_aligned(
                                        payload,
                                        model,
                                        &completion_request,
                                        &session,
                                        &completion_model_header,
                                        remaining.min(Duration::from_secs(30)),
                                    )
                                    .await
                                {
                                    Ok(body) => {
                                        last_completion_preview =
                                            compact_response_preview(&body, 320);
                                        if let Ok(assets) =
                                            gemini_canvas::extract_stream_generate_media_assets(
                                                &body,
                                                gemini_canvas::GeminiCanvasMediaOperation::Image,
                                            )
                                        {
                                            return Ok((assets, body));
                                        }
                                        if let Ok(assets) =
                                            gemini_canvas::extract_page_blob_media_assets(
                                                &body,
                                                gemini_canvas::GeminiCanvasMediaOperation::Image,
                                            )
                                        {
                                            return Ok((assets, body));
                                        }
                                        failures.push(format!(
                                            "attempt={} parity_conversation={} title={} hNvQHb_missing_asset",
                                            attempt,
                                            selected_entry.conversation_id,
                                            truncate_response_preview(
                                                selected_entry.title.as_str(),
                                                80
                                            )
                                        ));
                                    }
                                    Err(error) => {
                                        failures.push(format!(
                                            "attempt={} parity_conversation={} hNvQHb={}",
                                            attempt,
                                            selected_entry.conversation_id,
                                            summarize_gateway_error(&error)
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    Err(error) => {
                        failures.push(format!(
                            "attempt={} parity_preflight={}",
                            attempt,
                            summarize_gateway_error(&error)
                        ));
                    }
                }
            }

            let probe_body = match self
                .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                    payload,
                    model,
                    &probe_request,
                    &mut session,
                    &neutral_header,
                    remaining.min(Duration::from_secs(20)),
                )
                .await
            {
                Ok(body) => {
                    last_probe_preview = compact_response_preview(&body, 240);
                    Some(body)
                }
                Err(error) => {
                    failures.push(format!(
                        "attempt={} mazqic_probe={}",
                        attempt,
                        summarize_gateway_error(&error)
                    ));
                    None
                }
            };

            let full_body = match self
                .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                    payload,
                    model,
                    &full_request,
                    &mut session,
                    &neutral_header,
                    remaining.min(Duration::from_secs(20)),
                )
                .await
            {
                Ok(body) => {
                    last_full_preview = compact_response_preview(&body, 240);
                    Some(body)
                }
                Err(error) => {
                    failures.push(format!(
                        "attempt={} mazqic_full={}",
                        attempt,
                        summarize_gateway_error(&error)
                    ));
                    None
                }
            };

            let entries = full_body
                .as_deref()
                .map(gemini_canvas::extract_conversation_list_entries)
                .filter(|entries| !entries.is_empty())
                .or_else(|| {
                    probe_body
                        .as_deref()
                        .map(gemini_canvas::extract_conversation_list_entries)
                        .filter(|entries| !entries.is_empty())
                })
                .unwrap_or_default();

            if !entries.is_empty() {
                last_entries_preview = preview_gemini_canvas_conversation_entries(&entries, 5);
            }

            let Some(selected_entry) = select_gemini_canvas_recent_conversation_entry(
                &entries,
                &edit_context.prompt,
                edit_context.request_started_at,
            ) else {
                failures.push(format!(
                    "attempt={} no_recent_conversation probe_preview={} full_preview={} entries={}",
                    attempt, last_probe_preview, last_full_preview, last_entries_preview
                ));
                if total_budget
                    .checked_sub(started_at.elapsed())
                    .is_some_and(|time_left| time_left > Duration::from_secs(3))
                {
                    sleep(Duration::from_secs(2)).await;
                    continue;
                }
                break;
            };

            let app_path = selected_entry.app_path().unwrap_or_else(|| {
                format!(
                    "/app/{}",
                    selected_entry.conversation_id.trim_start_matches("c_")
                )
            });
            let completion_request = gemini_canvas::build_video_completion_followup_request(
                &bootstrap,
                &app_path,
                &selected_entry.conversation_id,
            )?;
            match self
                .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                    payload,
                    model,
                    &completion_request,
                    &mut session,
                    &completion_model_header,
                    remaining.min(Duration::from_secs(30)),
                )
                .await
            {
                Ok(body) => {
                    last_completion_preview = compact_response_preview(&body, 320);
                    if let Ok(assets) = gemini_canvas::extract_stream_generate_media_assets(
                        &body,
                        gemini_canvas::GeminiCanvasMediaOperation::Image,
                    ) {
                        return Ok((assets, body));
                    }
                    if let Ok(assets) = gemini_canvas::extract_page_blob_media_assets(
                        &body,
                        gemini_canvas::GeminiCanvasMediaOperation::Image,
                    ) {
                        return Ok((assets, body));
                    }
                    failures.push(format!(
                        "attempt={} conversation={} title={} hNvQHb_missing_asset",
                        attempt,
                        selected_entry.conversation_id,
                        truncate_response_preview(selected_entry.title.as_str(), 80)
                    ));
                }
                Err(error) => {
                    failures.push(format!(
                        "attempt={} conversation={} hNvQHb={}",
                        attempt,
                        selected_entry.conversation_id,
                        summarize_gateway_error(&error)
                    ));
                }
            }

            if total_budget
                .checked_sub(started_at.elapsed())
                .is_some_and(|time_left| time_left > Duration::from_secs(3))
            {
                sleep(Duration::from_secs(2)).await;
            }
        }

        let prompt_preview = truncate_response_preview(edit_context.prompt.as_str(), 120);
        let failure_summary = failures.join(" | ");
        Err(gemini_canvas_image_edit_conversation_followup_failed_error(
            &bootstrap_page,
            attempt,
            &prompt_preview,
            &last_entries_preview,
            &last_probe_preview,
            &last_full_preview,
            &last_completion_preview,
            &parity_probe_preview,
            &parity_full_preview,
            &parity_o30_preview,
            &parity_k4_preview,
            &failure_summary,
        ))
    }

    async fn poll_gemini_canvas_media_assets_from_conversation_page(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        stream_body: &str,
        timeout: Duration,
        force_root_app_followup: bool,
        locale_override: Option<&str>,
        session_override: Option<&gemini_canvas::GeminiCanvasPureHttpSession>,
        bootstrap_page_url_override: Option<&str>,
    ) -> Result<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String), GatewayError> {
        let provider = "gemini_canvas_compatible";
        let page_seed = prepare_gemini_canvas_page_seed(
            payload,
            runtime,
            operation,
            stream_body,
            force_root_app_followup,
            bootstrap_page_url_override,
            true,
            provider,
            "gemini canvas page poll",
        )?;
        let locator = page_seed.locator;
        let base_url = payload.base_url.trim_end_matches('/');
        let app_bootstrap_url = page_seed.app_bootstrap_url;
        let share_bootstrap_url = page_seed.share_bootstrap_url;
        let has_bootstrap_override = page_seed.has_page_url_override;
        let locator_mode = classify_gemini_canvas_page_target_mode(
            force_root_app_followup,
            has_bootstrap_override,
            locator.is_some(),
            false,
        );
        let conversation_url = page_seed.conversation_page_url;
        if operation == gemini_canvas::GeminiCanvasMediaOperation::Image && force_root_app_followup
        {
            append_gemini_canvas_image_edit_trace(
                "page-poll.bootstrap",
                conversation_url
                    .as_deref()
                    .unwrap_or(app_bootstrap_url.as_str()),
            );
        }
        let storage_state = gateway_object_storage()?
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
        let mut session = if let Some(existing_session) = session_override {
            existing_session.clone()
        } else {
            gemini_canvas_pure_http_session_from_payload_or_storage(
                payload,
                &storage_state,
                conversation_url
                    .as_deref()
                    .unwrap_or(app_bootstrap_url.as_str()),
                base_url,
                &auth_user,
            )?
        };
        let poll_urls = build_gemini_canvas_page_poll_urls(
            force_root_app_followup,
            has_bootstrap_override,
            conversation_url.as_deref(),
            &app_bootstrap_url,
            &share_bootstrap_url,
        );
        let mut failures = Vec::new();
        let mut last_page_preview = None;
        let started_at = Instant::now();
        let page_poll_timing = plan_gemini_canvas_conversation_page_poll_timing(
            operation,
            has_bootstrap_override,
            force_root_app_followup,
            timeout,
        );
        let poll_budget = page_poll_timing.poll_budget;
        let sleep_between_attempts = page_poll_timing.sleep_between_attempts;
        let max_fetch_timeout = page_poll_timing.max_fetch_timeout;
        let mut attempt_count = 0usize;

        loop {
            let Some(remaining) = resolve_gemini_canvas_conversation_page_poll_remaining(
                poll_budget,
                started_at.elapsed(),
            ) else {
                break;
            };
            attempt_count += 1;
            if let Some(refresh_page_url) =
                resolve_gemini_canvas_conversation_page_poll_refresh_target(
                    operation,
                    has_bootstrap_override,
                    force_root_app_followup,
                    conversation_url.as_deref(),
                    app_bootstrap_url.as_str(),
                )
            {
                match self
                    .trigger_gemini_canvas_image_page_refresh(
                        payload,
                        model,
                        &storage_state,
                        &mut session,
                        refresh_page_url,
                        remaining.min(Duration::from_secs(20)),
                    )
                    .await
                {
                    Ok(body) => {
                        failures.push(gemini_canvas_conversation_page_poll_refresh_body_entry(
                            attempt_count,
                            &body,
                        ));
                    }
                    Err(error) => {
                        failures.push(gemini_canvas_conversation_page_poll_refresh_error_entry(
                            attempt_count,
                            &error,
                        ));
                    }
                }
            }
            for page_url in &poll_urls {
                let Some(remaining_for_fetch) =
                    resolve_gemini_canvas_conversation_page_poll_remaining(
                        poll_budget,
                        started_at.elapsed(),
                    )
                else {
                    break;
                };
                match self
                    .fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale(
                        payload,
                        &mut session,
                        page_url,
                        remaining_for_fetch.min(max_fetch_timeout),
                        locale_override,
                    )
                    .await
                {
                    Ok(page_body) => {
                        match try_extract_gemini_canvas_conversation_page_poll_assets_from_body(
                            attempt_count,
                            page_url,
                            operation,
                            page_body,
                        ) {
                            Ok(result) => return Ok(result),
                            Err(failure) => {
                                last_page_preview = Some(failure.page_preview);
                                failures.push(failure.failure_entry);
                            }
                        }
                    }
                    Err(error) => {
                        failures.push(gemini_canvas_conversation_page_poll_fetch_error_entry(
                            attempt_count,
                            page_url,
                            &error,
                        ));
                    }
                }
            }

            if should_sleep_after_gemini_canvas_conversation_page_poll_attempt(
                poll_budget,
                started_at.elapsed(),
                sleep_between_attempts,
            ) {
                sleep(sleep_between_attempts).await;
                continue;
            }
            break;
        }

        let app_path = locator
            .as_ref()
            .map(|locator| locator.app_path.as_str())
            .unwrap_or(gemini_web::GEMINI_WEB_DEFAULT_APP_PATH);
        let failure_summary = failures.join(" | ");
        let last_page_preview_text = last_page_preview.unwrap_or_else(|| "<none>".to_string());
        Err(gemini_canvas_conversation_page_missing_asset_error(
            operation,
            locator_mode,
            app_path,
            attempt_count,
            poll_budget.as_secs(),
            &failure_summary,
            &last_page_preview_text,
        ))
    }

    async fn trigger_gemini_canvas_image_page_refresh(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        storage_state: &Value,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        app_url: &str,
        timeout: Duration,
    ) -> Result<String, GatewayError> {
        let fallback_bootstrap =
            gemini_web::bootstrap_from_payload_cache(payload.extra_body.as_ref());
        let locale_override = gemini_canvas::harvest_image_edit_template_locale(storage_state);
        let bootstrap_html = self
            .fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale(
                payload,
                session,
                app_url,
                timeout,
                locale_override.as_deref(),
            )
            .await
            .ok();
        let mut bootstrap = if let Some(html) = bootstrap_html.as_deref() {
            gemini_web::parse_bootstrap_from_app_html(html, Some("en"))
                .or_else(|primary_error| fallback_bootstrap.clone().ok_or(primary_error))?
        } else {
            fallback_bootstrap
                .clone()
                .ok_or_else(gemini_canvas_image_page_refresh_bootstrap_missing_error)?
        };
        bootstrap =
            gemini_web::merge_bootstrap_from_fallback(bootstrap, fallback_bootstrap.as_ref());
        if let Some(locale) = locale_override {
            bootstrap.language = locale;
        }
        let source_path = app_url
            .strip_prefix(payload.base_url.trim_end_matches('/'))
            .filter(|candidate| !candidate.trim().is_empty())
            .unwrap_or(gemini_web::GEMINI_WEB_DEFAULT_APP_PATH)
            .to_string();
        let mode_index = gemini_canvas::stream_generate_mode_index(
            gemini_canvas::GeminiCanvasMediaOperation::Image,
        );
        let batchexecute_header_id = Some(gemini_canvas::new_batchexecute_header_id());
        if let Err(error) = self
            .execute_gemini_canvas_page_init_preflight_sequence(
                payload,
                model,
                &bootstrap,
                &source_path,
                session,
                timeout.min(Duration::from_secs(20)),
                true,
            )
            .await
        {
            debug!(
                provider = "gemini_canvas_compatible",
                error = %summarize_gateway_error(&error),
                source_path = %source_path,
                "gemini canvas image page refresh page-init preflight failed; continuing with parity refresh"
            );
        }
        let parity = self
            .execute_gemini_canvas_media_capture_parity_preflight_sequence(
                payload,
                model,
                &bootstrap,
                &source_path,
                session,
                mode_index,
                batchexecute_header_id.as_deref(),
                true,
                timeout.min(Duration::from_secs(30)),
            )
            .await?;
        let mut stage_previews = vec![format!(
            "aPya6c={}",
            compact_response_preview(&parity.selected_bootstrap_body, 180)
        )];
        if let Some(body) = parity.o30o0e_body.as_deref() {
            stage_previews.push(format!("o30O0e={}", compact_response_preview(body, 180)));
        }
        if let Some(body) = parity.k4wwud_body.as_deref() {
            stage_previews.push(format!("K4WWud={}", compact_response_preview(body, 180)));
        }
        if let Some(body) = parity.maziqc_probe_body.as_deref() {
            stage_previews.push(format!(
                "MaZiqc.probe={}",
                compact_response_preview(body, 180)
            ));
        }
        if let Some(body) = parity.maziqc_full_body.as_deref() {
            stage_previews.push(format!(
                "MaZiqc.full={}",
                compact_response_preview(body, 180)
            ));
        }
        Ok(stage_previews.join(" | "))
    }

    async fn poll_gemini_canvas_direct_http_video_completion_body(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        source_path: &str,
        conversation_id: &str,
        response_id: &str,
        model_header: &str,
        primary_body: &str,
        timeout: Duration,
    ) -> Result<String, GatewayError> {
        let requests = build_gemini_canvas_video_completion_requests(
            bootstrap,
            source_path,
            conversation_id,
            response_id,
            primary_body,
        )?;
        let started_at = Instant::now();
        let total_budget = timeout
            .max(Duration::from_secs(45))
            .min(Duration::from_secs(120));
        let mut poll_state = GeminiCanvasVideoCompletionPollState::default();

        loop {
            let Some(remaining) = total_budget.checked_sub(started_at.elapsed()) else {
                break;
            };
            if remaining <= Duration::from_secs(5) {
                break;
            }
            let attempt = poll_state.next_attempt();

            if let Some(job_body) = self
                .try_send_gemini_canvas_video_job_poll(
                    payload,
                    model,
                    session,
                    attempt,
                    source_path,
                    conversation_id,
                    model_header,
                    remaining,
                    requests.job_poll_request.as_ref(),
                )
                .await
            {
                if let Ok(completed_body) = classify_gemini_canvas_video_stage_body(job_body) {
                    return Ok(completed_body);
                }
            }

            let attempt_state = match self
                .execute_gemini_canvas_video_completion_attempt(
                    payload,
                    model,
                    session,
                    attempt,
                    source_path,
                    conversation_id,
                    response_id,
                    &requests,
                    model_header,
                    remaining,
                )
                .await?
            {
                GeminiCanvasVideoCompletionAttemptOutcome::Completed(body) => return Ok(body),
                GeminiCanvasVideoCompletionAttemptOutcome::Continue(state) => state,
            };

            let next_sleep_for = poll_state.record_attempt_progress(&attempt_state);

            let Some(sleep_for) = next_sleep_for else {
                break;
            };
            if let Some(remaining_after_sleep) = total_budget.checked_sub(started_at.elapsed()) {
                if remaining_after_sleep > sleep_for + Duration::from_secs(2) {
                    sleep(sleep_for).await;
                    continue;
                }
            }
            break;
        }

        Err(poll_state.build_missing_asset_error(
            source_path,
            conversation_id,
            response_id,
            requests.job_id.as_deref(),
        ))
    }

    async fn execute_gemini_canvas_direct_http_image_json(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        timeout: Duration,
    ) -> Result<Value, GatewayError> {
        let preview_model = gemini_canvas::resolve_direct_http_image_model(model)?;
        let preview_base_url = gemini_canvas::direct_http_image_api_base_url(runtime);
        let google_api_base_url = runtime.api_base_url.trim_end_matches('/').to_string();
        let official_model = gemini_canvas::resolve_official_image_model(model)?;
        let is_edit_request = req.endpoint_kind == EndpointKind::ImagesEdits;
        let storage_state = gateway_object_storage()?
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
        let share_url = gemini_canvas::direct_http_referrer(
            payload.base_url.trim_end_matches('/'),
            &runtime.share_id,
        );
        let app_url = format!(
            "{}{}",
            payload.base_url.trim_end_matches('/'),
            gemini_web::GEMINI_WEB_DEFAULT_APP_PATH
        );
        let session = gemini_canvas::storage_state_to_pure_http_session(
            &storage_state,
            &share_url,
            payload.base_url.trim_end_matches('/'),
            &auth_user,
        )?;
        let mut api_key_candidates =
            gemini_canvas::direct_http_google_api_keys(payload, &storage_state);
        let (harvested_api_keys, page_harvest_probes) = self
            .harvest_gemini_canvas_direct_http_api_keys(payload, runtime, &session, timeout)
            .await;
        for harvested in harvested_api_keys {
            if api_key_candidates
                .iter()
                .any(|existing| existing == &harvested)
            {
                continue;
            }
            api_key_candidates.push(harvested);
        }
        if api_key_candidates.is_empty() {
            api_key_candidates = GEMINI_CANVAS_PUBLIC_PAGE_API_KEY_FALLBACKS
                .iter()
                .map(|candidate| (*candidate).to_string())
                .collect();
        }
        enum GeminiCanvasDirectHttpImageAttemptKind {
            GenerateContent,
            ImagenPredict,
        }

        struct GeminiCanvasDirectHttpImageAttempt {
            label: &'static str,
            request_url: String,
            request_body: Value,
            kind: GeminiCanvasDirectHttpImageAttemptKind,
            preserve_cross_origin_origin: bool,
            preserve_cross_origin_referer: bool,
            include_signed_headers: bool,
            signed_origin_override: Option<String>,
            referer_override: Option<String>,
        }

        fn normalize_generate_content_image_body(
            mut body: Value,
            keep_image_config: bool,
            response_modalities: Option<&[&str]>,
        ) -> Value {
            let Some(map) = body.as_object_mut() else {
                return body;
            };
            map.remove("tools");
            map.remove("toolConfig");
            map.remove("tool_config");
            map.remove("toolChoice");
            map.remove("systemInstruction");
            if let Some(config) = map
                .get_mut("generationConfig")
                .and_then(Value::as_object_mut)
            {
                config.remove("thinkingConfig");
                config.remove("responseMimeType");
                config.remove("response_mime_type");
                match response_modalities {
                    Some(values) => {
                        config.insert("responseModalities".to_string(), json!(values));
                    }
                    None => {
                        config.remove("responseModalities");
                    }
                }
                if !keep_image_config {
                    config.remove("imageConfig");
                }
                if config.is_empty() {
                    map.remove("generationConfig");
                }
            }
            body
        }

        let page_origin = gemini_canvas_http_origin(payload);
        let clients6_request_url = format!(
            "{}/models/{}:generateContent",
            preview_base_url.trim_end_matches('/'),
            preview_model
        );
        let preview_text_and_image_body =
            gemini_canvas::build_direct_http_image_request_body(req, preview_model);
        let preview_image_only_body = gemini_canvas::build_image_request_body(req, preview_model);
        let preview_contents_only_body =
            normalize_generate_content_image_body(preview_text_and_image_body.clone(), false, None);
        let preview_aspect_only_body =
            normalize_generate_content_image_body(preview_text_and_image_body.clone(), true, None);
        let preview_image_only_no_modalities_body =
            normalize_generate_content_image_body(preview_image_only_body.clone(), true, None);
        let mut attempts = vec![
            GeminiCanvasDirectHttpImageAttempt {
                label: "canvas_web_preview_signed_app_text_image",
                request_url: clients6_request_url.clone(),
                request_body: preview_text_and_image_body.clone(),
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: true,
                preserve_cross_origin_referer: true,
                include_signed_headers: true,
                signed_origin_override: Some(page_origin.clone()),
                referer_override: Some(app_url.clone()),
            },
            GeminiCanvasDirectHttpImageAttempt {
                label: "canvas_web_preview_signed_share_text_image",
                request_url: clients6_request_url.clone(),
                request_body: preview_text_and_image_body.clone(),
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: true,
                preserve_cross_origin_referer: true,
                include_signed_headers: true,
                signed_origin_override: Some(page_origin.clone()),
                referer_override: Some(share_url.clone()),
            },
            GeminiCanvasDirectHttpImageAttempt {
                label: "canvas_web_preview_signed_share_image_only",
                request_url: clients6_request_url.clone(),
                request_body: preview_image_only_body.clone(),
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: true,
                preserve_cross_origin_referer: true,
                include_signed_headers: true,
                signed_origin_override: Some(page_origin.clone()),
                referer_override: Some(share_url.clone()),
            },
            GeminiCanvasDirectHttpImageAttempt {
                label: "canvas_web_preview_plain_share_text_image",
                request_url: clients6_request_url.clone(),
                request_body: preview_text_and_image_body.clone(),
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: true,
                preserve_cross_origin_referer: true,
                include_signed_headers: false,
                signed_origin_override: None,
                referer_override: Some(share_url.clone()),
            },
            GeminiCanvasDirectHttpImageAttempt {
                label: "canvas_web_preview_plain_minimal_image_only",
                request_url: clients6_request_url.clone(),
                request_body: preview_image_only_no_modalities_body.clone(),
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: false,
                preserve_cross_origin_referer: false,
                include_signed_headers: false,
                signed_origin_override: None,
                referer_override: None,
            },
            GeminiCanvasDirectHttpImageAttempt {
                label: "canvas_web_preview_signed_share_contents_only",
                request_url: clients6_request_url.clone(),
                request_body: preview_contents_only_body.clone(),
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: true,
                preserve_cross_origin_referer: true,
                include_signed_headers: true,
                signed_origin_override: Some(page_origin.clone()),
                referer_override: Some(share_url.clone()),
            },
            GeminiCanvasDirectHttpImageAttempt {
                label: "canvas_web_preview_signed_share_aspect_only",
                request_url: clients6_request_url.clone(),
                request_body: preview_aspect_only_body.clone(),
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: true,
                preserve_cross_origin_referer: true,
                include_signed_headers: true,
                signed_origin_override: Some(page_origin.clone()),
                referer_override: Some(share_url.clone()),
            },
            GeminiCanvasDirectHttpImageAttempt {
                label: "canvas_web_preview_plain_share_contents_only",
                request_url: clients6_request_url.clone(),
                request_body: preview_contents_only_body,
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: true,
                preserve_cross_origin_referer: true,
                include_signed_headers: false,
                signed_origin_override: None,
                referer_override: Some(share_url.clone()),
            },
            GeminiCanvasDirectHttpImageAttempt {
                label: "canvas_web_preview_plain_share_aspect_only",
                request_url: clients6_request_url.clone(),
                request_body: preview_aspect_only_body,
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: true,
                preserve_cross_origin_referer: true,
                include_signed_headers: false,
                signed_origin_override: None,
                referer_override: Some(share_url.clone()),
            },
            GeminiCanvasDirectHttpImageAttempt {
                label: "google_api_preview",
                request_url: format!(
                    "{}/models/{}:generateContent",
                    google_api_base_url.trim_end_matches('/'),
                    preview_model
                ),
                request_body: gemini_canvas::build_direct_http_image_request_body(
                    req,
                    preview_model,
                ),
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: false,
                preserve_cross_origin_referer: true,
                include_signed_headers: false,
                signed_origin_override: None,
                referer_override: Some(app_url.clone()),
            },
            GeminiCanvasDirectHttpImageAttempt {
                label: "google_api_official",
                request_url: format!(
                    "{}/models/{}:generateContent",
                    google_api_base_url.trim_end_matches('/'),
                    official_model
                ),
                request_body: gemini_canvas::build_image_request_body(req, official_model),
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: false,
                preserve_cross_origin_referer: true,
                include_signed_headers: false,
                signed_origin_override: None,
                referer_override: Some(app_url.clone()),
            },
        ];
        if !is_edit_request {
            let imagen_predict_body = gemini_canvas::build_imagen_predict_request(req, prompt);
            for (label, imagen_model) in [
                (
                    "google_api_imagen4_predict",
                    gemini_canvas::GEMINI_CANVAS_IMAGEN_4_MODEL,
                ),
                (
                    "google_api_imagen3_predict",
                    gemini_canvas::GEMINI_CANVAS_IMAGEN_3_MODEL,
                ),
                (
                    "google_api_imagen3_legacy_predict",
                    gemini_canvas::GEMINI_CANVAS_IMAGEN_3_LEGACY_MODEL,
                ),
            ] {
                attempts.push(GeminiCanvasDirectHttpImageAttempt {
                    label,
                    request_url: format!(
                        "{}/models/{}:predict",
                        google_api_base_url.trim_end_matches('/'),
                        imagen_model
                    ),
                    request_body: imagen_predict_body.clone(),
                    kind: GeminiCanvasDirectHttpImageAttemptKind::ImagenPredict,
                    preserve_cross_origin_origin: false,
                    preserve_cross_origin_referer: true,
                    include_signed_headers: false,
                    signed_origin_override: None,
                    referer_override: Some(app_url.clone()),
                });
            }
        }
        let mut failures = Vec::new();
        let mut last_error = None;

        for attempt in attempts {
            let mut candidate_keys: Vec<Option<&str>> = if api_key_candidates.is_empty() {
                vec![None]
            } else {
                api_key_candidates
                    .iter()
                    .map(|candidate| Some(candidate.as_str()))
                    .collect()
            };
            if attempt.include_signed_headers
                && !candidate_keys.iter().any(|candidate| candidate.is_none())
            {
                candidate_keys.insert(0, None);
            }

            for api_key in candidate_keys {
                let transports: Vec<GeminiCanvasDirectHttpApiKeyTransport> = if api_key.is_some() {
                    gemini_canvas_direct_http_api_key_transports(&attempt.request_url).to_vec()
                } else {
                    vec![GeminiCanvasDirectHttpApiKeyTransport::HeaderOnly]
                };
                for transport in transports {
                    let attempt_label = if let Some(api_key) = api_key {
                        format!(
                            "{}[key={}][transport={}]",
                            attempt.label,
                            redact_gemini_canvas_api_key_for_logs(api_key),
                            transport.label()
                        )
                    } else {
                        format!("{}[key=none][transport=none]", attempt.label)
                    };
                    let body = match self
                        .execute_gemini_canvas_direct_http_json_with_options(
                            payload,
                            runtime,
                            &attempt.request_url,
                            &attempt.request_body,
                            timeout.max(Duration::from_secs(120)),
                            api_key,
                            transport,
                            attempt.signed_origin_override.as_deref(),
                            attempt.referer_override.as_deref(),
                            attempt.preserve_cross_origin_origin,
                            attempt.preserve_cross_origin_referer,
                            attempt.include_signed_headers,
                        )
                        .await
                    {
                        Ok(body) => body,
                        Err(error) => {
                            failures.push(format!(
                                "{attempt_label}={}",
                                summarize_gateway_error(&error)
                            ));
                            last_error = Some(error);
                            continue;
                        }
                    };
                    match attempt.kind {
                        GeminiCanvasDirectHttpImageAttemptKind::GenerateContent => {
                            let image = match gemini_canvas::extract_inline_image_from_generate_content_response(&body) {
                                Ok(image) => image,
                                Err(error) => {
                                    failures.push(format!(
                                        "{attempt_label}={}",
                                        summarize_gateway_error(&error)
                                    ));
                                    last_error = Some(error);
                                    continue;
                                }
                            };
                            return gemini_canvas::build_openai_images_response_from_bytes(
                                req,
                                prompt,
                                &[image],
                            );
                        }
                        GeminiCanvasDirectHttpImageAttemptKind::ImagenPredict => {
                            let images =
                                match gemini_canvas::extract_images_from_imagen_predict_response(
                                    &body,
                                ) {
                                    Ok(images) => images,
                                    Err(error) => {
                                        failures.push(format!(
                                            "{attempt_label}={}",
                                            summarize_gateway_error(&error)
                                        ));
                                        last_error = Some(error);
                                        continue;
                                    }
                                };
                            return gemini_canvas::build_openai_images_response_from_bytes(
                                req, prompt, &images,
                            );
                        }
                    }
                }
            }
        }

        let mut error =
            last_error.unwrap_or_else(gemini_canvas_image_json_attempts_exhausted_error);
        if !failures.is_empty() {
            let key_summary = if api_key_candidates.is_empty() {
                "none".to_string()
            } else {
                api_key_candidates
                    .iter()
                    .map(|candidate| redact_gemini_canvas_api_key_for_logs(candidate))
                    .collect::<Vec<_>>()
                    .join(",")
            };
            error.message = format!(
                "{}; api_key_candidates={}; page_harvest={}; attempts={}",
                error.message,
                key_summary,
                if page_harvest_probes.is_empty() {
                    "none".to_string()
                } else {
                    page_harvest_probes.join(" | ")
                },
                failures.join(" | ")
            );
        }
        Err(error)
    }

    async fn harvest_gemini_canvas_direct_http_api_keys(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        timeout: Duration,
    ) -> (Vec<String>, Vec<String>) {
        let provider = "gemini_canvas_compatible";
        let base_url = payload.base_url.trim_end_matches('/');
        let page_urls = [
            gemini_canvas::direct_http_referrer(base_url, &runtime.share_id),
            format!("{base_url}{}", gemini_web::GEMINI_WEB_DEFAULT_APP_PATH),
        ];
        let mut collected = Vec::new();
        let mut probes = Vec::new();

        for page_url in page_urls {
            let page_html = match self
                .fetch_gemini_canvas_direct_http_page_html(payload, session, &page_url, timeout)
                .await
            {
                Ok(body) => body,
                Err(error) => {
                    probes.push(format!(
                        "{} fetch_error={}",
                        page_url,
                        summarize_gateway_error(&error)
                    ));
                    debug!(
                        provider,
                        url = %page_url,
                        error = %summarize_gateway_error(&error),
                        "gemini canvas page API key harvest failed"
                    );
                    continue;
                }
            };
            let page_contains_aiza = page_html.contains("AIza");
            let harvested = gemini_canvas::extract_google_api_keys_from_page_blob(&page_html);
            probes.push(format!(
                "{} len={} contains_aiza={} api_key_count={}",
                page_url,
                page_html.len(),
                page_contains_aiza,
                harvested.len()
            ));
            debug!(
                provider,
                url = %page_url,
                page_len = page_html.len(),
                page_contains_aiza,
                api_key_count = harvested.len(),
                "gemini canvas direct HTTP page harvest probe"
            );
            if harvested.is_empty() {
                continue;
            }
            debug!(
                provider,
                url = %page_url,
                api_key_count = harvested.len(),
                "harvested Gemini Canvas API key candidates from current page state"
            );
            for candidate in harvested {
                if collected.iter().any(|existing| existing == &candidate) {
                    continue;
                }
                collected.push(candidate);
            }
        }

        (collected, probes)
    }

    async fn fetch_gemini_canvas_direct_http_page_html(
        &self,
        payload: &ProviderAccountPayload,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        page_url: &str,
        timeout: Duration,
    ) -> Result<String, GatewayError> {
        self.fetch_gemini_canvas_direct_http_page_html_with_locale(
            payload, session, page_url, timeout, None,
        )
        .await
    }

    async fn fetch_gemini_canvas_direct_http_page_html_with_locale(
        &self,
        payload: &ProviderAccountPayload,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        page_url: &str,
        timeout: Duration,
        locale_override: Option<&str>,
    ) -> Result<String, GatewayError> {
        let mut session = session.clone();
        self.fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale(
            payload,
            &mut session,
            page_url,
            timeout,
            locale_override,
        )
        .await
    }

    async fn fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale(
        &self,
        payload: &ProviderAccountPayload,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        page_url: &str,
        timeout: Duration,
        locale_override: Option<&str>,
    ) -> Result<String, GatewayError> {
        fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale(
            &self.http,
            &self.plain_http,
            payload,
            session,
            page_url,
            timeout,
            locale_override,
        )
        .await
    }

    pub(crate) async fn execute_gemini_canvas_media(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let runtime = gemini_canvas::runtime_from_payload(payload)?;
        let browser_runtime_state_object_key_for = |operation: &str| {
            gemini_canvas::browser_runtime_state_object_key_for_browser_operation(
                payload, operation,
            )
            .unwrap_or_else(|| runtime.runtime_state_object_key.clone())
        };
        let browser_cdp_url = gemini_canvas::browser_cdp_url(payload);
        let browser_cookie_header = gemini_canvas::browser_cookie_header(payload);
        let locale = gemini_canvas::locale_from_payload(payload);
        let base_url = payload.base_url.trim_end_matches('/');

        match req.endpoint_kind {
            EndpointKind::ImagesGenerations => {
                let _ = gemini_canvas::resolve_image_model(model)?;
                if gemini_canvas::requested_output_count(req) > 1 {
                    return Err(gemini_canvas::unsupported_image_count_error(provider));
                }

                let prompt =
                    gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
                        req,
                        gemini_canvas::GeminiCanvasMediaOperation::Image,
                    )?;
                if gemini_canvas::pure_http_enabled(payload) {
                    match self
                        .execute_gemini_canvas_media_direct_http(
                            payload,
                            req,
                            model,
                            gemini_canvas::GeminiCanvasMediaOperation::Image,
                            prompt.clone(),
                            self.timeout.max(Duration::from_secs(240)),
                        )
                        .await
                    {
                        Ok(body) => return Ok(body),
                        Err(error)
                            if payload.adapter != "gemini_web_reverse_modular_compatible" =>
                        {
                            return Err(error);
                        }
                        Err(error) => {
                            debug!(
                                provider,
                                adapter = %payload.adapter,
                                error = %summarize_gateway_error(&error),
                                "gemini web reverse modular legacy image direct HTTP lane failed; falling back to browser-owned image invocation"
                            );
                        }
                    }
                }
                let browser_runtime_state_object_key =
                    browser_runtime_state_object_key_for("image");
                let request_timeout = self.timeout.max(Duration::from_secs(240));
                let invocation_input =
                    gemini_canvas_web_reverse_modular::build_browser_operation_invocation_input_from_values(
                        base_url,
                        &runtime.share_id,
                        &browser_runtime_state_object_key,
                        browser_cdp_url.as_deref(),
                        browser_cookie_header.as_deref(),
                        "image",
                        &prompt,
                        &locale,
                        request_timeout,
                    );
                let result = if let Some(result) = self
                    .execute_remote_browser_executor(
                        "gemini_canvas",
                        provider_account_id,
                        req.endpoint_kind,
                        invocation_input,
                    )
                    .await?
                {
                    gemini_canvas_web_reverse_modular::parse_remote_media_browser_invocation_value(
                        provider, result, "image",
                    )?
                } else {
                    let browser_pool_base_url =
                        self.ensure_gemini_canvas_browser_pool(provider).await?;
                    self.execute_gemini_canvas_browser_request_with_recovery(
                        provider,
                        &browser_pool_base_url,
                        base_url,
                        &runtime.share_id,
                        &browser_runtime_state_object_key,
                        browser_cdp_url.as_deref(),
                        browser_cookie_header.as_deref(),
                        "image",
                        &prompt,
                        &locale,
                        request_timeout,
                    )
                    .await?
                };

                let image_assets =
                    gemini_canvas_web_reverse_modular::collect_image_media_assets(&result)
                        .into_iter()
                        .map(convert_gemini_canvas_asset)
                        .collect::<Vec<_>>();
                if image_assets.is_empty() {
                    return Err(
                        gemini_canvas_web_reverse_modular::browser_pool_missing_image_asset_error(
                            provider,
                        ),
                    );
                }

                let selected_image_assets = image_assets
                    .iter()
                    .take(gemini_canvas::requested_output_count(req))
                    .collect::<Vec<_>>();
                if gemini_canvas::prefers_url_response(req)?
                    && selected_image_assets
                        .iter()
                        .all(|asset| gemini_canvas_asset_url_is_caller_usable(&asset.url))
                {
                    return gemini_canvas::build_openai_images_response_from_urls(
                        req,
                        &prompt,
                        &image_assets,
                    );
                }

                let mut downloaded = Vec::with_capacity(image_assets.len());
                for asset in selected_image_assets {
                    if let Some(image) =
                        gemini_canvas_web_reverse_modular::decode_browser_pool_inline_image_asset(
                            provider, asset,
                        )?
                    {
                        downloaded.push(image);
                        continue;
                    }

                    if asset.url.starts_with("http://")
                        || asset.url.starts_with("https://")
                        || asset.url.starts_with("//")
                        || asset.url.starts_with('/')
                    {
                        let image = match self
                            .fetch_gemini_canvas_direct_http_image_asset(
                                payload,
                                &runtime,
                                asset,
                                request_timeout,
                            )
                            .await
                        {
                            Ok(image) => image,
                            Err(download_error) => {
                                let browser_pool_base_url =
                                    self.ensure_gemini_canvas_browser_pool(provider).await?;
                                let (bytes, response_content_type) =
                                    gemini_canvas_web_reverse_modular::execute_connected_fetch_get_bytes(
                                        &self.http,
                                        request_timeout,
                                        provider,
                                        &browser_pool_base_url,
                                        base_url,
                                        &runtime.share_id,
                                        &browser_runtime_state_object_key,
                                        browser_cdp_url.as_deref(),
                                        browser_cookie_header.as_deref(),
                                        &asset.url,
                                    )
                                    .await
                                    .map_err(|browser_fetch_error| {
                                        let mut browser_fetch_error = browser_fetch_error;
                                        browser_fetch_error.message = format!(
                                            "{}; direct_http_download={}",
                                            browser_fetch_error.message,
                                            summarize_gateway_error(&download_error)
                                        );
                                        browser_fetch_error
                                    })?;
                                gemini_canvas_web_reverse_modular::build_downloaded_image_from_bytes(
                                    asset,
                                    response_content_type.as_deref(),
                                    &bytes,
                                )
                            }
                        };
                        downloaded.push(image);
                        continue;
                    }

                    let response = self
                        .http
                        .request(Method::GET, &asset.url)
                        .timeout(request_timeout)
                        .redirect(rquest::redirect::Policy::limited(10))
                        .send()
                        .await
                        .map_err(|e| classify_network_error(&e, Some(provider)))?;

                    let status = response.status().as_u16();
                    if !response.status().is_success() {
                        let body_text = response
                            .text()
                            .await
                            .unwrap_or_else(|_| String::from("<unreadable body>"));
                        return Err(classify_upstream_error(status, &body_text, Some(provider)));
                    }

                    let mime_type = response
                        .headers()
                        .get(rquest::header::CONTENT_TYPE)
                        .and_then(|value| value.to_str().ok())
                        .map(str::trim)
                        .filter(|value| value.starts_with("image/"))
                        .map(ToString::to_string)
                        .unwrap_or_else(|| asset.mime_type.clone());
                    let bytes = response
                        .bytes()
                        .await
                        .map_err(|e| classify_network_error(&e, Some(provider)))?;
                    downloaded.push(gemini_canvas::GeminiCanvasImage {
                        mime_type,
                        bytes: bytes.to_vec(),
                    });
                }

                gemini_canvas::build_openai_images_response_from_bytes(req, &prompt, &downloaded)
            }
            EndpointKind::ImagesEdits => {
                let _ = gemini_canvas::resolve_image_model(model)?;
                if gemini_canvas::requested_output_count(req) > 1 {
                    return Err(gemini_canvas::unsupported_image_edit_count_error(provider));
                }

                let prompt =
                    gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
                        req,
                        gemini_canvas::GeminiCanvasMediaOperation::Image,
                    )?;
                self.execute_gemini_canvas_media_direct_http(
                    payload,
                    req,
                    model,
                    gemini_canvas::GeminiCanvasMediaOperation::Image,
                    prompt,
                    self.timeout
                        .min(Duration::from_secs(420))
                        .max(Duration::from_secs(330)),
                )
                .await
            }
            EndpointKind::MusicGenerations => {
                let _ = gemini_canvas::resolve_music_model(model)?;
                let prompt =
                    gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
                        req,
                        gemini_canvas::GeminiCanvasMediaOperation::Music,
                    )?;
                if gemini_canvas::pure_http_enabled(payload) {
                    match self
                        .execute_gemini_canvas_media_direct_http(
                            payload,
                            req,
                            model,
                            gemini_canvas::GeminiCanvasMediaOperation::Music,
                            prompt.clone(),
                            self.timeout.max(Duration::from_secs(480)),
                        )
                        .await
                    {
                        Ok(body)
                            if payload.adapter == "gemini_web_reverse_modular_compatible"
                                && gemini_canvas_music_response_requires_browser_followup(
                                    &body,
                                ) =>
                        {
                            debug!(
                                provider,
                                adapter = %payload.adapter,
                                "gemini web reverse modular legacy music direct HTTP lane reached accepted/pending semantics; continuing with browser-owned music invocation"
                            );
                        }
                        Ok(body) => return Ok(body),
                        Err(error)
                            if payload.adapter != "gemini_web_reverse_modular_compatible" =>
                        {
                            return Err(error);
                        }
                        Err(error) => {
                            debug!(
                                provider,
                                adapter = %payload.adapter,
                                error = %summarize_gateway_error(&error),
                                "gemini web reverse modular legacy music direct HTTP lane failed; falling back to browser-owned music invocation"
                            );
                        }
                    }
                }
                let browser_runtime_state_object_key =
                    browser_runtime_state_object_key_for("music");
                let request_timeout = self.timeout.max(Duration::from_secs(480));
                let invocation_input =
                    gemini_canvas_web_reverse_modular::build_browser_operation_invocation_input_from_values(
                        base_url,
                        &runtime.share_id,
                        &browser_runtime_state_object_key,
                        browser_cdp_url.as_deref(),
                        browser_cookie_header.as_deref(),
                        "music",
                        &prompt,
                        &locale,
                        request_timeout,
                    );
                let result = if let Some(result) = self
                    .execute_remote_browser_executor(
                        "gemini_canvas",
                        provider_account_id,
                        req.endpoint_kind,
                        invocation_input,
                    )
                    .await?
                {
                    gemini_canvas_web_reverse_modular::parse_remote_media_browser_invocation_value(
                        provider, result, "music",
                    )?
                } else {
                    let browser_pool_base_url =
                        self.ensure_gemini_canvas_browser_pool(provider).await?;
                    self.execute_gemini_canvas_browser_request_with_recovery(
                        provider,
                        &browser_pool_base_url,
                        base_url,
                        &runtime.share_id,
                        &browser_runtime_state_object_key,
                        browser_cdp_url.as_deref(),
                        browser_cookie_header.as_deref(),
                        "music",
                        &prompt,
                        &locale,
                        request_timeout,
                    )
                    .await?
                };

                let asset = result
                    .media
                    .iter()
                    .find(|asset| asset.kind == "video" || asset.kind == "audio")
                    .map(convert_gemini_canvas_asset)
                    .ok_or_else(|| gemini_canvas_music_missing_asset_error(provider))?;

                Ok(gemini_canvas::build_music_generation_response(
                    model,
                    &prompt,
                    &asset,
                    result.body_text.as_deref(),
                ))
            }
            EndpointKind::VideosGenerations => {
                let _ = gemini_canvas::resolve_video_model(model)?;
                if gemini_canvas::requested_output_count(req) > 1 {
                    return Err(gemini_canvas_video_unsupported_count_error(provider));
                }

                let prompt =
                    gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
                        req,
                        gemini_canvas::GeminiCanvasMediaOperation::Video,
                    )?;
                let continuation = gemini_canvas_video_continuation_from_request(req)?;
                let is_continuation = continuation.is_some();
                let browser_resume_requested = req
                    .raw_body
                    .get("resume_existing_media")
                    .and_then(Value::as_bool)
                    .or_else(|| {
                        req.raw_body
                            .get("resumeExistingMedia")
                            .and_then(Value::as_bool)
                    })
                    .unwrap_or(false);
                if gemini_canvas::pure_http_enabled(payload)
                    && is_continuation
                    && !browser_resume_requested
                {
                    return self
                        .execute_gemini_canvas_media_direct_http(
                            payload,
                            req,
                            model,
                            gemini_canvas::GeminiCanvasMediaOperation::Video,
                            prompt,
                            self.timeout.max(Duration::from_secs(720)),
                        )
                        .await;
                }
                let browser_runtime_state_object_key =
                    browser_runtime_state_object_key_for("video");
                let request_timeout = self.timeout.max(Duration::from_secs(720));
                let mut invocation_input =
                    gemini_canvas_web_reverse_modular::build_browser_operation_invocation_input_from_values(
                        base_url,
                        &runtime.share_id,
                        &browser_runtime_state_object_key,
                        browser_cdp_url.as_deref(),
                        browser_cookie_header.as_deref(),
                        "video",
                        &prompt,
                        &locale,
                        request_timeout,
                    );
                if let Some(continuation) = continuation.as_ref() {
                    apply_gemini_canvas_browser_video_continuation(
                        &mut invocation_input,
                        continuation,
                    );
                }
                let result = if let Some(result) = self
                    .execute_remote_browser_executor(
                        "gemini_canvas",
                        provider_account_id,
                        req.endpoint_kind,
                        invocation_input.clone(),
                    )
                    .await?
                {
                    gemini_canvas_web_reverse_modular::parse_remote_media_browser_invocation_value(
                        provider, result, "video",
                    )?
                } else {
                    let browser_pool_base_url =
                        self.ensure_gemini_canvas_browser_pool(provider).await?;
                    gemini_canvas_web_reverse_modular::execute_browser_request_input_with_recovery(
                        &self.http,
                        request_timeout,
                        provider,
                        &browser_pool_base_url,
                        &invocation_input,
                        "video",
                    )
                    .await?
                };

                let mut asset = result
                    .media
                    .iter()
                    .find(|asset| asset.kind == "video")
                    .map(convert_gemini_canvas_asset)
                    .ok_or_else(|| gemini_canvas_video_missing_asset_error(provider))?;

                if asset.body_base64.is_none()
                    && (asset.url.starts_with("http://") || asset.url.starts_with("https://"))
                {
                    let browser_pool_base_url =
                        self.ensure_gemini_canvas_browser_pool(provider).await?;
                    let (bytes, response_content_type) =
                        gemini_canvas_web_reverse_modular::execute_connected_fetch_get_bytes(
                            &self.http,
                            request_timeout,
                            provider,
                            &browser_pool_base_url,
                            base_url,
                            &runtime.share_id,
                            &browser_runtime_state_object_key,
                            browser_cdp_url.as_deref(),
                            browser_cookie_header.as_deref(),
                            &asset.url,
                        )
                        .await?;
                    if bytes.is_empty() {
                        return Err(gemini_canvas_video_missing_asset_error(provider));
                    }
                    if let Some(content_type) = response_content_type
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| value.starts_with("video/"))
                    {
                        asset.mime_type = content_type.to_string();
                    }
                    asset.body_base64 =
                        Some(base64::engine::general_purpose::STANDARD.encode(bytes));
                }

                Ok(gemini_canvas::build_video_generation_response(
                    model,
                    &prompt,
                    &asset,
                    result.body_text.as_deref(),
                ))
            }
            _ => Err(gemini_canvas::unsupported_media_adapter_endpoint_error(
                provider,
            )),
        }
    }

    pub(crate) async fn execute_gemini_canvas_text(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<CanonicalRelayResponse, GatewayError> {
        let body = self
            .execute_gemini_canvas_generate_content_json(payload, req, model)
            .await?;
        gemini_api_modular::parse_generate_content_response(&body, model)
    }

    pub(crate) async fn execute_gemini_canvas_text_stream(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<
        std::pin::Pin<Box<dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>>,
        GatewayError,
    > {
        let canonical = self
            .execute_gemini_canvas_text(payload, req, model, None)
            .await?;
        let sse_bytes = canonical_response_to_openai_sse_bytes(req, model, &canonical)
            .into_iter()
            .map(Ok);
        Ok(Box::pin(futures::stream::iter(sse_bytes)))
    }

    async fn execute_gemini_canvas_modular_browser_relay_text(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<CanonicalRelayResponse, GatewayError> {
        let request_timeout = self.timeout.max(Duration::from_secs(300));
        let locale = gemini_canvas::locale_from_payload(payload);
        let provider = payload.adapter.as_str();
        let prompt = gemini_canvas::prompt_for_text_request(
            req,
            "Gemini Canvas program relay text requests require a prompt.",
            "missing_gemini_canvas_program_text_prompt",
        )?;
        let model = gemini_canvas::resolve_text_model(model)?;
        let program_owned = payload.adapter == "gemini_canvas_program_web_reverse_compatible";
        let effective_payload = if program_owned {
            self.ensure_gemini_canvas_program_payload_handle(
                payload,
                "text",
                &locale,
                request_timeout,
            )
            .await?
        } else {
            payload.clone()
        };
        let runtime = gemini_canvas::runtime_from_payload(&effective_payload)?;
        if program_owned {
            let canonical = self
                .execute_gemini_canvas_direct_http_stream_generate_text(
                    &effective_payload,
                    req,
                    model,
                    &runtime,
                    &prompt,
                    request_timeout,
                )
                .await?;
            if gemini_canvas_text_response_is_generic_welcome(&prompt, &canonical.text) {
                return Err(gemini_canvas_generic_welcome_response_error(provider));
            }
            return Ok(canonical);
        }
        let browser_runtime_state_object_key =
            resolved_gemini_canvas_browser_runtime_state_object_key(&effective_payload, &runtime);
        let browser_cookie_header = gemini_canvas::browser_cookie_header(&effective_payload);
        let request_body = gemini_canvas::build_text_request_body(req, model);
        let request_url = gemini_canvas::build_text_fetch_url(&runtime, model);
        let browser_pool_base_url = self.ensure_gemini_canvas_browser_pool(provider).await?;
        let body = self
            .execute_gemini_canvas_connected_fetch_json_with_mode(
                provider,
                &browser_pool_base_url,
                payload.base_url.trim_end_matches('/'),
                &runtime.share_id,
                &browser_runtime_state_object_key,
                gemini_canvas::browser_cdp_url(payload).as_deref(),
                browser_cookie_header.as_deref(),
                &request_url,
                &request_body,
                "canvas_proxy",
                request_timeout,
            )
            .await?;
        let canonical = gemini_api_modular::parse_generate_content_response(&body, model)?;
        if gemini_canvas_text_response_is_generic_welcome(&prompt, &canonical.text) {
            return Err(gemini_canvas_generic_welcome_response_error(provider));
        }
        Ok(canonical)
    }

    async fn execute_gemini_canvas_modular_browser_relay_text_stream(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<
        std::pin::Pin<Box<dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>>,
        GatewayError,
    > {
        let canonical = self
            .execute_gemini_canvas_modular_browser_relay_text(payload, req, model, None)
            .await?;
        let sse_bytes = canonical_response_to_openai_sse_bytes(req, model, &canonical)
            .into_iter()
            .map(Ok);
        Ok(Box::pin(futures::stream::iter(sse_bytes)))
    }

    async fn execute_gemini_canvas_modular_browser_relay_tts(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<BinaryUpstreamResponse, GatewayError> {
        let locale = gemini_canvas::locale_from_payload(payload);
        let request_timeout = self.timeout.max(Duration::from_secs(300));
        let provider = payload.adapter.as_str();
        let prompt = gemini_canvas::prompt_for_text_request(
            req,
            "Gemini Canvas browser relay TTS requests require a prompt.",
            "missing_gemini_canvas_modular_tts_prompt",
        )?;
        let program_owned = payload.adapter == "gemini_canvas_program_web_reverse_compatible";
        let effective_payload = if program_owned {
            self.ensure_gemini_canvas_program_payload_handle(
                payload,
                "tts",
                &locale,
                request_timeout,
            )
            .await?
        } else {
            payload.clone()
        };
        let runtime = gemini_canvas::runtime_from_payload(&effective_payload)?;
        let locale = gemini_canvas::locale_from_payload(payload);
        let program_config = if effective_payload.adapter
            == "gemini_canvas_program_web_reverse_compatible"
        {
            ensure_gemini_canvas_program_payload_avoids_official_api_identity(&effective_payload)?;
            None
        } else {
            None
        };
        if program_owned {
            if gemini_canvas::pure_http_enabled(&effective_payload) {
                return self
                    .execute_gemini_canvas_direct_http_tts(
                        &effective_payload,
                        req,
                        model,
                        &runtime,
                        request_timeout,
                    )
                    .await;
            }
            return Err(gemini_canvas_program_tts_pure_http_required_error(provider));
        }
        let result = self
            .execute_gemini_canvas_owned_browser_invocation(
                &effective_payload,
                provider,
                &self.ensure_gemini_canvas_browser_pool(provider).await?,
                effective_payload.base_url.trim_end_matches('/'),
                &runtime,
                program_config.as_ref(),
                "tts",
                &prompt,
                &locale,
                request_timeout,
            )
            .await?;

        let audio =
            gemini_canvas_web_reverse_modular::decode_modular_tts_audio_payload(&result, provider)?;

        Ok(BinaryUpstreamResponse {
            body: bytes::Bytes::from(audio.bytes),
            content_type: Some(audio.mime_type),
            extra_headers: Vec::new(),
        })
    }

    async fn execute_gemini_canvas_program_pure_http_image(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        prompt: &str,
        timeout: Duration,
    ) -> Result<Value, GatewayError> {
        let provider = "gemini_canvas_program_web_reverse_compatible";
        let runtime = gemini_canvas::runtime_from_payload(payload)?;
        ensure_gemini_canvas_program_payload_avoids_official_api_identity(payload)?;
        if gemini_canvas::pure_http_enabled(payload) {
            return self
                .execute_gemini_canvas_direct_http_image(
                    payload,
                    req,
                    model,
                    &runtime,
                    prompt.to_string(),
                    timeout,
                )
                .await;
        }
        Err(gemini_canvas_program_image_pure_http_required_error(
            provider,
        ))
    }

    async fn execute_gemini_canvas_modular_browser_relay_media(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let locale = gemini_canvas::locale_from_payload(payload);
        let provider = payload.adapter.as_str();
        let bootstrap_operation = match req.endpoint_kind {
            EndpointKind::ImagesGenerations => "image",
            EndpointKind::MusicGenerations => "music",
            EndpointKind::VideosGenerations => "video",
            _ => "image",
        };
        let bootstrap_timeout = match req.endpoint_kind {
            EndpointKind::ImagesGenerations => self.timeout.max(Duration::from_secs(240)),
            EndpointKind::MusicGenerations => self.timeout.max(Duration::from_secs(480)),
            EndpointKind::VideosGenerations => self.timeout.max(Duration::from_secs(720)),
            _ => self.timeout.max(Duration::from_secs(240)),
        };
        let program_owned = payload.adapter == "gemini_canvas_program_web_reverse_compatible";

        match req.endpoint_kind {
            EndpointKind::ImagesGenerations => {
                let _ = gemini_canvas::resolve_image_model(model)?;
                if gemini_canvas::requested_output_count(req) > 1 {
                    return Err(gemini_canvas::unsupported_modular_image_count_error(
                        provider,
                    ));
                }
                let prompt =
                    gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
                        req,
                        gemini_canvas::GeminiCanvasMediaOperation::Image,
                    )?;
                let request_timeout = self.timeout.max(Duration::from_secs(240));
                if program_owned {
                    if let Some(body) = self
                        .maybe_execute_gemini_canvas_program_modular_media_direct_http(
                            payload,
                            req,
                            model,
                            &prompt,
                            request_timeout,
                            gemini_canvas::GeminiCanvasMediaOperation::Image,
                        )
                        .await?
                    {
                        return Ok(body);
                    }
                }
                let effective_payload = if program_owned {
                    self.ensure_gemini_canvas_program_payload_handle(
                        payload,
                        bootstrap_operation,
                        &locale,
                        bootstrap_timeout,
                    )
                    .await?
                } else {
                    payload.clone()
                };
                let runtime = gemini_canvas::runtime_from_payload(&effective_payload)?;
                let base_url = effective_payload.base_url.trim_end_matches('/');
                let program_config = if program_owned {
                    Some(
                        gemini_canvas_program_web_reverse_modular::relay_config_from_payload(
                            &effective_payload,
                        )?,
                    )
                } else {
                    None
                };
                let result = self
                    .execute_gemini_canvas_modular_media_browser_result(
                        provider_account_id,
                        &effective_payload,
                        provider,
                        req.endpoint_kind,
                        base_url,
                        &runtime,
                        program_config.as_ref(),
                        "image",
                        &prompt,
                        &locale,
                        request_timeout,
                    )
                    .await?;

                gemini_canvas_web_reverse_modular::build_image_generation_response_from_invocation(
                    &self.http,
                    provider,
                    req,
                    &prompt,
                    &result,
                    request_timeout,
                )
                .await
            }
            EndpointKind::ImagesEdits => Err(gemini_canvas::unsupported_modular_image_edits_error(
                provider,
            )),
            EndpointKind::MusicGenerations => {
                let _ = gemini_canvas::resolve_music_model(model)?;
                let prompt =
                    gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
                        req,
                        gemini_canvas::GeminiCanvasMediaOperation::Music,
                    )?;
                let request_timeout = self.timeout.max(Duration::from_secs(480));
                let effective_payload = if program_owned {
                    self.ensure_gemini_canvas_program_payload_handle(
                        payload,
                        bootstrap_operation,
                        &locale,
                        bootstrap_timeout,
                    )
                    .await?
                } else {
                    payload.clone()
                };
                let runtime = gemini_canvas::runtime_from_payload(&effective_payload)?;
                let base_url = effective_payload.base_url.trim_end_matches('/');
                let program_config = if program_owned {
                    Some(
                        gemini_canvas_program_web_reverse_modular::relay_config_from_payload(
                            &effective_payload,
                        )?,
                    )
                } else {
                    None
                };
                if program_owned {
                    if let Some(body) = self
                        .maybe_execute_gemini_canvas_program_modular_media_direct_http(
                            &effective_payload,
                            req,
                            model,
                            &prompt,
                            request_timeout,
                            gemini_canvas::GeminiCanvasMediaOperation::Music,
                        )
                        .await?
                    {
                        return Ok(body);
                    }
                }
                let result = self
                    .execute_gemini_canvas_modular_media_browser_result(
                        provider_account_id,
                        &effective_payload,
                        provider,
                        req.endpoint_kind,
                        base_url,
                        &runtime,
                        program_config.as_ref(),
                        "music",
                        &prompt,
                        &locale,
                        request_timeout,
                    )
                    .await?;

                gemini_canvas_web_reverse_modular::build_music_generation_response_from_invocation(
                    model, &prompt, &result, provider,
                )
            }
            EndpointKind::VideosGenerations => {
                let _ = gemini_canvas::resolve_video_model(model)?;
                if gemini_canvas::requested_output_count(req) > 1 {
                    return Err(gemini_canvas_modular_video_unsupported_count_error(
                        provider,
                    ));
                }

                let prompt =
                    gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
                        req,
                        gemini_canvas::GeminiCanvasMediaOperation::Video,
                    )?;
                let request_timeout = self.timeout.max(Duration::from_secs(720));
                let direct_http_attempt_timeout = request_timeout.min(Duration::from_secs(300));
                let program_direct_http_payload = if program_owned {
                    Some(
                        self.ensure_gemini_canvas_program_payload_handle(
                            payload,
                            bootstrap_operation,
                            &locale,
                            bootstrap_timeout,
                        )
                        .await?,
                    )
                } else {
                    None
                };
                if program_owned {
                    if let Some(body) = self
                        .maybe_execute_gemini_canvas_program_modular_media_direct_http(
                            program_direct_http_payload.as_ref().unwrap_or(payload),
                            req,
                            model,
                            &prompt,
                            direct_http_attempt_timeout,
                            gemini_canvas::GeminiCanvasMediaOperation::Video,
                        )
                        .await?
                    {
                        return Ok(body);
                    }
                    return Err(
                        gemini_canvas_program_video_browser_fallback_forbidden_error(provider),
                    );
                }
                let effective_payload = if let Some(ensured_payload) = program_direct_http_payload {
                    ensured_payload
                } else {
                    payload.clone()
                };
                let runtime = gemini_canvas::runtime_from_payload(&effective_payload)?;
                let base_url = effective_payload.base_url.trim_end_matches('/');
                let program_config = if program_owned {
                    Some(
                        gemini_canvas_program_web_reverse_modular::relay_config_from_payload(
                            &effective_payload,
                        )?,
                    )
                } else {
                    None
                };
                let result = self
                    .execute_gemini_canvas_modular_media_browser_result(
                        provider_account_id,
                        &effective_payload,
                        provider,
                        req.endpoint_kind,
                        base_url,
                        &runtime,
                        program_config.as_ref(),
                        "video",
                        &prompt,
                        &locale,
                        request_timeout,
                    )
                    .await?;

                gemini_canvas_web_reverse_modular::build_video_generation_response_from_invocation(
                    model, &prompt, &result, provider,
                )
            }
            _ => Err(gemini_canvas::unsupported_modular_endpoint_error(provider)),
        }
    }

    async fn maybe_execute_gemini_canvas_program_modular_media_direct_http(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        prompt: &str,
        request_timeout: Duration,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
    ) -> Result<Option<Value>, GatewayError> {
        if !should_attempt_gemini_canvas_program_modular_media_direct_http(payload, operation) {
            return Ok(None);
        }

        let provider = payload.adapter.as_str();
        let result = match operation {
            gemini_canvas::GeminiCanvasMediaOperation::Image => {
                self.execute_gemini_canvas_program_pure_http_image(
                    payload,
                    req,
                    model,
                    prompt,
                    request_timeout,
                )
                .await
            }
            gemini_canvas::GeminiCanvasMediaOperation::Music
            | gemini_canvas::GeminiCanvasMediaOperation::Video => {
                self.execute_gemini_canvas_media_direct_http(
                    payload,
                    req,
                    model,
                    operation,
                    prompt.to_string(),
                    request_timeout,
                )
                .await
            }
        };

        match result {
            Ok(body) => Ok(Some(body)),
            Err(error) => {
                if should_treat_gemini_canvas_program_modular_media_direct_http_as_authoritative(
                    payload,
                ) {
                    return Err(error);
                }
                let operation_label = match operation {
                    gemini_canvas::GeminiCanvasMediaOperation::Image => "image",
                    gemini_canvas::GeminiCanvasMediaOperation::Music => "music",
                    gemini_canvas::GeminiCanvasMediaOperation::Video => "video",
                };
                debug!(
                    provider,
                    error = %summarize_gateway_error(&error),
                    "gemini canvas program modular pure HTTP {} lane failed; falling back to current program-owned browser execution",
                    operation_label,
                );
                Ok(None)
            }
        }
    }

    pub(crate) async fn execute_gemini_canvas_tts(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<BinaryUpstreamResponse, GatewayError> {
        let request_timeout = self.timeout.max(Duration::from_secs(300));
        let runtime = gemini_canvas::runtime_from_payload(payload)?;
        if payload.adapter == "gemini_canvas_program_web_reverse_compatible" {
            ensure_gemini_canvas_program_payload_avoids_official_api_identity(payload)?;
            if gemini_canvas::pure_http_enabled(payload) {
                return self
                    .execute_gemini_canvas_direct_http_tts(
                        payload,
                        req,
                        model,
                        &runtime,
                        request_timeout,
                    )
                    .await;
            }
            return Err(gemini_canvas_program_tts_pure_http_required_error(
                payload.adapter.as_str(),
            ));
        }
        if gemini_canvas::pure_http_enabled(payload) {
            return self
                .execute_gemini_canvas_direct_http_tts(
                    payload,
                    req,
                    model,
                    &runtime,
                    request_timeout,
                )
                .await;
        }

        self.execute_gemini_canvas_browser_backed_tts(payload, req, model, None)
            .await
    }

    pub(crate) async fn execute_gemini_canvas_browser_backed_tts(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        _model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<BinaryUpstreamResponse, GatewayError> {
        let request_timeout = self.timeout.max(Duration::from_secs(300));
        let runtime = gemini_canvas::runtime_from_payload(payload)?;
        let browser_runtime_state_object_key =
            resolved_gemini_canvas_browser_runtime_state_object_key(payload, &runtime);
        let browser_cdp_url = gemini_canvas::browser_cdp_url(payload);
        let browser_cookie_header = gemini_canvas::browser_cookie_header(payload);
        let provider = "gemini_canvas_compatible";
        let locale = gemini_canvas::locale_from_payload(payload);
        let prompt = gemini_canvas::prompt_for_text_request(
            req,
            "Gemini Canvas TTS requests require a prompt.",
            "missing_gemini_canvas_tts_prompt",
        )?;
        let browser_pool_base_url = self.ensure_gemini_canvas_browser_pool(provider).await?;
        let result = self
            .execute_gemini_canvas_browser_request_with_recovery(
                provider,
                &browser_pool_base_url,
                payload.base_url.trim_end_matches('/'),
                &runtime.share_id,
                &browser_runtime_state_object_key,
                browser_cdp_url.as_deref(),
                browser_cookie_header.as_deref(),
                "tts",
                &prompt,
                &locale,
                request_timeout,
            )
            .await?;

        let audio =
            gemini_canvas_web_reverse_modular::decode_browser_tts_audio_payload(&result, provider)?;
        let (body, content_type) = gemini_canvas::build_audio_binary_response(req, &audio)?;
        Ok(BinaryUpstreamResponse {
            body: bytes::Bytes::from(body),
            content_type: Some(content_type),
            extra_headers: Vec::new(),
        })
    }

    pub(crate) async fn execute_gemini_canvas_direct_http_tts(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        timeout: Duration,
    ) -> Result<BinaryUpstreamResponse, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let resolved_model = gemini_canvas::resolve_text_model(model)?;
        let prompt = gemini_canvas::prompt_for_text_request(
            req,
            "Gemini Canvas TTS requests require a prompt.",
            "missing_gemini_canvas_tts_prompt",
        )?;
        let (session, bootstrap, batchexecute_header_id) = self
            .prepare_gemini_canvas_direct_http_text_context(
                payload,
                resolved_model,
                runtime,
                timeout,
            )
            .await?;
        let stream_response = self
            .execute_gemini_canvas_direct_http_stream_generate_response_with_text_context(
                payload,
                resolved_model,
                &prompt,
                &session,
                &bootstrap,
                timeout,
            )
            .await?;
        let stream_status = stream_response.status().as_u16();
        let stream_content_type = stream_response
            .headers()
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let mut stream = stream_response.bytes_stream();
        let mut stream_body = String::new();
        let mut locator: Option<gemini_canvas::GeminiCanvasStreamGenerateLocator> = None;
        let mut trigger_body: Option<String> = None;
        let mut followup_body: Option<String> = None;
        let mut export_body: Option<String> = None;

        while let Some(chunk_result) = stream.next().await {
            let chunk =
                chunk_result.map_err(|error| classify_network_error(&error, Some(provider)))?;
            stream_body.push_str(&String::from_utf8_lossy(&chunk));
            if locator.is_some() {
                continue;
            }
            let Ok(candidate_locator) =
                gemini_canvas::extract_stream_generate_locator(&stream_body)
            else {
                continue;
            };
            let (candidate_trigger_body, candidate_followup_body) = self
                .execute_gemini_canvas_direct_http_tts_followups(
                    payload,
                    resolved_model,
                    &session,
                    &bootstrap,
                    batchexecute_header_id.as_deref(),
                    &candidate_locator,
                    timeout,
                )
                .await?;
            locator = Some(candidate_locator);
            trigger_body = Some(candidate_trigger_body);
            followup_body = Some(candidate_followup_body);
        }

        if locator.is_none()
            && (gemini_web::response_indicates_browser_challenge(
                stream_status,
                stream_content_type.as_deref(),
                &stream_body,
            ) || gemini_web::response_indicates_session_invalid(
                stream_status,
                stream_content_type.as_deref(),
                &stream_body,
            ))
        {
            return Err(classify_gemini_canvas_pure_http_error(
                stream_status,
                stream_content_type.as_deref(),
                &stream_body,
            ));
        }

        let locator = if let Some(locator) = locator {
            locator
        } else {
            let locator = gemini_canvas::extract_stream_generate_locator(&stream_body)?;
            let (candidate_trigger_body, candidate_followup_body) = self
                .execute_gemini_canvas_direct_http_tts_followups(
                    payload,
                    resolved_model,
                    &session,
                    &bootstrap,
                    batchexecute_header_id.as_deref(),
                    &locator,
                    timeout,
                )
                .await?;
            trigger_body = Some(candidate_trigger_body);
            followup_body = Some(candidate_followup_body);
            locator
        };
        let trigger_body = trigger_body.unwrap_or_default();
        let followup_body = followup_body.unwrap_or_default();
        if let Some(stream_text) =
            gemini_web::accumulate_gemini_web_response(&stream_body, resolved_model)
                .ok()
                .map(|response| response.text.trim().to_string())
                .filter(|text| !text.is_empty())
        {
            export_body = Some(
                self.execute_gemini_canvas_direct_http_tts_export(
                    payload,
                    resolved_model,
                    &session,
                    &bootstrap,
                    batchexecute_header_id.as_deref(),
                    &locator.app_path,
                    &stream_text,
                    timeout,
                )
                .await?,
            );
        }
        let export_body = export_body.unwrap_or_default();

        if let Some(audio_response) =
            gemini_web_reverse_modular::resolve_direct_http_tts_audio_response(
                self,
                payload,
                req,
                &session,
                timeout,
                &stream_body,
                &trigger_body,
                &followup_body,
                &export_body,
            )
            .await?
        {
            return Ok(audio_response);
        }

        let preview = |body: &str| {
            let trimmed = body.trim();
            if trimmed.is_empty() {
                "<empty>".to_string()
            } else {
                truncate_response_preview(trimmed, 180).to_string()
            }
        };
        let tail_preview = |body: &str| {
            let trimmed = body.trim();
            if trimmed.is_empty() {
                "<empty>".to_string()
            } else {
                let chars: Vec<char> = trimmed.chars().collect();
                let start = chars.len().saturating_sub(180);
                chars[start..].iter().collect::<String>()
            }
        };
        let stream_head_preview = preview(&stream_body);
        let stream_tail_preview = tail_preview(&stream_body);
        let trigger_preview = preview(&trigger_body);
        let followup_preview = preview(&followup_body);
        let export_preview = preview(&export_body);
        Err(
            gemini_web_reverse_modular::gemini_canvas_tts_direct_http_audio_unavailable_error(
                &locator.app_path,
                &stream_head_preview,
                &stream_tail_preview,
                &trigger_preview,
                &followup_preview,
                &export_preview,
            ),
        )
    }

    async fn execute_gemini_canvas_direct_http_tts_followups(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        batchexecute_header_id: Option<&str>,
        locator: &gemini_canvas::GeminiCanvasStreamGenerateLocator,
        timeout: Duration,
    ) -> Result<(String, String), GatewayError> {
        gemini_web_reverse_modular::execute_direct_http_tts_followups(
            self,
            payload,
            model,
            session,
            bootstrap,
            batchexecute_header_id,
            locator,
            timeout,
        )
        .await
    }

    async fn execute_gemini_canvas_direct_http_tts_export(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        batchexecute_header_id: Option<&str>,
        source_path: &str,
        response_text: &str,
        timeout: Duration,
    ) -> Result<String, GatewayError> {
        gemini_web_reverse_modular::execute_direct_http_tts_export(
            self,
            payload,
            model,
            session,
            bootstrap,
            batchexecute_header_id,
            source_path,
            response_text,
            timeout,
        )
        .await
    }

    async fn execute_gemini_canvas_direct_http_stream_generate_response_with_text_context(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        prompt: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        timeout: Duration,
    ) -> Result<rquest::Response, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let configured_base_url = payload.base_url.trim_end_matches('/');
        let effective_base_url = if configured_base_url.is_empty() {
            gemini_canvas_http_origin(payload)
        } else {
            configured_base_url.to_string()
        };
        let stream_url = format!(
            "{effective_base_url}{}",
            gemini_web::GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH
        );
        let text_preflight_source_path = gemini_web::GEMINI_WEB_DEFAULT_APP_PATH.to_string();
        let request_uuid = gemini_canvas::new_stream_generate_request_uuid();
        let request = gemini_canvas::build_stream_generate_heavy_request(
            prompt,
            bootstrap,
            &request_uuid,
            gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX,
        )?;

        for (rpcid, rpc_payload, model_header) in
            build_gemini_canvas_direct_http_text_generic_preflight_specs(&bootstrap.language)
        {
            self.execute_gemini_canvas_text_generic_preflight(
                payload,
                model,
                bootstrap,
                &text_preflight_source_path,
                session,
                rpcid,
                rpc_payload,
                model_header,
                timeout,
            )
            .await?;
        }
        let (state_len, tail_index, tail_value, marker) =
            gemini_canvas_direct_http_text_fast_version_preflight_spec();
        self.execute_gemini_canvas_text_state_variant_preflight(
            payload,
            model,
            bootstrap,
            &text_preflight_source_path,
            session,
            state_len,
            tail_index,
            tail_value,
            marker,
            timeout,
        )
        .await?;
        self.execute_gemini_canvas_text_mode_selection_preflight(
            payload,
            model,
            bootstrap,
            &text_preflight_source_path,
            session,
            "",
            "",
            timeout,
        )
        .await?;
        self.execute_gemini_canvas_text_bootstrap_preflight(
            payload,
            model,
            bootstrap,
            &text_preflight_source_path,
            session,
            "",
            "",
            timeout,
        )
        .await?;
        self.execute_gemini_canvas_text_state_preflight(
            payload,
            model,
            bootstrap,
            &text_preflight_source_path,
            session,
            "",
            "",
            timeout,
        )
        .await?;
        for (state_len, tail_index, tail_value, marker) in [
            (41usize, 40usize, Value::from(0), "side_nav_open_by_default"),
            (87usize, 86usize, Value::from(1), "popup_zs_visits_cooldown"),
            (87usize, 86usize, Value::from(2), "popup_zs_visits_cooldown"),
            (
                94usize,
                93usize,
                Value::String("NULL".to_string()),
                "current_popup_id",
            ),
            (
                94usize,
                93usize,
                Value::String("HUMAN_REVIEWER_DISCLOSURE".to_string()),
                "current_popup_id",
            ),
        ] {
            if let Err(error) = self
                .execute_gemini_canvas_text_state_variant_preflight(
                    payload,
                    model,
                    bootstrap,
                    &text_preflight_source_path,
                    session,
                    state_len,
                    tail_index,
                    tail_value,
                    marker,
                    timeout,
                )
                .await
            {
                debug!(
                    provider = "gemini_canvas_compatible",
                    error = %summarize_gateway_error(&error),
                    marker,
                    state_len,
                    tail_index,
                    "gemini canvas text optional page-state update failed during TTS direct HTTP StreamGenerate; continuing"
                );
            }
        }

        let mut headers = gemini_web_reverse_modular::build_headers(payload, None, model);
        apply_gemini_canvas_direct_http_stream_generate_headers(
            &mut headers,
            payload,
            &request_uuid,
            gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_MODEL_HEADER,
            Some(session),
            true,
        );

        debug!(
            provider,
            url = %stream_url,
            "sending gemini canvas pure HTTP TTS StreamGenerate request with reused text context"
        );
        let mut request_form = request.form.clone();
        let mut xsrf_retry_token: Option<String> = None;
        loop {
            let response = self
                .http
                .request(Method::POST, &stream_url)
                .headers(headers.clone())
                .query(&request.query)
                .timeout(timeout.max(Duration::from_secs(120)))
                .form(&request_form)
                .send()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            let status = response.status().as_u16();
            let content_type = response
                .headers()
                .get(rquest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let content_type_is_html = content_type
                .as_deref()
                .is_some_and(|value| value.to_ascii_lowercase().contains("text/html"));
            if !(200..300).contains(&status) || content_type_is_html {
                let body_text = response
                    .text()
                    .await
                    .map_err(|error| classify_network_error(&error, Some(provider)))?;
                if status == 400 {
                    if let Some(token) = extract_gemini_canvas_batchexecute_xsrf_token(&body_text) {
                        let current_at = header_map_string_from_form(&request_form, "at");
                        if xsrf_retry_token.as_deref() != Some(token.as_str())
                            && current_at.as_deref() != Some(token.as_str())
                        {
                            debug!(
                                provider,
                                xsrf_token_preview = %truncate_response_preview(&token, 24),
                                "retrying gemini canvas TTS StreamGenerate with xsrf token extracted from upstream error"
                            );
                            upsert_form_field(&mut request_form, "at", &token);
                            xsrf_retry_token = Some(token);
                            continue;
                        }
                    }
                }
                return Err(classify_gemini_canvas_pure_http_error(
                    status,
                    content_type.as_deref(),
                    &body_text,
                ));
            }

            return Ok(response);
        }
    }

    async fn execute_gemini_canvas_direct_http_image_lane(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        body_text: &str,
        timeout: Duration,
        mode_index: i64,
        request_started_at: SystemTime,
        initial_stream_allows_replay_template: bool,
        image_edit_uploads: Option<&[gemini_canvas::GeminiCanvasImageEditUpload]>,
        mut image_edit_followup_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
        image_json_policy: &GeminiCanvasDirectHttpImageJsonPolicy,
    ) -> Result<Value, GatewayError> {
        let recovery_strategy = build_gemini_canvas_image_recovery_strategy(
            req.endpoint_kind,
            initial_stream_allows_replay_template,
            body_text,
        );
        let image_assets = self
            .resolve_gemini_canvas_direct_http_image_assets(
                payload,
                req.endpoint_kind,
                model,
                runtime,
                prompt,
                body_text,
                timeout,
                mode_index,
                request_started_at,
                recovery_strategy,
                image_edit_uploads,
                image_edit_followup_context.as_deref_mut(),
            )
            .await?;

        self.build_gemini_canvas_direct_http_image_response(
            payload,
            req,
            model,
            runtime,
            prompt,
            timeout,
            &image_assets,
            image_json_policy,
        )
        .await
    }

    async fn resolve_gemini_canvas_direct_http_image_assets(
        &self,
        payload: &ProviderAccountPayload,
        endpoint_kind: EndpointKind,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        body_text: &str,
        timeout: Duration,
        mode_index: i64,
        request_started_at: SystemTime,
        recovery_strategy: GeminiCanvasImageRecoveryStrategy,
        image_edit_uploads: Option<&[gemini_canvas::GeminiCanvasImageEditUpload]>,
        mut image_edit_followup_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
    ) -> Result<Vec<gemini_canvas::GeminiCanvasMediaAsset>, GatewayError> {
        let (image_assets, _resolved_body_text) = match self
            .extract_gemini_canvas_image_assets_with_followup(
                payload,
                model,
                runtime,
                prompt,
                body_text,
                request_started_at,
                timeout,
                recovery_strategy.mode,
                image_edit_followup_context.as_deref_mut(),
            )
            .await
        {
            Ok(result) => result,
            Err(primary_error) => {
                self.execute_gemini_canvas_direct_http_image_retry_strategy(
                    payload,
                    endpoint_kind,
                    model,
                    runtime,
                    prompt,
                    primary_error,
                    timeout,
                    mode_index,
                    request_started_at,
                    recovery_strategy,
                    image_edit_uploads,
                    image_edit_followup_context.as_deref_mut(),
                )
                .await?
            }
        };
        Ok(image_assets)
    }

    async fn execute_gemini_canvas_direct_http_image_retry_strategy(
        &self,
        payload: &ProviderAccountPayload,
        endpoint_kind: EndpointKind,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        primary_error: GatewayError,
        timeout: Duration,
        mode_index: i64,
        request_started_at: SystemTime,
        recovery_strategy: GeminiCanvasImageRecoveryStrategy,
        image_edit_uploads: Option<&[gemini_canvas::GeminiCanvasImageEditUpload]>,
        mut image_edit_followup_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
    ) -> Result<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String), GatewayError> {
        let provider = "gemini_canvas_compatible";
        match recovery_strategy.template_retry_action {
            GeminiCanvasImageTemplateRetryAction::ReturnOriginal => Err(primary_error),
            GeminiCanvasImageTemplateRetryAction::ReturnOriginalWithLog(message) => {
                debug!(
                    provider,
                    endpoint_kind = ?endpoint_kind,
                    "{message}"
                );
                if should_attempt_gemini_canvas_browser_backed_image_edit_retry(
                    payload,
                    endpoint_kind,
                ) {
                    return self
                        .retry_resolve_gemini_canvas_direct_http_image_assets_with_legacy_template(
                            payload,
                            model,
                            runtime,
                            prompt,
                            &primary_error,
                            timeout,
                            mode_index,
                            request_started_at,
                            image_edit_uploads,
                            image_edit_followup_context.as_deref_mut(),
                            endpoint_kind,
                            recovery_strategy.mode,
                        )
                        .await;
                }
                Err(primary_error)
            }
            GeminiCanvasImageTemplateRetryAction::RetryLegacyTemplate => {
                self.retry_resolve_gemini_canvas_direct_http_image_assets_with_legacy_template(
                    payload,
                    model,
                    runtime,
                    prompt,
                    &primary_error,
                    timeout,
                    mode_index,
                    request_started_at,
                    image_edit_uploads,
                    image_edit_followup_context.as_deref_mut(),
                    endpoint_kind,
                    recovery_strategy.mode,
                )
                .await
            }
        }
    }

    async fn retry_resolve_gemini_canvas_direct_http_image_assets_with_legacy_template(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        primary_error: &GatewayError,
        timeout: Duration,
        mode_index: i64,
        request_started_at: SystemTime,
        image_edit_uploads: Option<&[gemini_canvas::GeminiCanvasImageEditUpload]>,
        mut image_edit_followup_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
        endpoint_kind: EndpointKind,
        recovery_mode: GeminiCanvasImageRecoveryMode,
    ) -> Result<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String), GatewayError> {
        let provider = "gemini_canvas_compatible";
        let retry_summary = summarize_gateway_error(primary_error);
        debug!(
            provider,
            error = %retry_summary,
            endpoint_kind = ?endpoint_kind,
            "gemini canvas direct HTTP image asset extraction failed after template-capable path; retrying legacy heavy builder"
        );
        let retry_body = self
            .execute_gemini_canvas_direct_http_stream_generate_body(
                payload,
                model,
                runtime,
                mode_index,
                prompt,
                timeout,
                false,
                image_edit_uploads,
                image_edit_followup_context.as_deref_mut(),
            )
            .await
            .map_err(|retry_error| {
                append_gateway_error_summary(
                    retry_error,
                    "image_template_extract_failure",
                    Some(&retry_summary),
                )
            })?;
        self.extract_gemini_canvas_image_assets_with_followup(
            payload,
            model,
            runtime,
            prompt,
            &retry_body,
            request_started_at,
            timeout,
            recovery_mode,
            image_edit_followup_context.as_deref_mut(),
        )
        .await
        .map_err(|retry_extract_error| {
            append_gateway_error_summary(
                retry_extract_error,
                "image_template_extract_failure",
                Some(&retry_summary),
            )
        })
    }

    async fn build_gemini_canvas_direct_http_image_response(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        timeout: Duration,
        image_assets: &[gemini_canvas::GeminiCanvasMediaAsset],
        image_json_policy: &GeminiCanvasDirectHttpImageJsonPolicy,
    ) -> Result<Value, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let selected_image_assets =
            match gemini_canvas_web_reverse_modular::plan_direct_http_image_response(
                req,
                prompt,
                image_assets,
                provider,
            )? {
                GeminiCanvasImageResponsePlan::FinalResponse(body) => return Ok(body),
                GeminiCanvasImageResponsePlan::Materialize(assets) => assets,
            };
        let request_timeout = timeout.max(Duration::from_secs(240));
        match self
            .build_gemini_canvas_materialized_image_response(
                payload,
                req,
                runtime,
                prompt,
                request_timeout,
                &selected_image_assets,
                provider,
            )
            .await
        {
            Ok(body) => Ok(body),
            Err(download_error) => {
                self.recover_gemini_canvas_direct_http_image_materialize_failure(
                    payload,
                    req,
                    model,
                    runtime,
                    prompt,
                    timeout,
                    image_json_policy,
                    download_error,
                )
                .await
            }
        }
    }

    async fn recover_gemini_canvas_direct_http_image_materialize_failure(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        timeout: Duration,
        image_json_policy: &GeminiCanvasDirectHttpImageJsonPolicy,
        download_error: GatewayError,
    ) -> Result<Value, GatewayError> {
        match image_json_policy.on_materialize_failure() {
            GeminiCanvasDirectHttpImageJsonAction::TryJsonWithErrorContext { context_key } => {
                match self
                    .execute_gemini_canvas_direct_http_image_json(
                        payload, req, model, runtime, prompt, timeout,
                    )
                    .await
                {
                    Ok(body) => Ok(body),
                    Err(image_json_error) => {
                        let image_json_summary = summarize_gateway_error(&image_json_error);
                        Err(append_gateway_error_summary(
                            download_error,
                            context_key,
                            Some(&image_json_summary),
                        ))
                    }
                }
            }
            GeminiCanvasDirectHttpImageJsonAction::ReturnOriginal
            | GeminiCanvasDirectHttpImageJsonAction::ReturnOriginalWithSummary { .. }
            | GeminiCanvasDirectHttpImageJsonAction::Skip
            | GeminiCanvasDirectHttpImageJsonAction::TryJson => Err(download_error),
        }
    }

    async fn build_gemini_canvas_materialized_image_response(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        request_timeout: Duration,
        selected_image_assets: &[&gemini_canvas::GeminiCanvasMediaAsset],
        provider: &str,
    ) -> Result<Value, GatewayError> {
        let downloaded = self
            .materialize_gemini_canvas_direct_http_images(
                payload,
                runtime,
                selected_image_assets,
                request_timeout,
                provider,
            )
            .await?;

        gemini_canvas::build_openai_images_response_from_bytes(req, prompt, &downloaded)
    }

    async fn materialize_gemini_canvas_direct_http_images(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        assets: &[&gemini_canvas::GeminiCanvasMediaAsset],
        request_timeout: Duration,
        provider: &str,
    ) -> Result<Vec<gemini_canvas::GeminiCanvasImage>, GatewayError> {
        let mut downloaded = Vec::with_capacity(assets.len());
        for asset in assets {
            if let Some(image) =
                gemini_canvas_web_reverse_modular::decode_direct_http_inline_image_asset(
                    provider, asset,
                )?
            {
                downloaded.push(image);
                continue;
            }

            let image = self
                .fetch_gemini_canvas_direct_http_image_asset(
                    payload,
                    runtime,
                    asset,
                    request_timeout,
                )
                .await?;
            downloaded.push(image);
        }
        Ok(downloaded)
    }

    async fn fetch_gemini_canvas_direct_http_image_asset(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        asset: &gemini_canvas::GeminiCanvasMediaAsset,
        timeout: Duration,
    ) -> Result<gemini_canvas::GeminiCanvasImage, GatewayError> {
        let fetched_asset = match self
            .materialize_gemini_canvas_direct_http_media_asset(
                payload,
                runtime,
                &asset.url,
                Some("image"),
                Some(&asset.mime_type),
                timeout,
            )
            .await
        {
            Ok(asset) => asset,
            Err(download_error) => {
                if payload.adapter == "gemini_canvas_program_web_reverse_compatible" {
                    return Err(download_error);
                }
                let browser_runtime_state_object_key =
                    gemini_canvas::browser_runtime_state_object_key_for_browser_operation(
                        payload, "image",
                    )
                    .unwrap_or_else(|| runtime.runtime_state_object_key.clone());
                let browser_cdp_url = gemini_canvas::browser_cdp_url(payload);
                let browser_cookie_header = gemini_canvas::browser_cookie_header(payload);
                let browser_pool_base_url = self
                    .ensure_gemini_canvas_browser_pool(payload.adapter.as_str())
                    .await?;
                let (bytes, response_content_type) =
                    gemini_canvas_web_reverse_modular::execute_connected_fetch_get_bytes(
                        &self.http,
                        timeout,
                        payload.adapter.as_str(),
                        &browser_pool_base_url,
                        payload.base_url.trim_end_matches('/'),
                        &runtime.share_id,
                        &browser_runtime_state_object_key,
                        browser_cdp_url.as_deref(),
                        browser_cookie_header.as_deref(),
                        &asset.url,
                    )
                    .await
                    .map_err(|browser_fetch_error| {
                        let mut browser_fetch_error = browser_fetch_error;
                        browser_fetch_error.message = format!(
                            "{}; direct_http_download={}",
                            browser_fetch_error.message,
                            summarize_gateway_error(&download_error)
                        );
                        browser_fetch_error
                    })?;
                return Ok(
                    gemini_canvas_web_reverse_modular::build_downloaded_image_from_bytes(
                        asset,
                        response_content_type.as_deref(),
                        &bytes,
                    ),
                );
            }
        };
        let body_base64 = fetched_asset
            .body_base64
            .as_deref()
            .ok_or_else(gemini_canvas_image_fetch_missing_inline_bytes_error)?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(body_base64)
            .map_err(|error| {
                gemini_canvas_image_fetch_invalid_inline_bytes_error(error.to_string().as_str())
            })?;
        let mime_type = if fetched_asset.mime_type.starts_with("image/") {
            fetched_asset.mime_type
        } else {
            sniff_image_mime_type_from_bytes(&bytes)
                .map(str::to_string)
                .unwrap_or_else(|| asset.mime_type.clone())
        };
        Ok(gemini_canvas::GeminiCanvasImage { mime_type, bytes })
    }

    async fn execute_gemini_canvas_generate_content_json(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
    ) -> Result<Value, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let runtime = gemini_canvas::runtime_from_payload(payload)?;
        let browser_runtime_state_object_key =
            resolved_gemini_canvas_browser_runtime_state_object_key(payload, &runtime);
        let browser_cdp_url = gemini_canvas::browser_cdp_url(payload);
        let browser_cookie_header = gemini_canvas::browser_cookie_header(payload);
        let model = gemini_canvas::resolve_text_model(model)?;
        let request_timeout = self.timeout.max(Duration::from_secs(300));
        let prompt = gemini_canvas::prompt_for_text_request(
            req,
            "Gemini Canvas requests require a prompt.",
            "missing_gemini_canvas_text_prompt",
        )?;
        let mut direct_http_failure_summary: Option<String> = None;
        if gemini_canvas::pure_http_enabled(payload) {
            let primary_stream_error = match self
                .execute_gemini_canvas_direct_http_stream_generate_text(
                    payload,
                    req,
                    model,
                    &runtime,
                    &prompt,
                    request_timeout,
                )
                .await
            {
                Ok(canonical) => {
                    if !gemini_canvas_text_response_is_generic_welcome(&prompt, &canonical.text) {
                        return Ok(build_gemini_canvas_text_success_body(
                            req,
                            model,
                            &canonical.text,
                            canonical.usage.as_ref(),
                            &canonical.tool_calls,
                        ));
                    }
                    let error = gemini_canvas_generic_welcome_response_error(provider);
                    debug!(
                        provider,
                        "Gemini Canvas StreamGenerate returned a generic welcome; falling back to generateContent JSON direct HTTP"
                    );
                    error
                }
                Err(error) => {
                    let stream_summary = summarize_gateway_error(&error);
                    debug!(
                        provider,
                        error = %stream_summary,
                        "Gemini Canvas StreamGenerate direct HTTP failed; falling back to generateContent JSON direct HTTP"
                    );
                    error
                }
            };

            let request_body = gemini_canvas::build_text_request_body(req, model);
            let request_url = gemini_canvas::build_text_fetch_url(&runtime, model);
            let mut fallback_failures = Vec::new();
            let runtime_api = if payload.api_key.trim().is_empty() {
                match self
                    .prepare_gemini_canvas_runtime_api_payload(payload, &runtime, request_timeout)
                    .await
                {
                    Ok(context) => Some(context),
                    Err(error) => {
                        let summary = summarize_gateway_error(&error);
                        debug!(
                            provider,
                            error = %summary,
                            "Gemini Canvas generateContent text fallback could not prepare runtime API key candidates; retrying legacy direct HTTP request"
                        );
                        fallback_failures.push(format!("runtime_api_prepare={summary}"));
                        None
                    }
                }
            } else {
                None
            };
            let attempts = build_gemini_canvas_text_direct_http_fallback_attempts(
                payload,
                runtime_api.as_ref(),
            );
            let mut last_fallback_error: Option<GatewayError> = None;

            for attempt in attempts {
                let transports: Vec<GeminiCanvasDirectHttpApiKeyTransport> =
                    if attempt.api_key_override.is_some() {
                        gemini_canvas_direct_http_api_key_transports(&request_url).to_vec()
                    } else {
                        vec![GeminiCanvasDirectHttpApiKeyTransport::HeaderOnly]
                    };
                for transport in transports {
                    match self
                        .execute_gemini_canvas_direct_http_json_with_options(
                            payload,
                            &runtime,
                            &request_url,
                            &request_body,
                            request_timeout,
                            attempt.api_key_override,
                            transport,
                            None,
                            attempt.referer_override,
                            false,
                            attempt.preserve_cross_origin_referer,
                            attempt.include_signed_headers,
                        )
                        .await
                    {
                        Ok(body) => {
                            let canonical =
                                gemini_api_modular::parse_generate_content_response(&body, model)?;
                            if gemini_canvas_text_response_is_generic_welcome(
                                &prompt,
                                &canonical.text,
                            ) {
                                let error = gemini_canvas_generic_welcome_response_error(provider);
                                fallback_failures.push(format!(
                                    "{}[transport={}]={}",
                                    attempt.label,
                                    transport.label(),
                                    summarize_gateway_error(&error)
                                ));
                                last_fallback_error = Some(error);
                                continue;
                            }
                            return Ok(build_gemini_canvas_text_success_body(
                                req,
                                model,
                                &canonical.text,
                                canonical.usage.as_ref(),
                                &canonical.tool_calls,
                            ));
                        }
                        Err(error) => {
                            fallback_failures.push(format!(
                                "{}[transport={}]={}",
                                attempt.label,
                                transport.label(),
                                summarize_gateway_error(&error)
                            ));
                            last_fallback_error = Some(error);
                        }
                    }
                }
            }

            let mut fallback_error = last_fallback_error
                .expect("Gemini Canvas text direct HTTP fallback attempts should record an error");
            if !fallback_failures.is_empty() {
                fallback_error.message = format!(
                    "{}; direct_http_fallback_attempts={}",
                    fallback_error.message,
                    fallback_failures.join(" | ")
                );
            }
            fallback_error.message = format!(
                "{}; StreamGenerate primary failure: {}",
                fallback_error.message,
                summarize_gateway_error(&primary_stream_error)
            );
            let summary = summarize_gateway_error(&fallback_error);
            debug!(
                provider,
                error = %summary,
                "Gemini Canvas direct HTTP text fallback failed; retrying through browser-backed invocation"
            );
            direct_http_failure_summary = Some(summary);
        }

        let locale = gemini_canvas::locale_from_payload(payload);
        let browser_pool_base_url = self.ensure_gemini_canvas_browser_pool(provider).await?;
        let invocation = self
            .execute_gemini_canvas_browser_request_with_recovery(
                provider,
                &browser_pool_base_url,
                payload.base_url.trim_end_matches('/'),
                &runtime.share_id,
                &browser_runtime_state_object_key,
                browser_cdp_url.as_deref(),
                browser_cookie_header.as_deref(),
                "text",
                &prompt,
                &locale,
                request_timeout,
            )
            .await
            .map_err(|error| {
                if let Some(summary) = direct_http_failure_summary.as_deref() {
                    let mut error = error;
                    error.message = format!("{}; direct_http_failure={summary}", error.message);
                    return error;
                }
                error
            })?;
        let raw_text = gemini_canvas_web_reverse_modular::extract_text_or_body_text(&invocation);
        if gemini_canvas_text_response_is_generic_welcome(&prompt, &raw_text) {
            return Err(gemini_canvas_generic_welcome_response_error(provider));
        }
        Ok(build_gemini_canvas_text_success_body(
            req,
            model,
            &raw_text,
            None,
            &[],
        ))
    }

    async fn prepare_gemini_canvas_runtime_api_payload(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        timeout: Duration,
    ) -> Result<GeminiCanvasRuntimeApiContext, GatewayError> {
        let base_url = payload.base_url.trim_end_matches('/');
        let harvest_target_url = gemini_canvas::direct_http_referrer(base_url, &runtime.share_id);
        self.prepare_gemini_canvas_runtime_api_payload_for_target(
            payload,
            runtime,
            &harvest_target_url,
            timeout,
        )
        .await
    }

    async fn prepare_gemini_canvas_runtime_api_payload_for_target(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        harvest_target_url: &str,
        timeout: Duration,
    ) -> Result<GeminiCanvasRuntimeApiContext, GatewayError> {
        let storage_state = gateway_object_storage()?
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
        let base_url = payload.base_url.trim_end_matches('/');
        let explicit_cookie_header = payload
            .extra_body
            .as_ref()
            .and_then(|extra| {
                extra
                    .get("canvasProgramInvokeContract")
                    .and_then(Value::as_object)
                    .and_then(|contract| contract.get("cookieHeader"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| {
                payload
                    .extra_body
                    .as_ref()
                    .and_then(|extra| extra.get("cookieHeader"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| {
                payload
                    .extra_body
                    .as_ref()
                    .and_then(|extra| {
                        extra
                            .get("canvasProgramInvokeContract")
                            .and_then(Value::as_object)
                            .and_then(|contract| contract.get("cookie_header"))
                            .and_then(Value::as_str)
                    })
                    .map(str::to_string)
            })
            .or_else(|| {
                payload
                    .extra_body
                    .as_ref()
                    .and_then(|extra| extra.get("cookie_header"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| read_json_string(&storage_state, "cookieHeader"));
        let session = if let Some(cookie_header) = explicit_cookie_header
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            gemini_canvas::pure_http_session_from_cookie_header(cookie_header, &auth_user)?
        } else {
            gemini_canvas::storage_state_to_pure_http_session(
                &storage_state,
                harvest_target_url,
                base_url,
                &auth_user,
            )?
        };
        let page_origin = origin_from_url(harvest_target_url)
            .unwrap_or_else(|| gemini_canvas_http_origin(payload));
        let page_referer = if harvest_target_url.trim().is_empty() {
            format!("{}/", page_origin.trim_end_matches('/'))
        } else {
            harvest_target_url.to_string()
        };
        if payload.adapter == "gemini_canvas_program_web_reverse_compatible" {
            ensure_gemini_canvas_program_payload_avoids_official_api_identity(payload)?;
            let discovered_identity =
                gemini_canvas::direct_http_google_api_keys(payload, &storage_state);
            if !discovered_identity.is_empty() {
                return Err(
                    gemini_canvas_program_runtime_material_official_api_key_forbidden_error(),
                );
            }
            let mut runtime_api_payload = payload.clone();
            runtime_api_payload.base_url = runtime.api_base_url.trim_end_matches('/').to_string();
            runtime_api_payload.api_key.clear();
            return Ok(GeminiCanvasRuntimeApiContext {
                payload: runtime_api_payload,
                api_key_candidates: Vec::new(),
                session,
                page_origin: page_origin.clone(),
                page_referer: page_referer.clone(),
            });
        }
        let mut page_harvest_probes = Vec::new();
        let mut google_api_key_candidates =
            gemini_canvas::direct_http_google_api_keys(payload, &storage_state);
        if google_api_key_candidates.is_empty() {
            let (harvested, probes) = self
                .harvest_gemini_canvas_direct_http_api_keys(payload, runtime, &session, timeout)
                .await;
            page_harvest_probes = probes;
            for candidate in harvested {
                if !google_api_key_candidates
                    .iter()
                    .any(|existing| existing == &candidate)
                {
                    google_api_key_candidates.push(candidate);
                }
            }
        }
        let google_api_key = google_api_key_candidates.first().cloned().ok_or_else(|| {
            gemini_canvas_runtime_api_missing_google_api_key_error(&page_harvest_probes)
        })?;
        let mut runtime_api_payload = payload.clone();
        runtime_api_payload.base_url = runtime.api_base_url.trim_end_matches('/').to_string();
        runtime_api_payload.api_key = google_api_key;
        Ok(GeminiCanvasRuntimeApiContext {
            payload: runtime_api_payload,
            api_key_candidates: google_api_key_candidates,
            session,
            page_origin,
            page_referer,
        })
    }

    async fn prepare_gemini_canvas_program_app_endpoint_api_context(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        timeout: Duration,
    ) -> Result<GeminiCanvasProgramAppEndpointApiContext, GatewayError> {
        let relay_config =
            gemini_canvas_program_web_reverse_modular::relay_config_from_payload(payload)?;
        if !relay_config.has_concrete_handle() {
            return Err(
                gemini_canvas_program_web_reverse_modular::missing_gemini_canvas_program_app_endpoint_handle_error(
                    "gemini_canvas_program_web_reverse_compatible",
                ),
            );
        }
        let base_url = payload.base_url.trim_end_matches('/');
        let harvest_target_url =
            gemini_canvas_program_web_reverse_modular::preferred_app_endpoint_harvest_target_url(
                base_url,
                &relay_config,
            );
        let runtime_api = self
            .prepare_gemini_canvas_runtime_api_payload_for_target(
                payload,
                runtime,
                &harvest_target_url,
                timeout,
            )
            .await?;
        let page_url = gemini_canvas_program_web_reverse_modular::preferred_app_endpoint_page_url(
            base_url,
            &relay_config,
        )
        .unwrap_or_else(|| harvest_target_url.clone());
        let official_extra_headers =
            gemini_canvas_program_web_reverse_modular::build_program_app_endpoint_official_extra_headers(
                &gemini_canvas::locale_from_payload(payload),
                &runtime_api.session.auth_user,
                &page_url,
            );
        let invoke_base_url =
            gemini_canvas_program_web_reverse_modular::preferred_app_endpoint_invoke_base_url(
                &runtime.api_base_url,
                &relay_config,
            );
        Ok(GeminiCanvasProgramAppEndpointApiContext {
            relay_config,
            runtime_api,
            official_extra_headers,
            invoke_base_url,
        })
    }

    async fn execute_gemini_canvas_official_media(
        &self,
        _provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        gemini_api_modular::execute_official_media(
            &self.http,
            self.timeout,
            payload,
            req,
            model,
            extra_headers,
        )
        .await
    }

    async fn execute_producer_media(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        match req.endpoint_kind {
            EndpointKind::ImagesGenerations => {
                return self
                    .execute_producer_image(payload, req, model, extra_headers)
                    .await;
            }
            EndpointKind::MusicGenerations => {
                return self
                    .execute_producer_music(payload, req, model, extra_headers)
                    .await;
            }
            EndpointKind::VideosGenerations => {
                return self
                    .execute_producer_video(payload, req, model, extra_headers)
                    .await;
            }
            _ => {
                return Err(producer::unsupported_media_endpoint_error());
            }
        }
    }

    async fn execute_producer_image(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let headers = maybe_refresh_producer_headers(
            &self.http,
            &build_upstream_headers_with(payload, extra_headers),
        )
        .await;
        let base_url = payload.base_url.trim_end_matches('/');
        let request_timeout = self.timeout.max(Duration::from_secs(180));
        execute_producer_image_http(&self.http, base_url, &headers, req, model, request_timeout)
            .await
    }

    async fn execute_producer_browser_backed(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let provider = "producer_compatible";
        let headers = maybe_refresh_producer_headers(
            &self.http,
            &build_upstream_headers_with(payload, extra_headers),
        )
        .await;
        let request_timeout = self.timeout.max(Duration::from_secs(900));
        let prepared = prepare_producer_browser_execution_input(
            &payload.base_url,
            &headers,
            &req.raw_body,
            model,
            request_timeout,
        );
        if let Some(result) = self
            .execute_remote_browser_executor(
                "producer",
                provider_account_id,
                req.endpoint_kind,
                build_producer_browser_executor_payload_from_prepared(
                    &prepared,
                    std::env::var("PRODUCER_BROWSER_EXECUTABLE_PATH").ok(),
                ),
            )
            .await?
        {
            return Ok(result);
        }

        execute_producer_browser_worker(provider, &prepared, false).await
    }

    async fn execute_producer_music(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let headers = maybe_refresh_producer_headers(
            &self.http,
            &build_upstream_headers_with(payload, extra_headers),
        )
        .await;
        let base_url = payload.base_url.trim_end_matches('/');
        let request_timeout = self.timeout.max(Duration::from_secs(180));
        execute_producer_music_http(&self.http, base_url, &headers, req, model, request_timeout)
            .await
    }

    async fn execute_producer_video(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let provider = "producer_compatible";
        let headers = maybe_refresh_producer_headers(
            &self.http,
            &build_upstream_headers_with(payload, extra_headers),
        )
        .await;
        let request_timeout = self.timeout.max(Duration::from_secs(900));
        let prepared = prepare_producer_browser_execution_input(
            &payload.base_url,
            &headers,
            &req.raw_body,
            model,
            request_timeout,
        );
        match execute_producer_video_http(
            &self.http,
            &prepared.base_url,
            &prepared.headers,
            &prepared.request_body,
            &prepared.model,
            prepared.timeout,
        )
        .await
        {
            Ok(result) => Ok(result),
            Err(error) if should_fallback_producer_video_http_error(&error) => {
                debug!(
                    provider,
                    message = %error.message,
                    code = ?error.code,
                    status = ?error.http_status,
                    "producer direct-http video orchestration failed; falling back to browser worker"
                );
                execute_producer_browser_worker(provider, &prepared, true).await
            }
            Err(error) => Err(error),
        }
    }

    async fn execute_gemini_canvas_http_replay_worker(
        &self,
        provider: &str,
        template: &gemini_canvas::GeminiCanvasTextStreamGenerateTemplate,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        operation: Option<&str>,
        timeout: Duration,
    ) -> Result<GeminiCanvasHttpReplayWorkerSuccess, GatewayError> {
        let operation_kind = operation.and_then(|value| match value {
            "image" => Some(gemini_canvas::GeminiCanvasMediaOperation::Image),
            "music" => Some(gemini_canvas::GeminiCanvasMediaOperation::Music),
            "video" => Some(gemini_canvas::GeminiCanvasMediaOperation::Video),
            _ => None,
        });
        let mut effective_template = template.clone();
        let mut xsrf_retry_attempted = false;

        loop {
            if let Some(operation_kind) = operation_kind {
                let mut headers = HeaderMap::new();
                for (key, value) in &effective_template.headers {
                    insert_header_map_value(&mut headers, key, value);
                }
                apply_gemini_canvas_cookie_header(&mut headers, session);
                match self
                    .http
                    .request(Method::POST, &effective_template.url)
                    .headers(headers)
                    .query(&effective_template.query)
                    .timeout(timeout)
                    .body(effective_template.raw_post_data.clone())
                    .send()
                    .await
                {
                    Ok(response) => {
                        let status = response.status().as_u16();
                        let content_type = response
                            .headers()
                            .get(rquest::header::CONTENT_TYPE)
                            .and_then(|value| value.to_str().ok())
                            .map(str::to_string);
                        let body_text = self
                            .collect_gemini_canvas_stream_generate_body(
                                response,
                                provider,
                                operation_kind,
                                operation_kind != gemini_canvas::GeminiCanvasMediaOperation::Music,
                            )
                            .await?;
                        if status == 400 {
                            if let Some(token) =
                                maybe_retry_gemini_canvas_stream_template_access_token(
                                    &mut effective_template,
                                    &body_text,
                                    &mut xsrf_retry_attempted,
                                )
                            {
                                debug!(
                                    provider,
                                    operation = operation.unwrap_or(""),
                                    xsrf_token_preview = %truncate_response_preview(&token, 24),
                                    "retrying gemini canvas HTTP replay worker StreamGenerate with xsrf token extracted from upstream error"
                                );
                                continue;
                            }
                        }
                        if !body_text.is_empty() {
                            return Ok(GeminiCanvasHttpReplayWorkerSuccess {
                                status,
                                content_type,
                                body_text,
                            });
                        }
                        if status >= 400 {
                            debug!(
                                provider,
                                operation = operation.unwrap_or(""),
                                status,
                                "gemini canvas direct Rust StreamGenerate replay returned an empty error body; falling back to node worker for exact response capture"
                            );
                        } else {
                            debug!(
                                provider,
                                operation = operation.unwrap_or(""),
                                "gemini canvas direct Rust StreamGenerate replay returned an empty body; falling back to node worker"
                            );
                        }
                    }
                    Err(error) => {
                        debug!(
                            provider,
                            operation = operation.unwrap_or(""),
                            error = %summarize_gateway_error(&classify_network_error(&error, Some(provider))),
                            "gemini canvas direct Rust StreamGenerate replay failed; falling back to node worker"
                        );
                    }
                }
            }

            let script_path = gemini_canvas_http_replay_worker_script_path();
            let input = gemini_canvas_web_reverse_modular::build_http_replay_worker_input(
                &effective_template.url,
                &effective_template.query,
                &effective_template.headers,
                &effective_template.raw_post_data,
                &session.cookie_header,
                operation,
                timeout,
            );
            let stdin_json = serde_json::to_vec(&input).map_err(|error| {
                gemini_canvas_http_replay_worker_input_serialize_error(error.to_string().as_str())
            })?;
            maybe_dump_gemini_canvas_http_replay_worker_debug(
                "input",
                operation.unwrap_or("unknown"),
                &stdin_json,
            );

            let mut child = Command::new(
                std::env::var("GEMINI_CANVAS_HTTP_NODE_BIN").unwrap_or_else(|_| "node".to_string()),
            )
            .arg(&script_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                gemini_canvas_http_replay_worker_spawn_failed_error(
                    script_path.as_path(),
                    error.to_string().as_str(),
                )
            })?;

            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(&stdin_json).await.map_err(|error| {
                    gemini_canvas_http_replay_worker_stdin_error(error.to_string().as_str())
                })?;
            }

            let output = tokio::time::timeout(timeout, child.wait_with_output())
                .await
                .map_err(|_| gemini_canvas_http_replay_worker_timeout_error())?
                .map_err(|error| {
                    gemini_canvas_http_replay_worker_wait_failed_error(error.to_string().as_str())
                })?;

            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            maybe_dump_gemini_canvas_http_replay_worker_debug(
                "stdout",
                operation.unwrap_or("unknown"),
                stdout.as_bytes(),
            );
            if !stderr.is_empty() {
                maybe_dump_gemini_canvas_http_replay_worker_debug(
                    "stderr",
                    operation.unwrap_or("unknown"),
                    stderr.as_bytes(),
                );
            }
            let result = gemini_canvas_web_reverse_modular::parse_http_replay_worker_output(
                &stdout, &stderr,
            )?;

            if result.ok {
                let success =
                    gemini_canvas_web_reverse_modular::extract_http_replay_worker_success(result)?;
                if success.status == 400 {
                    if let Some(token) = maybe_retry_gemini_canvas_stream_template_access_token(
                        &mut effective_template,
                        &success.body_text,
                        &mut xsrf_retry_attempted,
                    ) {
                        debug!(
                            provider,
                            operation = operation.unwrap_or(""),
                            xsrf_token_preview = %truncate_response_preview(&token, 24),
                            "retrying gemini canvas HTTP replay worker node StreamGenerate with xsrf token extracted from upstream error"
                        );
                        continue;
                    }
                }
                return Ok(success);
            }

            let status = result
                .error
                .as_ref()
                .and_then(|entry| entry.status)
                .or(result.status)
                .unwrap_or(500);
            let body_text = result
                .error
                .as_ref()
                .and_then(|entry| entry.body_text.as_deref())
                .or_else(|| stderr.is_empty().then_some("").or(Some(stderr.as_str())))
                .unwrap_or_default();
            if status == 400 {
                if let Some(token) = maybe_retry_gemini_canvas_stream_template_access_token(
                    &mut effective_template,
                    &body_text,
                    &mut xsrf_retry_attempted,
                ) {
                    debug!(
                        provider,
                        operation = operation.unwrap_or(""),
                        xsrf_token_preview = %truncate_response_preview(&token, 24),
                        "retrying gemini canvas HTTP replay worker error path with xsrf token extracted from upstream error"
                    );
                    continue;
                }
            }
            return Err(
                gemini_canvas_web_reverse_modular::classify_http_replay_worker_failure(
                    result, &stderr, provider,
                ),
            );
        }
    }

    async fn ensure_gemini_canvas_browser_pool(
        &self,
        _provider: &str,
    ) -> Result<String, GatewayError> {
        let base_url = gemini_canvas_browser_pool_base_url();
        if self
            .http
            .request(Method::GET, format!("{}/health", base_url))
            .timeout(Duration::from_secs(2))
            .send()
            .await
            .ok()
            .is_some_and(|response| response.status().is_success())
        {
            return Ok(base_url);
        }

        let _startup_guard = gemini_canvas_browser_pool_start_mutex().lock().await;
        if self
            .http
            .request(Method::GET, format!("{}/health", base_url))
            .timeout(Duration::from_secs(2))
            .send()
            .await
            .ok()
            .is_some_and(|response| response.status().is_success())
        {
            return Ok(base_url);
        }

        let script_path = gemini_canvas_browser_pool_script_path();
        let log_path = gemini_canvas_browser_pool_log_path();
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let stdout = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map(Stdio::from)
            .map_err(|error| {
                gemini_canvas_browser_pool_log_open_error(
                    log_path.as_path(),
                    error.to_string().as_str(),
                )
            })?;
        let stderr = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map(Stdio::from)
            .map_err(|error| {
                gemini_canvas_browser_pool_log_open_error(
                    log_path.as_path(),
                    error.to_string().as_str(),
                )
            })?;
        Command::new(
            std::env::var("GEMINI_CANVAS_BROWSER_NODE_BIN").unwrap_or_else(|_| "node".to_string()),
        )
        .arg(&script_path)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .map_err(|error| {
            gemini_canvas_browser_pool_spawn_failed_error(
                script_path.as_path(),
                error.to_string().as_str(),
            )
        })?;

        let deadline = std::time::Instant::now() + Duration::from_secs(45);
        loop {
            if std::time::Instant::now() >= deadline {
                return Err(gemini_canvas_browser_pool_start_timeout_error(
                    log_path.as_path(),
                ));
            }

            if self
                .http
                .request(Method::GET, format!("{}/health", base_url))
                .timeout(Duration::from_secs(2))
                .send()
                .await
                .ok()
                .is_some_and(|response| response.status().is_success())
            {
                return Ok(base_url);
            }

            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    }

    async fn execute_gemini_canvas_browser_request(
        &self,
        provider: &str,
        browser_pool_base_url: &str,
        base_url: &str,
        share_id: &str,
        runtime_state_object_key: &str,
        browser_cdp_url: Option<&str>,
        cookie_header: Option<&str>,
        operation: &str,
        prompt: &str,
        locale: &str,
        timeout: Duration,
    ) -> Result<GeminiCanvasBrowserInvocationResult, GatewayError> {
        gemini_canvas_web_reverse_modular::execute_browser_request(
            &self.http,
            timeout,
            provider,
            browser_pool_base_url,
            base_url,
            share_id,
            runtime_state_object_key,
            browser_cdp_url,
            cookie_header,
            operation,
            prompt,
            locale,
        )
        .await
    }

    async fn execute_gemini_canvas_connected_fetch_json_with_mode(
        &self,
        provider: &str,
        browser_pool_base_url: &str,
        base_url: &str,
        share_id: &str,
        runtime_state_object_key: &str,
        browser_cdp_url: Option<&str>,
        cookie_header: Option<&str>,
        request_url: &str,
        request_body: &Value,
        google_fetch_mode: &str,
        timeout: Duration,
    ) -> Result<Value, GatewayError> {
        gemini_canvas_web_reverse_modular::execute_connected_fetch_json_with_mode(
            &self.http,
            timeout,
            provider,
            browser_pool_base_url,
            base_url,
            share_id,
            runtime_state_object_key,
            browser_cdp_url,
            cookie_header,
            request_url,
            request_body,
            google_fetch_mode,
        )
        .await
    }

    // Direct HTTP helpers for the active `gemini_canvas_compatible` hot path.
    // Legacy browser-connected and browser-pool flows remain below as explicit
    // compatibility fallbacks when pure HTTP replay is disabled.
    async fn execute_gemini_canvas_direct_http_json_with_options(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        request_url: &str,
        request_body: &Value,
        timeout: Duration,
        api_key_override: Option<&str>,
        api_key_transport: GeminiCanvasDirectHttpApiKeyTransport,
        signed_origin_override: Option<&str>,
        referer_override: Option<&str>,
        preserve_cross_origin_origin: bool,
        preserve_cross_origin_referer: bool,
        include_signed_headers: bool,
    ) -> Result<Value, GatewayError> {
        send_gemini_canvas_direct_http_json_with_options(
            &self.http,
            payload,
            runtime,
            request_url,
            request_body,
            timeout,
            api_key_override,
            api_key_transport,
            signed_origin_override,
            referer_override,
            preserve_cross_origin_origin,
            preserve_cross_origin_referer,
            include_signed_headers,
        )
        .await
    }

    async fn upload_gemini_canvas_image_edit_inputs(
        &self,
        payload: &ProviderAccountPayload,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        uploads: &[gemini_canvas::GeminiCanvasImageEditUpload],
        timeout: Duration,
    ) -> Result<Vec<gemini_canvas::GeminiCanvasUploadedFileRef>, GatewayError> {
        upload_gemini_canvas_image_edit_inputs_with_http(
            &self.http, payload, session, bootstrap, uploads, timeout,
        )
        .await
    }

    async fn send_gemini_canvas_signaler_poll_request_refreshing_session(
        &self,
        payload: &ProviderAccountPayload,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        url: &str,
        timeout: Duration,
        aid_hint: u64,
        locale_override: Option<&str>,
    ) -> Result<String, GatewayError> {
        send_gemini_canvas_signaler_poll_request_refreshing_session_with_http(
            &self.http,
            payload,
            session,
            url,
            timeout,
            aid_hint,
            locale_override,
        )
        .await
    }

    async fn prewarm_gemini_canvas_image_edit_signaler(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        locale_override: Option<&str>,
        timeout: Duration,
    ) -> Result<GeminiCanvasSignalerChannel, GatewayError> {
        prewarm_gemini_canvas_image_edit_signaler_with_http(
            &self.http,
            &self.plain_http,
            payload,
            runtime,
            session,
            locale_override,
            timeout,
        )
        .await
    }

    async fn poll_gemini_canvas_image_edit_signaler_assets(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        timeout: Duration,
        mut edit_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
    ) -> Result<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String), GatewayError> {
        let base_url = payload.base_url.trim_end_matches('/');
        let app_url = format!("{base_url}{}", gemini_web::GEMINI_WEB_DEFAULT_APP_PATH);
        let storage_state = gateway_object_storage()?
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        let locale_hint = edit_context
            .as_ref()
            .and_then(|context| context.locale_hint.clone())
            .or_else(|| gemini_canvas::harvest_image_edit_template_locale(&storage_state));
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
        let (mut session, mut channel) =
            prepare_gemini_canvas_image_edit_signaler_poll_state_with_http(
                &self.http,
                &self.plain_http,
                payload,
                runtime,
                &storage_state,
                base_url,
                &app_url,
                &auth_user,
                locale_hint.as_deref(),
                edit_context.as_ref().map(|context| &**context),
                timeout,
            )
            .await?;
        let started_at = Instant::now();
        let total_budget = timeout
            .min(Duration::from_secs(540))
            .max(Duration::from_secs(270));
        let mut last_body_preview = None;
        let mut failures = Vec::new();
        let mut seen_app_paths = HashSet::new();
        let mut first_app_path_seen_at: Option<Instant> = None;

        loop {
            let Some(remaining) = total_budget.checked_sub(started_at.elapsed()) else {
                break;
            };
            if remaining <= Duration::from_secs(3) {
                break;
            }
            let poll_url = build_gemini_canvas_image_edit_signaler_poll_url(
                &channel,
                &gemini_canvas_signaler_zx_token(),
            );
            match self
                .send_gemini_canvas_signaler_poll_request_refreshing_session(
                    payload,
                    &mut session,
                    &poll_url,
                    remaining.min(Duration::from_secs(285)),
                    channel.next_aid,
                    locale_hint.as_deref(),
                )
                .await
            {
                Ok(body) => {
                    last_body_preview =
                        Some(record_gemini_canvas_image_edit_signaler_poll_body_preview(
                            edit_context.as_deref_mut(),
                            &body,
                        ));
                    if let Ok(assets) = extract_gemini_canvas_image_edit_signaler_assets_from_body(
                        &body,
                        &session,
                        &channel,
                        locale_hint.as_deref(),
                        edit_context.as_deref_mut(),
                    ) {
                        return Ok((assets, body));
                    }
                    for app_path in gemini_canvas::extract_signaler_app_paths(&body) {
                        let page_url = record_gemini_canvas_image_edit_signaler_app_path(
                            base_url,
                            app_path.as_str(),
                            &mut seen_app_paths,
                            &mut first_app_path_seen_at,
                            edit_context.as_deref_mut(),
                        );
                        match self
                            .fetch_gemini_canvas_direct_http_page_html_with_locale(
                                payload,
                                &session,
                                &page_url,
                                remaining.min(Duration::from_secs(20)),
                                locale_hint.as_deref(),
                            )
                            .await
                        {
                            Ok(page_body) => {
                                if let Some((assets, page_body)) =
                                    try_extract_gemini_canvas_image_edit_signaler_assets_response_from_body(
                                        page_body,
                                        &session,
                                        &channel,
                                        locale_hint.as_deref(),
                                        edit_context.as_deref_mut(),
                                    )
                                {
                                    return Ok((assets, page_body));
                                }
                                match self
                                    .trigger_gemini_canvas_image_page_refresh(
                                        payload,
                                        model,
                                        &storage_state,
                                        &mut session,
                                        &page_url,
                                        remaining.min(Duration::from_secs(20)),
                                    )
                                    .await
                                {
                                    Ok(refresh_preview) => {
                                        failures.push(
                                            gemini_canvas_image_edit_signaler_page_failure_entry(
                                                &page_url,
                                                "signaler_page_refresh",
                                                compact_response_preview(&refresh_preview, 220),
                                            ),
                                        );
                                        match self
                                            .fetch_gemini_canvas_direct_http_page_html_with_locale(
                                                payload,
                                                &session,
                                                &page_url,
                                                remaining.min(Duration::from_secs(20)),
                                                locale_hint.as_deref(),
                                            )
                                            .await
                                        {
                                            Ok(refreshed_page_body) => {
                                                if let Some((assets, refreshed_page_body)) =
                                                    try_extract_gemini_canvas_image_edit_signaler_assets_response_from_body(
                                                        refreshed_page_body,
                                                        &session,
                                                        &channel,
                                                        locale_hint.as_deref(),
                                                        edit_context.as_deref_mut(),
                                                    )
                                                {
                                                    return Ok((assets, refreshed_page_body));
                                                }
                                            }
                                            Err(error) => {
                                                failures.push(
                                                    gemini_canvas_image_edit_signaler_page_failure_entry(
                                                    &page_url,
                                                    "refetch",
                                                    summarize_gateway_error(&error)
                                                    ),
                                                );
                                            }
                                        }
                                    }
                                    Err(error) => {
                                        failures.push(
                                            gemini_canvas_image_edit_signaler_page_failure_entry(
                                                &page_url,
                                                "signaler_page_refresh",
                                                summarize_gateway_error(&error),
                                            ),
                                        );
                                    }
                                }
                            }
                            Err(error) => {
                                failures.push(
                                    gemini_canvas_image_edit_signaler_page_failure_entry(
                                        &page_url,
                                        "fetch",
                                        summarize_gateway_error(&error),
                                    ),
                                );
                            }
                        }
                    }
                    if let Some(handoff_error) =
                        try_finish_gemini_canvas_image_edit_signaler_handoff_ready(
                            seen_app_paths.len(),
                            first_app_path_seen_at.map(|first_seen_at| first_seen_at.elapsed()),
                            last_body_preview.as_deref(),
                            edit_context.as_deref_mut(),
                            &session,
                            &channel,
                            locale_hint.as_deref(),
                            &body,
                        )
                    {
                        return Err(handoff_error);
                    }
                    match refresh_gemini_canvas_image_edit_signaler_creds_from_body_with_http(
                        &self.http,
                        payload,
                        &mut session,
                        &channel,
                        &body,
                        remaining.min(Duration::from_secs(30)),
                        locale_hint.as_deref(),
                    )
                    .await
                    {
                        Ok(_) => {}
                        Err(error) => {
                            failures.push(gemini_canvas_image_edit_signaler_refresh_error_entry(
                                channel.next_aid,
                                summarize_gateway_error(&error),
                            ));
                        }
                    }
                    update_gemini_canvas_image_edit_signaler_next_aid_from_body(
                        &mut channel,
                        &body,
                    );
                }
                Err(error) => {
                    failures.push(gemini_canvas_image_edit_signaler_poll_error_entry(
                        channel.next_aid,
                        summarize_gateway_error(&error),
                    ));
                    sleep(Duration::from_millis(800)).await;
                }
            }
        }
        Err(finish_gemini_canvas_image_edit_signaler_missing_asset(
            edit_context.as_deref_mut(),
            &session,
            &channel,
            locale_hint.as_deref(),
            &failures,
            last_body_preview,
        ))
    }

    async fn execute_gemini_canvas_direct_http_stream_generate_body(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        mode_index: i64,
        prompt: &str,
        timeout: Duration,
        allow_replay_template: bool,
        image_edit_uploads: Option<&[gemini_canvas::GeminiCanvasImageEditUpload]>,
        mut image_edit_followup_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
    ) -> Result<String, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let configured_base_url = payload.base_url.trim_end_matches('/');
        let effective_base_url = if configured_base_url.is_empty() {
            gemini_canvas_http_origin(payload)
        } else {
            configured_base_url.to_string()
        };
        let app_bootstrap_url = if payload.adapter == "gemini_canvas_program_web_reverse_compatible"
        {
            gemini_canvas_program_web_reverse_modular::gemini_canvas_program_payload_page_url(
                payload,
                &effective_base_url,
            )
            .unwrap_or_else(|| {
                format!(
                    "{}{}",
                    effective_base_url,
                    gemini_web::GEMINI_WEB_DEFAULT_APP_PATH
                )
            })
        } else {
            format!(
                "{}{}",
                effective_base_url,
                gemini_web::GEMINI_WEB_DEFAULT_APP_PATH
            )
        };
        let share_bootstrap_url =
            gemini_canvas::direct_http_referrer(&effective_base_url, &runtime.share_id);
        let is_text_mode =
            mode_index == gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX;
        let is_image_mode =
            mode_index == gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX;
        let is_image_edit_request = is_image_mode
            && image_edit_uploads
                .map(|uploads| !uploads.is_empty())
                .unwrap_or(false);
        let image_stream_timeout = if is_image_edit_request {
            // Real successful image-edit captures can spend multiple minutes in the
            // page-owned async generation flow before the final asset frames land.
            // The latest broad captures keep the same StreamGenerate request open
            // for roughly 9 minutes before downstream signaler/page settlement
            // has fully converged. Earlier 45s / 120s / 240-300s caps caused us
            // to preserve only the initial metadata, progress, or short-ack
            // frames, which looked like a completed response even though the
            // upstream stream was still active.
            timeout
                .min(Duration::from_secs(900))
                .max(Duration::from_secs(660))
        } else {
            timeout.max(Duration::from_secs(120))
        };
        let bootstrap_url = app_bootstrap_url.clone();
        let stream_url = format!(
            "{effective_base_url}{}",
            gemini_web::GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH
        );
        let object_storage = gateway_object_storage()?;
        let mut storage_state = object_storage
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        if is_image_edit_request
            && storage_state
                .get("imageEditStreamGenerateTemplate")
                .is_none()
        {
            let sidecar_key = gemini_canvas::image_edit_stream_generate_template_object_key(
                &runtime.runtime_state_object_key,
            );
            let remote_sidecar = object_storage.read_json(&sidecar_key).await.ok();
            let local_sidecar = read_gemini_canvas_runtime_mirror_json(&sidecar_key);
            let sidecar = if remote_sidecar
                .as_ref()
                .map(|value| {
                    gemini_canvas_sidecar_has_any_key(
                        value,
                        &["imageEditStreamGenerateTemplate", "imageEditStreamTemplate"],
                    )
                })
                .unwrap_or(false)
            {
                remote_sidecar
            } else if local_sidecar
                .as_ref()
                .map(|value| {
                    gemini_canvas_sidecar_has_any_key(
                        value,
                        &["imageEditStreamGenerateTemplate", "imageEditStreamTemplate"],
                    )
                })
                .unwrap_or(false)
            {
                local_sidecar
            } else {
                remote_sidecar.or(local_sidecar)
            };
            if let Some(sidecar) = sidecar {
                let maybe_template = sidecar
                    .get("imageEditStreamGenerateTemplate")
                    .cloned()
                    .or_else(|| {
                        sidecar
                            .get("url")
                            .and_then(Value::as_str)
                            .map(|_| sidecar.clone())
                    });
                if let Some(template) = maybe_template {
                    if let Some(root) = storage_state.as_object_mut() {
                        root.insert("imageEditStreamGenerateTemplate".to_string(), template);
                    }
                }
            }
        }
        if is_image_mode && storage_state.get("imageStreamGenerateTemplate").is_none() {
            let sidecar_key = gemini_canvas::image_stream_generate_template_object_key(
                &runtime.runtime_state_object_key,
            );
            let remote_sidecar = object_storage.read_json(&sidecar_key).await.ok();
            let local_sidecar = read_gemini_canvas_runtime_mirror_json(&sidecar_key);
            let sidecar = if remote_sidecar
                .as_ref()
                .map(|value| {
                    gemini_canvas_sidecar_has_any_key(
                        value,
                        &["imageStreamGenerateTemplate", "mediaStreamGenerateTemplate"],
                    )
                })
                .unwrap_or(false)
            {
                remote_sidecar
            } else if local_sidecar
                .as_ref()
                .map(|value| {
                    gemini_canvas_sidecar_has_any_key(
                        value,
                        &["imageStreamGenerateTemplate", "mediaStreamGenerateTemplate"],
                    )
                })
                .unwrap_or(false)
            {
                local_sidecar
            } else {
                remote_sidecar.or(local_sidecar)
            };
            if let Some(sidecar) = sidecar {
                let maybe_template =
                    sidecar
                        .get("imageStreamGenerateTemplate")
                        .cloned()
                        .or_else(|| {
                            sidecar
                                .get("url")
                                .and_then(Value::as_str)
                                .map(|_| sidecar.clone())
                        });
                if let Some(template) = maybe_template {
                    if let Some(root) = storage_state.as_object_mut() {
                        root.insert("imageStreamGenerateTemplate".to_string(), template);
                    }
                }
            }
        }
        let batchexecute_header_id = if is_image_edit_request {
            Some(gemini_canvas::new_batchexecute_header_id())
        } else {
            gemini_canvas::harvest_text_batchexecute_header_id(&storage_state)
        };
        let replay_template = if allow_replay_template {
            if is_text_mode {
                match gemini_canvas::build_text_stream_generate_request_from_template(
                    &storage_state,
                    prompt,
                ) {
                    Ok(template) => template,
                    Err(error) => {
                        debug!(
                            provider,
                            error = %summarize_gateway_error(&error),
                            "gemini canvas text replay template was invalid; falling back to bootstrap + legacy request builder"
                        );
                        None
                    }
                }
            } else if is_image_mode && !is_image_edit_request {
                match gemini_canvas::build_image_stream_generate_request_from_template(
                    &storage_state,
                    prompt,
                    "",
                ) {
                    Ok(template) => template,
                    Err(error) => {
                        debug!(
                            provider,
                            error = %summarize_gateway_error(&error),
                            "gemini canvas image replay template was invalid; falling back to bootstrap + legacy request builder"
                        );
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };
        let image_edit_locale_hint = if is_image_edit_request {
            gemini_canvas::harvest_image_edit_template_locale(&storage_state)
        } else {
            None
        };
        if let Some(context) = image_edit_followup_context.as_deref_mut() {
            if context.locale_hint.is_none() {
                context.locale_hint = image_edit_locale_hint.clone();
            }
        }
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
        let explicit_cookie_header = payload
            .extra_body
            .as_ref()
            .and_then(|extra| {
                extra
                    .get("canvasProgramInvokeContract")
                    .and_then(Value::as_object)
                    .and_then(|contract| contract.get("cookieHeader"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| {
                payload
                    .extra_body
                    .as_ref()
                    .and_then(|extra| extra.get("cookieHeader"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| {
                payload
                    .extra_body
                    .as_ref()
                    .and_then(|extra| {
                        extra
                            .get("canvasProgramInvokeContract")
                            .and_then(Value::as_object)
                            .and_then(|contract| contract.get("cookie_header"))
                            .and_then(Value::as_str)
                    })
                    .map(str::to_string)
            })
            .or_else(|| {
                payload
                    .extra_body
                    .as_ref()
                    .and_then(|extra| extra.get("cookie_header"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| read_json_string(&storage_state, "cookieHeader"));
        let session_target_url = replay_template
            .as_ref()
            .map(|template| template.url.as_str())
            .unwrap_or_else(|| {
                if is_text_mode || is_image_mode {
                    app_bootstrap_url.as_str()
                } else {
                    share_bootstrap_url.as_str()
                }
            });
        let mut session = if let Some(cookie_header) = explicit_cookie_header
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            gemini_canvas::pure_http_session_from_cookie_header(cookie_header, &auth_user)?
        } else {
            gemini_canvas::storage_state_to_pure_http_session(
                &storage_state,
                session_target_url,
                &effective_base_url,
                &auth_user,
            )?
        };
        if is_image_edit_request {
            append_gemini_canvas_image_edit_trace("prewarm.start", session_target_url);
            match self
                .prewarm_gemini_canvas_image_edit_signaler(
                    payload,
                    runtime,
                    &mut session,
                    image_edit_locale_hint.as_deref(),
                    timeout,
                )
                .await
            {
                Ok(channel) => {
                    append_gemini_canvas_image_edit_trace(
                        "prewarm.ok",
                        format!("next_aid={}", channel.next_aid),
                    );
                    if let Some(context) = image_edit_followup_context.as_deref_mut() {
                        context.signaler_session = Some(session.clone());
                        context.signaler_channel = Some(channel);
                    }
                }
                Err(error) => {
                    append_gemini_canvas_image_edit_trace(
                        "prewarm.err",
                        summarize_gateway_error(&error),
                    );
                    debug!(
                        provider,
                        error = %summarize_gateway_error(&error),
                        "gemini canvas image-edit signaler prewarm failed; continuing with direct HTTP StreamGenerate"
                    );
                }
            }
        }
        if is_image_mode {
            if let Some(template) = replay_template.as_ref() {
                match self
                    .execute_gemini_canvas_http_replay_worker(
                        provider,
                        template,
                        &session,
                        Some("image"),
                        image_stream_timeout,
                    )
                    .await
                {
                    Ok(worker_result)
                        if (200..300).contains(&worker_result.status)
                            && !gemini_web::response_indicates_browser_challenge(
                                worker_result.status,
                                worker_result.content_type.as_deref(),
                                &worker_result.body_text,
                            )
                            && !gemini_web::response_indicates_session_invalid(
                                worker_result.status,
                                worker_result.content_type.as_deref(),
                                &worker_result.body_text,
                            )
                            && !gemini_canvas::response_indicates_image_generation_unavailable(
                                worker_result.status,
                                worker_result.content_type.as_deref(),
                                &worker_result.body_text,
                            ) =>
                    {
                        if is_image_edit_request {
                            if let Some(context) = image_edit_followup_context.as_deref_mut() {
                                if context.signaler_response_id.is_none() {
                                    if let Some(response_id) =
                                        gemini_canvas::extract_stream_generate_response_id(
                                            &worker_result.body_text,
                                        )
                                        .ok()
                                    {
                                        context.signaler_response_id = Some(response_id);
                                    }
                                }
                            }
                            debug!(
                                provider,
                                status = worker_result.status,
                                "gemini canvas pure HTTP image-edit replay worker produced a non-challenge StreamGenerate body before bootstrap refresh; deferring asset resolution to follow-up extraction"
                            );
                            return Ok(worker_result.body_text);
                        }
                        match gemini_canvas::extract_stream_generate_media_assets(
                            &worker_result.body_text,
                            gemini_canvas::GeminiCanvasMediaOperation::Image,
                        ) {
                            Ok(assets) if !assets.is_empty() => {
                                debug!(
                                    provider,
                                    status = worker_result.status,
                                    asset_count = assets.len(),
                                    "gemini canvas pure HTTP image replay worker produced a parseable StreamGenerate body before bootstrap refresh"
                                );
                                return Ok(worker_result.body_text);
                            }
                            Ok(_) => {
                                debug!(
                                    provider,
                                    status = worker_result.status,
                                    "gemini canvas pure HTTP image replay worker returned no parseable image assets; falling back to direct Rust replay before bootstrap refresh"
                                );
                            }
                            Err(parse_error) => {
                                debug!(
                                    provider,
                                    status = worker_result.status,
                                    error = %summarize_gateway_error(&parse_error),
                                    "gemini canvas pure HTTP image replay worker returned a non-parseable StreamGenerate body; falling back to direct Rust replay before bootstrap refresh"
                                );
                            }
                        }
                    }
                    Ok(worker_result) => {
                        debug!(
                            provider,
                            status = worker_result.status,
                            content_type =
                                worker_result.content_type.as_deref().unwrap_or("<none>"),
                            "gemini canvas pure HTTP image replay worker returned a non-usable response; falling back to direct Rust replay before bootstrap refresh"
                        );
                    }
                    Err(error) => {
                        debug!(
                            provider,
                            error = %summarize_gateway_error(&error),
                            "gemini canvas pure HTTP image replay worker failed; falling back to direct Rust replay before bootstrap refresh"
                        );
                    }
                }
                let mut headers = HeaderMap::new();
                apply_gemini_canvas_replay_template_headers(
                    &mut headers,
                    &template.headers,
                    &session,
                );
                debug!(
                    provider,
                    url = %template.url,
                    "sending gemini canvas pure HTTP image StreamGenerate request via harvested replay template before bootstrap refresh"
                );
                let response = self
                    .http
                    .request(Method::POST, &template.url)
                    .headers(headers)
                    .query(&template.query)
                    .timeout(image_stream_timeout)
                    .body(template.raw_post_data.clone())
                    .send()
                    .await
                    .map_err(|error| classify_network_error(&error, Some(provider)))?;
                let status = response.status().as_u16();
                let content_type = response
                    .headers()
                    .get(rquest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string);
                let body_text = self
                    .collect_gemini_canvas_stream_generate_body(
                        response,
                        provider,
                        gemini_canvas::GeminiCanvasMediaOperation::Image,
                        is_image_edit_request,
                    )
                    .await?;
                if is_image_edit_request {
                    append_gemini_canvas_image_edit_stream_response_debug_snapshot(
                        "gemini-canvas-image-edit-stream-response-template-before-refresh",
                        "template-before-refresh-response",
                        &template.url,
                        status,
                        None,
                        content_type.as_deref(),
                        &body_text,
                        build_gemini_canvas_image_edit_stream_response_template_before_refresh_debug_extra(
                            &runtime.runtime_state_object_key,
                            allow_replay_template,
                        ),
                    );
                }
                if (200..300).contains(&status)
                    && !gemini_web::response_indicates_browser_challenge(
                        status,
                        content_type.as_deref(),
                        &body_text,
                    )
                    && !gemini_web::response_indicates_session_invalid(
                        status,
                        content_type.as_deref(),
                        &body_text,
                    )
                    && !gemini_canvas::response_indicates_image_generation_unavailable(
                        status,
                        content_type.as_deref(),
                        &body_text,
                    )
                {
                    if is_image_edit_request {
                        if let Some(context) = image_edit_followup_context.as_deref_mut() {
                            if context.signaler_response_id.is_none() {
                                if let Some(response_id) =
                                    gemini_canvas::extract_stream_generate_response_id(&body_text)
                                        .ok()
                                {
                                    context.signaler_response_id = Some(response_id);
                                }
                            }
                        }
                        debug!(
                            provider,
                            status,
                            "gemini canvas pure HTTP image-edit harvested replay template returned a non-challenge StreamGenerate body before bootstrap refresh; deferring asset resolution to follow-up extraction"
                        );
                        return Ok(body_text);
                    }
                    return Ok(body_text);
                }
                debug!(
                    provider,
                    status,
                    content_type = content_type.as_deref().unwrap_or("<none>"),
                    "gemini canvas pure HTTP image harvested replay template failed before bootstrap refresh; continuing with bootstrap + preflights"
                );
            }
        }
        if is_text_mode {
            if let Some(template) = replay_template.as_ref() {
                let mut headers = HeaderMap::new();
                apply_gemini_canvas_replay_template_headers(
                    &mut headers,
                    &template.headers,
                    &session,
                );
                debug!(
                    provider,
                    url = %template.url,
                    "sending gemini canvas pure HTTP StreamGenerate request via harvested replay template"
                );
                let response = self
                    .http
                    .request(Method::POST, &template.url)
                    .headers(headers)
                    .query(&template.query)
                    .timeout(timeout.max(Duration::from_secs(120)))
                    .form(&template.form)
                    .send()
                    .await
                    .map_err(|error| classify_network_error(&error, Some(provider)))?;
                let status = response.status().as_u16();
                let content_type = response
                    .headers()
                    .get(rquest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string);
                let body_text = self
                    .collect_gemini_canvas_stream_generate_body(
                        response,
                        provider,
                        gemini_canvas::GeminiCanvasMediaOperation::Image,
                        is_image_edit_request,
                    )
                    .await?;
                if !(200..300).contains(&status)
                    || gemini_web::response_indicates_browser_challenge(
                        status,
                        content_type.as_deref(),
                        &body_text,
                    )
                    || gemini_web::response_indicates_session_invalid(
                        status,
                        content_type.as_deref(),
                        &body_text,
                    )
                {
                    return Err(classify_gemini_canvas_pure_http_error(
                        status,
                        content_type.as_deref(),
                        &body_text,
                    ));
                }

                return Ok(body_text);
            }
        }
        let origin = gemini_canvas_http_origin(payload);
        let authorization = gemini_canvas::build_sapisid_authorization(
            &session.sapisid,
            &origin,
            current_unix_timestamp_i64(),
        )?;
        let (bootstrap_body, _bootstrap_request_contract, bootstrap_page_url) = if is_text_mode {
            let mut headers = HeaderMap::new();
            apply_gemini_canvas_navigation_headers(&mut headers);
            apply_gemini_canvas_cookie_header(&mut headers, &session);
            insert_header_map_value(
                &mut headers,
                "accept-language",
                &gemini_canvas::locale_from_payload(payload),
            );
            let bootstrap_request_contract =
                build_gemini_canvas_direct_http_text_bootstrap_request_contract(
                    &bootstrap_url,
                    &headers,
                );

            let bootstrap_response = self
                .http
                .request(Method::GET, &bootstrap_url)
                .headers(headers)
                .timeout(timeout.max(Duration::from_secs(30)))
                .send()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            let bootstrap_final_url = bootstrap_response.url().to_string();
            let bootstrap_status = bootstrap_response.status().as_u16();
            let bootstrap_location = bootstrap_response
                .headers()
                .get(rquest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let bootstrap_content_type = bootstrap_response
                .headers()
                .get(rquest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let bootstrap_body = bootstrap_response
                .text()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            let bootstrap_response_meta = build_gemini_canvas_direct_http_bootstrap_response_meta(
                &bootstrap_final_url,
                bootstrap_location.as_deref(),
                bootstrap_content_type.as_deref(),
                &bootstrap_body,
            );
            if !(200..300).contains(&bootstrap_status)
                || gemini_web::response_indicates_browser_challenge(
                    bootstrap_status,
                    bootstrap_content_type.as_deref(),
                    &bootstrap_body,
                )
                || gemini_web::response_indicates_session_invalid(
                    bootstrap_status,
                    bootstrap_content_type.as_deref(),
                    &bootstrap_body,
                )
            {
                return Err(
                    build_gemini_canvas_direct_http_text_bootstrap_failure_error(
                        bootstrap_status,
                        bootstrap_content_type.as_deref(),
                        &bootstrap_body,
                        &bootstrap_request_contract,
                        &bootstrap_response_meta,
                    ),
                );
            }

            (
                bootstrap_body,
                bootstrap_request_contract,
                bootstrap_final_url,
            )
        } else {
            let bootstrap_candidates = gemini_canvas_direct_http_bootstrap_candidates(
                payload,
                &effective_base_url,
                &runtime.share_id,
                is_image_mode,
            );
            let mut failures = Vec::new();
            let mut last_error = None;
            let mut selected_bootstrap: Option<(String, String)> = None;

            for candidate_url in &bootstrap_candidates {
                match self
                    .fetch_gemini_canvas_direct_http_page_html(
                        payload,
                        &session,
                        candidate_url,
                        timeout,
                    )
                    .await
                {
                    Ok(body) => {
                        selected_bootstrap = Some((body, candidate_url.to_string()));
                        break;
                    }
                    Err(error) => {
                        failures.push(format!(
                            "{}: {}",
                            candidate_url,
                            summarize_gateway_error(&error)
                        ));
                        last_error = Some(error);
                    }
                }
            }

            if let Some((body, selected_url)) = selected_bootstrap {
                let bootstrap_request_contract =
                    build_gemini_canvas_direct_http_page_harvest_bootstrap_request_contract(
                        &selected_url,
                        &bootstrap_candidates,
                        session_target_url,
                        session.cookie_header.len(),
                    );
                (body, bootstrap_request_contract, selected_url)
            } else {
                return Err(build_gemini_canvas_direct_http_page_harvest_failure_error(
                    last_error,
                    &bootstrap_candidates,
                    session_target_url,
                    &failures,
                ));
            }
        };

        let fallback_bootstrap =
            gemini_web::bootstrap_from_payload_cache(payload.extra_body.as_ref());
        let mut bootstrap = gemini_web::parse_bootstrap_from_app_html(
            &bootstrap_body,
            payload
                .extra_body
                .as_ref()
                .and_then(|extra| extra.get("language").and_then(Value::as_str)),
        )
        .or_else(|primary_error| fallback_bootstrap.clone().ok_or(primary_error))?;
        bootstrap =
            gemini_web::merge_bootstrap_from_fallback(bootstrap, fallback_bootstrap.as_ref());
        if is_image_edit_request {
            if let Some(locale) = gemini_canvas::harvest_image_edit_template_locale(&storage_state)
            {
                bootstrap.language = locale;
            }
        }
        let page_path =
            resolve_gemini_canvas_direct_http_bootstrap_page_path(&bootstrap_page_url, &bootstrap);
        let text_preflight_source_path =
            resolve_gemini_canvas_direct_http_preflight_source_path(payload);

        let referer = resolve_gemini_canvas_direct_http_referer(
            &effective_base_url,
            &page_path,
            &text_preflight_source_path,
            is_text_mode,
            is_image_mode,
        );
        let harvested_stream_generate_model_header =
            gemini_canvas::build_stream_generate_model_header_from_storage_state(
                &storage_state,
                mode_index,
                is_image_edit_request,
            );
        let model_header = resolve_gemini_canvas_direct_http_stream_generate_model_header(
            harvested_stream_generate_model_header.as_deref(),
            is_text_mode,
            is_image_mode,
        );

        if is_text_mode {
            self.execute_gemini_canvas_page_init_preflight_sequence(
                payload,
                model,
                &bootstrap,
                &text_preflight_source_path,
                &session,
                timeout,
                false,
            )
            .await?;
            self.execute_gemini_canvas_text_mode_selection_preflight(
                payload,
                model,
                &bootstrap,
                &text_preflight_source_path,
                &session,
                &origin,
                &authorization,
                timeout,
            )
            .await?;
            self.execute_gemini_canvas_text_bootstrap_preflight(
                payload,
                model,
                &bootstrap,
                &text_preflight_source_path,
                &session,
                &origin,
                &authorization,
                timeout,
            )
            .await?;
            self.execute_gemini_canvas_text_state_preflight(
                payload,
                model,
                &bootstrap,
                &text_preflight_source_path,
                &session,
                &origin,
                &authorization,
                timeout,
            )
            .await?;
            for (state_len, tail_index, tail_value, marker) in
                gemini_canvas_direct_http_text_state_variant_preflight_specs()
            {
                if let Err(error) = self
                    .execute_gemini_canvas_text_state_variant_preflight(
                        payload,
                        model,
                        &bootstrap,
                        &text_preflight_source_path,
                        &session,
                        state_len,
                        tail_index,
                        tail_value,
                        marker,
                        timeout,
                    )
                    .await
                {
                    debug!(
                        provider = "gemini_canvas_compatible",
                        error = %summarize_gateway_error(&error),
                        marker,
                        state_len,
                        tail_index,
                        "gemini canvas text optional page-state update failed; continuing with StreamGenerate"
                    );
                }
            }
        } else {
            let media_preflight_source_path = text_preflight_source_path.as_str();
            if is_image_mode {
                let image_preflight_result = if is_image_edit_request {
                    self.execute_gemini_canvas_media_capture_parity_preflight_sequence(
                        payload,
                        model,
                        &bootstrap,
                        media_preflight_source_path,
                        &mut session,
                        mode_index,
                        batchexecute_header_id.as_deref(),
                        is_image_edit_request,
                        timeout,
                    )
                    .await
                    .map(|_| ())
                } else {
                    self.execute_gemini_canvas_image_capture_parity_preflight_sequence(
                        payload,
                        model,
                        &bootstrap,
                        media_preflight_source_path,
                        &session,
                        batchexecute_header_id.as_deref(),
                        timeout,
                    )
                    .await
                };
                if let Err(error) = image_preflight_result {
                    debug!(
                        provider = "gemini_canvas_compatible",
                        error = %summarize_gateway_error(&error),
                        image_edit = is_image_edit_request,
                        source_path = media_preflight_source_path,
                        "gemini canvas image parity preflight failed; falling back to legacy media preflight chain"
                    );
                    self.execute_gemini_canvas_media_legacy_preflight_sequence(
                        payload,
                        model,
                        &bootstrap,
                        media_preflight_source_path,
                        &session,
                        mode_index,
                        batchexecute_header_id.as_deref(),
                        timeout,
                    )
                    .await?;
                }
            } else {
                if let Err(error) = self
                    .execute_gemini_canvas_media_capture_parity_preflight_sequence(
                        payload,
                        model,
                        &bootstrap,
                        media_preflight_source_path,
                        &mut session,
                        mode_index,
                        batchexecute_header_id.as_deref(),
                        false,
                        timeout,
                    )
                    .await
                {
                    debug!(
                        provider = "gemini_canvas_compatible",
                        error = %summarize_gateway_error(&error),
                        mode_index,
                        source_path = media_preflight_source_path,
                        "gemini canvas media parity preflight failed; falling back to legacy media preflight chain"
                    );
                    self.execute_gemini_canvas_media_legacy_preflight_sequence(
                        payload,
                        model,
                        &bootstrap,
                        media_preflight_source_path,
                        &session,
                        mode_index,
                        batchexecute_header_id.as_deref(),
                        timeout,
                    )
                    .await?;
                }
            }
        }

        let image_edit_sidecar_key = if is_image_edit_request {
            Some(
                gemini_canvas::image_edit_stream_generate_template_object_key(
                    &runtime.runtime_state_object_key,
                ),
            )
        } else {
            None
        };
        let image_edit_remote_sidecar = if let Some(sidecar_key) = image_edit_sidecar_key.as_deref()
        {
            object_storage.read_json(sidecar_key).await.ok()
        } else {
            None
        };
        let image_edit_local_sidecar = if let Some(sidecar_key) = image_edit_sidecar_key.as_deref()
        {
            read_gemini_canvas_runtime_mirror_json(sidecar_key)
        } else {
            None
        };
        let image_edit_template_source_origin = if is_image_edit_request
            && image_edit_local_sidecar
                .as_ref()
                .map(|value| {
                    gemini_canvas_sidecar_has_any_key(
                        value,
                        &["imageEditStreamGenerateTemplate", "imageEditStreamTemplate"],
                    )
                })
                .unwrap_or(false)
        {
            Some("local_sidecar")
        } else if is_image_edit_request
            && image_edit_remote_sidecar
                .as_ref()
                .map(|value| {
                    gemini_canvas_sidecar_has_any_key(
                        value,
                        &["imageEditStreamGenerateTemplate", "imageEditStreamTemplate"],
                    )
                })
                .unwrap_or(false)
        {
            Some("remote_sidecar")
        } else if is_image_edit_request
            && gemini_canvas_sidecar_has_any_key(
                &storage_state,
                &["imageEditStreamGenerateTemplate", "imageEditStreamTemplate"],
            )
        {
            Some("storage_state")
        } else {
            None
        };
        let image_edit_template_source = match image_edit_template_source_origin {
            Some("local_sidecar") => image_edit_local_sidecar.as_ref(),
            Some("remote_sidecar") => image_edit_remote_sidecar.as_ref(),
            Some("storage_state") => Some(&storage_state),
            _ => None,
        };

        let image_edit_uploaded_refs = if is_image_edit_request {
            Some(
                self.upload_gemini_canvas_image_edit_inputs(
                    payload,
                    &session,
                    &bootstrap,
                    image_edit_uploads.unwrap_or(&[]),
                    timeout,
                )
                .await?,
            )
        } else {
            None
        };
        let image_edit_seed = if is_image_edit_request {
            image_edit_template_source
                .and_then(gemini_canvas::harvest_image_edit_stream_generate_seed)
        } else {
            None
        };
        // Image-edit StreamGenerate requests appear to require a fresh request UUID on every send.
        // Reusing the harvested template UUID keeps the request pinned to the captured page-owned state
        // and prevents the replay lane from advancing into a new live edit settlement.
        let request_uuid = gemini_canvas::new_stream_generate_request_uuid();
        let stream_generate_header_id = if is_image_edit_request {
            Some(gemini_canvas::new_batchexecute_header_id())
        } else {
            None
        };
        let stream_generate_request_hex = if is_image_edit_request {
            Some(uuid::Uuid::new_v4().simple().to_string())
        } else {
            None
        };
        let mut replay_template = if is_image_edit_request && allow_replay_template {
            let uploaded_refs = image_edit_uploaded_refs.as_deref().unwrap_or(&[]);
            let initial_rebuild = image_edit_template_source.and_then(|value| {
                match gemini_canvas::build_image_edit_stream_generate_request_from_template(
                    value,
                    prompt,
                    &request_uuid,
                    uploaded_refs,
                ) {
                    Ok(template) => template,
                    Err(error) => {
                        debug!(
                            provider,
                            error = %summarize_gateway_error(&error),
                            template_source = image_edit_template_source_origin.unwrap_or("<none>"),
                            "gemini canvas image-edit preferred replay template source was invalid after upload; falling back to legacy heavy builder"
                        );
                        None
                    }
                }
            });
            match initial_rebuild {
                Some(template) => Some(template),
                None => {
                    let sidecar_has_template_key = image_edit_template_source_origin.is_some();
                    if initial_rebuild.is_none() {
                        append_gemini_canvas_image_edit_debug_json(
                            "gemini-canvas-image-edit-request-debug-template-miss.json",
                            &build_gemini_canvas_image_edit_template_miss_debug_snapshot(
                                &runtime.runtime_state_object_key,
                                image_edit_sidecar_key.as_deref(),
                                storage_state
                                    .get("imageEditStreamGenerateTemplate")
                                    .is_some(),
                                storage_state.get("imageEditStreamTemplate").is_some(),
                                image_edit_sidecar_key
                                    .as_ref()
                                    .and_then(|key| gemini_canvas_runtime_mirror_json_path(key))
                                    .map(|path| path.exists())
                                    .unwrap_or(false),
                                image_edit_template_source_origin,
                                sidecar_has_template_key,
                            ),
                        );
                    }
                    None
                }
            }
        } else {
            replay_template
        };
        if is_image_edit_request && replay_template.is_none() {
            let image_edit_sidecar_has_template_key = image_edit_remote_sidecar
                .as_ref()
                .or(image_edit_local_sidecar.as_ref())
                .map(|value| {
                    value.get("imageEditStreamGenerateTemplate").is_some()
                        || value.get("imageEditStreamTemplate").is_some()
                })
                .unwrap_or(false);
            if !image_edit_sidecar_has_template_key
                && !storage_state
                    .get("imageEditStreamGenerateTemplate")
                    .is_some()
                && !storage_state.get("imageEditStreamTemplate").is_some()
            {
                append_gemini_canvas_image_edit_debug_json(
                    "gemini-canvas-image-edit-request-debug-template-miss.json",
                    &build_gemini_canvas_image_edit_template_miss_debug_snapshot(
                        &runtime.runtime_state_object_key,
                        image_edit_sidecar_key.as_deref(),
                        storage_state
                            .get("imageEditStreamGenerateTemplate")
                            .is_some(),
                        storage_state.get("imageEditStreamTemplate").is_some(),
                        image_edit_sidecar_key
                            .as_ref()
                            .and_then(|key| gemini_canvas_runtime_mirror_json_path(key))
                            .map(|path| path.exists())
                            .unwrap_or(false),
                        image_edit_template_source_origin,
                        image_edit_sidecar_has_template_key,
                    ),
                );
            }
        }
        if let (true, Some(template), Some(header_id)) = (
            is_image_edit_request,
            replay_template.as_mut(),
            stream_generate_header_id.as_deref(),
        ) {
            let _ = gemini_canvas::refresh_stream_generate_template_model_header_id(
                template, header_id,
            );
        }
        if let (true, Some(template), Some(request_hex)) = (
            is_image_edit_request,
            replay_template.as_mut(),
            stream_generate_request_hex.as_deref(),
        ) {
            let _ =
                gemini_canvas::refresh_stream_generate_template_request_hex(template, request_hex);
        }
        if is_image_edit_request {
            if let Some(template) = replay_template.as_ref() {
                let uploaded_refs = build_gemini_canvas_image_edit_uploaded_refs_debug_snapshot(
                    image_edit_uploaded_refs.as_deref().unwrap_or(&[]),
                );
                let seed_snapshot =
                    build_gemini_canvas_image_edit_seed_debug_snapshot(image_edit_seed.as_ref());
                append_gemini_canvas_image_edit_request_debug_snapshot(
                    "gemini-canvas-image-edit-request-debug-template-pre-refresh.json",
                    "template-pre-refresh",
                    &template.url,
                    &template.query,
                    &template.form,
                    gemini_canvas_debug_headers_snapshot_from_hash_map(&template.headers),
                    build_gemini_canvas_image_edit_template_pre_refresh_debug_extra(
                        &request_uuid,
                        &runtime.runtime_state_object_key,
                        &gemini_canvas::image_edit_stream_generate_template_object_key(
                            &runtime.runtime_state_object_key,
                        ),
                        image_edit_template_source_origin,
                        gemini_canvas_sidecar_has_any_key(
                            &storage_state,
                            &["imageEditStreamGenerateTemplate", "imageEditStreamTemplate"],
                        ),
                        image_edit_remote_sidecar
                            .as_ref()
                            .map(|value| {
                                gemini_canvas_sidecar_has_any_key(
                                    value,
                                    &["imageEditStreamGenerateTemplate", "imageEditStreamTemplate"],
                                )
                            })
                            .unwrap_or(false),
                        image_edit_local_sidecar
                            .as_ref()
                            .map(|value| {
                                gemini_canvas_sidecar_has_any_key(
                                    value,
                                    &["imageEditStreamGenerateTemplate", "imageEditStreamTemplate"],
                                )
                            })
                            .unwrap_or(false),
                        &uploaded_refs,
                        seed_snapshot,
                    ),
                );
            }
        }
        let request = if let Some(uploaded_refs) = image_edit_uploaded_refs.as_deref() {
            gemini_canvas::build_stream_generate_heavy_request_with_uploaded_files_seeded(
                prompt,
                &bootstrap,
                &request_uuid,
                mode_index,
                uploaded_refs,
                image_edit_seed.as_ref(),
            )?
        } else {
            gemini_canvas::build_stream_generate_heavy_request(
                prompt,
                &bootstrap,
                &request_uuid,
                mode_index,
            )?
        };

        if is_image_mode {
            if let Some(template) = replay_template.as_ref() {
                let refreshed_template =
                    gemini_canvas::refresh_stream_generate_template_with_bootstrap(
                        template, &bootstrap, true,
                    )?;
                let mut headers = HeaderMap::new();
                apply_gemini_canvas_replay_template_headers(
                    &mut headers,
                    &refreshed_template.headers,
                    &session,
                );
                if is_image_edit_request {
                    append_gemini_canvas_image_edit_request_debug_snapshot(
                        "gemini-canvas-image-edit-request-debug-template-post-refresh.json",
                        "template-post-refresh",
                        &refreshed_template.url,
                        &refreshed_template.query,
                        &refreshed_template.form,
                        gemini_canvas_debug_headers_snapshot_from_header_map(&headers),
                        build_gemini_canvas_image_edit_template_post_refresh_debug_extra(
                            &request_uuid,
                            &runtime.runtime_state_object_key,
                            &gemini_canvas::image_edit_stream_generate_template_object_key(
                                &runtime.runtime_state_object_key,
                            ),
                            bootstrap.build_label.as_deref(),
                            bootstrap.session_id.as_deref(),
                            Some(bootstrap.language.as_str()),
                        ),
                    );
                }
                debug!(
                    provider,
                    url = %refreshed_template.url,
                    "sending gemini canvas pure HTTP image StreamGenerate request via refreshed replay template"
                );
                let response = self
                    .http
                    .request(Method::POST, &refreshed_template.url)
                    .headers(headers)
                    .query(&refreshed_template.query)
                    .timeout(image_stream_timeout)
                    .body(refreshed_template.raw_post_data.clone())
                    .send()
                    .await
                    .map_err(|error| classify_network_error(&error, Some(provider)))?;
                let status = response.status().as_u16();
                let final_url = response.url().to_string();
                let location = response
                    .headers()
                    .get(rquest::header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string);
                let content_type = response
                    .headers()
                    .get(rquest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string);
                let body_text = response
                    .text()
                    .await
                    .map_err(|error| classify_network_error(&error, Some(provider)))?;
                if is_image_edit_request {
                    append_gemini_canvas_image_edit_stream_response_debug_snapshot(
                        "gemini-canvas-image-edit-stream-response-template-after-refresh",
                        "template-after-refresh-response",
                        &final_url,
                        status,
                        location.as_deref(),
                        content_type.as_deref(),
                        &body_text,
                        build_gemini_canvas_image_edit_stream_response_template_after_refresh_debug_extra(
                            &runtime.runtime_state_object_key,
                            bootstrap.build_label.as_deref(),
                            bootstrap.session_id.as_deref(),
                            Some(bootstrap.language.as_str()),
                        ),
                    );
                }
                if (200..300).contains(&status)
                    && !gemini_web::response_indicates_browser_challenge(
                        status,
                        content_type.as_deref(),
                        &body_text,
                    )
                    && !gemini_web::response_indicates_session_invalid(
                        status,
                        content_type.as_deref(),
                        &body_text,
                    )
                    && !gemini_canvas::response_indicates_image_generation_unavailable(
                        status,
                        content_type.as_deref(),
                        &body_text,
                    )
                {
                    if is_image_edit_request {
                        if let Some(context) = image_edit_followup_context.as_deref_mut() {
                            if context.signaler_response_id.is_none() {
                                if let Some(response_id) =
                                    gemini_canvas::extract_stream_generate_response_id(&body_text)
                                        .ok()
                                {
                                    context.signaler_response_id = Some(response_id);
                                }
                            }
                        }
                        debug!(
                            provider,
                            status,
                            "gemini canvas pure HTTP image-edit replay template returned a non-challenge StreamGenerate body after bootstrap refresh; deferring asset resolution to follow-up extraction"
                        );
                        return Ok(body_text);
                    }
                    return Ok(body_text);
                }
                debug!(
                    provider,
                    status,
                    content_type = content_type.as_deref().unwrap_or("<none>"),
                    "gemini canvas pure HTTP image replay template failed after bootstrap refresh; falling back to heavy builder"
                );
            }
        }

        let mut headers = gemini_web_reverse_modular::build_headers(payload, None, model);
        apply_gemini_canvas_direct_http_stream_generate_headers(
            &mut headers,
            payload,
            &request_uuid,
            model_header,
            if is_text_mode { Some(&session) } else { None },
            is_text_mode || is_image_mode,
        );
        if is_text_mode {
            headers.remove(HeaderName::from_static("origin"));
        }
        if is_image_mode {
            apply_gemini_canvas_cookie_header(&mut headers, &session);
            insert_header_map_value(
                &mut headers,
                "x-same-domain",
                gemini_web::GEMINI_WEB_DEFAULT_SAME_DOMAIN_HEADER,
            );
            headers
                .entry(rquest::header::CONTENT_TYPE)
                .or_insert(HeaderValue::from_static(
                    "application/x-www-form-urlencoded;charset=UTF-8",
                ));
            headers.remove("authorization");
            headers.remove("x-origin");
            headers.remove("x-goog-authuser");
        } else if !is_text_mode {
            apply_gemini_canvas_signed_headers(
                &mut headers,
                &session,
                &origin,
                &referer,
                &authorization,
                true,
            );
        }
        let browser_runtime_state_object_key =
            gemini_canvas::browser_runtime_state_object_key_for_browser_operation(
                payload,
                if is_text_mode { "text" } else { "image" },
            )
            .unwrap_or_else(|| runtime.runtime_state_object_key.clone());
        let browser_cdp_url = gemini_canvas::browser_cdp_url(payload);
        let browser_cookie_header = gemini_canvas::browser_cookie_header(payload);
        let stream_request_contract = build_gemini_canvas_image_edit_stream_request_contract(
            &stream_url,
            &bootstrap_url,
            &page_path,
            mode_index,
            is_text_mode,
            is_image_mode,
            &headers,
        );
        if is_image_edit_request
            && !allow_replay_template
            && should_attempt_gemini_canvas_browser_backed_image_edit_retry(
                payload,
                EndpointKind::ImagesEdits,
            )
            && !browser_runtime_state_object_key.trim().is_empty()
        {
            match self.ensure_gemini_canvas_browser_pool(provider).await {
                Ok(browser_pool_base_url) => {
                    debug!(
                        provider,
                        browser_runtime_state_object_key = %browser_runtime_state_object_key,
                        has_browser_cdp = browser_cdp_url.is_some(),
                        "retrying gemini canvas image-edit heavy StreamGenerate through browser-backed fetch before pure HTTP fallback"
                    );
                    match self
                        .execute_gemini_canvas_browser_fetch_form_request(
                            provider,
                            &browser_pool_base_url,
                            origin.as_str(),
                            runtime.share_id.as_str(),
                            browser_runtime_state_object_key.as_str(),
                            browser_cdp_url.as_deref(),
                            browser_cookie_header.as_deref(),
                            &stream_url,
                            &request.query,
                            &headers,
                            &request.form,
                            image_stream_timeout,
                        )
                        .await
                    {
                        Ok(invocation) => {
                            let browser_status = invocation.status;
                            let browser_content_type = invocation.content_type.clone();
                            let browser_final_url = invocation
                                .final_url
                                .clone()
                                .unwrap_or_else(|| stream_url.clone());
                            if let Some(body_text) = invocation.body_text {
                                if is_image_edit_request {
                                    append_gemini_canvas_image_edit_stream_response_debug_snapshot(
                                        "gemini-canvas-image-edit-stream-response-heavy-browser-fetch",
                                        "heavy-builder-browser-fetch-response",
                                        &browser_final_url,
                                        browser_status,
                                        None,
                                        browser_content_type.as_deref(),
                                        &body_text,
                                        build_gemini_canvas_image_edit_stream_response_heavy_debug_extra(
                                            &browser_runtime_state_object_key,
                                            &request_uuid,
                                            &stream_request_contract,
                                        ),
                                    );
                                }
                                let browser_failed = !(200..300).contains(&browser_status)
                                    || gemini_web::response_indicates_browser_challenge(
                                        browser_status,
                                        browser_content_type.as_deref(),
                                        &body_text,
                                    )
                                    || gemini_web::response_indicates_session_invalid(
                                        browser_status,
                                        browser_content_type.as_deref(),
                                        &body_text,
                                    );
                                if !browser_failed {
                                    return Ok(body_text);
                                }
                                debug!(
                                    provider,
                                    status = browser_status,
                                    content_type = browser_content_type.as_deref().unwrap_or("<none>"),
                                    "browser-backed image-edit heavy StreamGenerate did not yield a usable non-challenge body; falling back to pure HTTP heavy send"
                                );
                            } else {
                                debug!(
                                    provider,
                                    status = browser_status,
                                    "browser-backed image-edit heavy StreamGenerate completed without a text body; falling back to pure HTTP heavy send"
                                );
                            }
                        }
                        Err(error) => {
                            debug!(
                                provider,
                                error = %summarize_gateway_error(&error),
                                "browser-backed image-edit heavy StreamGenerate fetch failed; falling back to pure HTTP heavy send"
                            );
                        }
                    }
                }
                Err(error) => {
                    debug!(
                        provider,
                        error = %summarize_gateway_error(&error),
                        "failed to prepare Gemini Canvas browser pool for image-edit heavy fetch; continuing with pure HTTP heavy send"
                    );
                }
            }
        }
        if is_image_edit_request {
            append_gemini_canvas_image_edit_request_debug_snapshot(
                "gemini-canvas-image-edit-request-debug-heavy.json",
                "heavy-builder",
                &stream_url,
                &request.query,
                &request.form,
                gemini_canvas_debug_headers_snapshot_from_header_map(&headers),
                build_gemini_canvas_image_edit_heavy_builder_debug_extra(
                    &request_uuid,
                    &runtime.runtime_state_object_key,
                    &gemini_canvas::image_edit_stream_generate_template_object_key(
                        &runtime.runtime_state_object_key,
                    ),
                    &stream_request_contract,
                    bootstrap.build_label.as_deref(),
                    bootstrap.session_id.as_deref(),
                    Some(bootstrap.language.as_str()),
                ),
            );
        }

        debug!(
            provider,
            url = %stream_url,
            page_path = %page_path,
            mode_index,
            "sending gemini canvas pure HTTP StreamGenerate request"
        );
        let mut request_form = request.form.clone();
        let mut xsrf_retry_token: Option<String> = None;

        loop {
            let response = self
                .http
                .request(Method::POST, &stream_url)
                .headers(headers.clone())
                .query(&request.query)
                .timeout(image_stream_timeout)
                .form(&request_form)
                .send()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            let final_url = response.url().to_string();
            let status = response.status().as_u16();
            let location = response
                .headers()
                .get(rquest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let content_type = response
                .headers()
                .get(rquest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let (stream_operation, allow_early_locator) =
                gemini_canvas_stream_collection_policy(mode_index, is_image_edit_request);
            let body_text = self
                .collect_gemini_canvas_stream_generate_body(
                    response,
                    provider,
                    stream_operation,
                    allow_early_locator,
                )
                .await?;
            if is_image_edit_request {
                append_gemini_canvas_image_edit_stream_response_debug_snapshot(
                    "gemini-canvas-image-edit-stream-response-heavy",
                    "heavy-builder-response",
                    &final_url,
                    status,
                    location.as_deref(),
                    content_type.as_deref(),
                    &body_text,
                    build_gemini_canvas_image_edit_stream_response_heavy_debug_extra(
                        &runtime.runtime_state_object_key,
                        &request_uuid,
                        &stream_request_contract,
                    ),
                );
            }
            let stream_response_meta = build_gemini_canvas_image_edit_stream_response_meta(
                &final_url,
                location.as_deref(),
                content_type.as_deref(),
                &body_text,
            );
            let failed = !(200..300).contains(&status)
                || gemini_web::response_indicates_browser_challenge(
                    status,
                    content_type.as_deref(),
                    &body_text,
                )
                || gemini_web::response_indicates_session_invalid(
                    status,
                    content_type.as_deref(),
                    &body_text,
                );
            if failed {
                if status == 400 {
                    if let Some(token) = maybe_retry_gemini_canvas_form_xsrf_token(
                        &mut request_form,
                        &body_text,
                        &mut xsrf_retry_token,
                    ) {
                        debug!(
                            provider,
                            xsrf_token_preview = %truncate_response_preview(&token, 24),
                            "retrying gemini canvas StreamGenerate with xsrf token extracted from upstream error"
                        );
                        continue;
                    }
                }
                return Err(append_gateway_error_summary(
                    append_gateway_error_summary(
                        classify_gemini_canvas_pure_http_error(
                            status,
                            content_type.as_deref(),
                            &body_text,
                        ),
                        "stream_request_contract",
                        Some(&stream_request_contract),
                    ),
                    "stream_response_meta",
                    Some(&stream_response_meta),
                ));
            }

            return Ok(body_text);
        }
    }

    async fn prepare_gemini_canvas_direct_http_text_context(
        &self,
        payload: &ProviderAccountPayload,
        _model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        timeout: Duration,
    ) -> Result<
        (
            gemini_canvas::GeminiCanvasPureHttpSession,
            gemini_web::GeminiWebBootstrap,
            Option<String>,
        ),
        GatewayError,
    > {
        let provider = "gemini_canvas_compatible";
        let configured_base_url = payload.base_url.trim_end_matches('/');
        let effective_base_url = if configured_base_url.is_empty() {
            gemini_canvas_http_origin(payload)
        } else {
            configured_base_url.to_string()
        };
        let bootstrap_url = if payload.adapter == "gemini_canvas_program_web_reverse_compatible" {
            gemini_canvas_program_web_reverse_modular::gemini_canvas_program_payload_page_url(
                payload,
                &effective_base_url,
            )
            .unwrap_or_else(|| {
                format!(
                    "{}{}",
                    effective_base_url,
                    gemini_web::GEMINI_WEB_DEFAULT_APP_PATH
                )
            })
        } else {
            format!(
                "{}{}",
                effective_base_url,
                gemini_web::GEMINI_WEB_DEFAULT_APP_PATH
            )
        };
        let storage_state = gateway_object_storage()?
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        let batchexecute_header_id =
            gemini_canvas::harvest_text_batchexecute_header_id(&storage_state);
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
        let explicit_cookie_header = payload
            .extra_body
            .as_ref()
            .and_then(|extra| {
                extra
                    .get("canvasProgramInvokeContract")
                    .and_then(Value::as_object)
                    .and_then(|contract| contract.get("cookieHeader"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| {
                payload
                    .extra_body
                    .as_ref()
                    .and_then(|extra| extra.get("cookieHeader"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| {
                payload
                    .extra_body
                    .as_ref()
                    .and_then(|extra| {
                        extra
                            .get("canvasProgramInvokeContract")
                            .and_then(Value::as_object)
                            .and_then(|contract| contract.get("cookie_header"))
                            .and_then(Value::as_str)
                    })
                    .map(str::to_string)
            })
            .or_else(|| {
                payload
                    .extra_body
                    .as_ref()
                    .and_then(|extra| extra.get("cookie_header"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| read_json_string(&storage_state, "cookieHeader"));
        let session = if let Some(cookie_header) = explicit_cookie_header
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            gemini_canvas::pure_http_session_from_cookie_header(cookie_header, &auth_user)?
        } else {
            gemini_canvas::storage_state_to_pure_http_session(
                &storage_state,
                &bootstrap_url,
                &effective_base_url,
                &auth_user,
            )?
        };

        let mut headers = HeaderMap::new();
        apply_gemini_canvas_navigation_headers(&mut headers);
        apply_gemini_canvas_cookie_header(&mut headers, &session);
        insert_header_map_value(
            &mut headers,
            "accept-language",
            &gemini_canvas::locale_from_payload(payload),
        );

        let bootstrap_response = self
            .http
            .request(Method::GET, &bootstrap_url)
            .headers(headers)
            .timeout(timeout.max(Duration::from_secs(30)))
            .send()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        let bootstrap_status = bootstrap_response.status().as_u16();
        let bootstrap_content_type = bootstrap_response
            .headers()
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let bootstrap_body = bootstrap_response
            .text()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        if !(200..300).contains(&bootstrap_status)
            || gemini_web::response_indicates_browser_challenge(
                bootstrap_status,
                bootstrap_content_type.as_deref(),
                &bootstrap_body,
            )
            || gemini_web::response_indicates_session_invalid(
                bootstrap_status,
                bootstrap_content_type.as_deref(),
                &bootstrap_body,
            )
        {
            return Err(classify_gemini_canvas_pure_http_error(
                bootstrap_status,
                bootstrap_content_type.as_deref(),
                &bootstrap_body,
            ));
        }

        let fallback_bootstrap =
            gemini_web::bootstrap_from_payload_cache(payload.extra_body.as_ref());
        let bootstrap = gemini_web::parse_bootstrap_from_app_html(
            &bootstrap_body,
            payload
                .extra_body
                .as_ref()
                .and_then(|extra| extra.get("language").and_then(Value::as_str)),
        )
        .or_else(|primary_error| fallback_bootstrap.clone().ok_or(primary_error))?;
        let bootstrap =
            gemini_web::merge_bootstrap_from_fallback(bootstrap, fallback_bootstrap.as_ref());
        Ok((session, bootstrap, batchexecute_header_id))
    }

    async fn execute_gemini_canvas_page_init_preflight_sequence(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        timeout: Duration,
        continue_on_error: bool,
    ) -> Result<(), GatewayError> {
        for (rpcid, rpc_payload, model_header) in [
            (
                "otAQ7b",
                json!([]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
            ),
            (
                "sJBwce",
                json!([[1, 2]]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
            ),
            (
                "DYBcR",
                json!([bootstrap.language]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
            ),
            (
                "cYRIkd",
                json!([bootstrap.language]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
            ),
            (
                "GPRiHf",
                json!([]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
            ),
            (
                "maGuAc",
                json!([0]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
            ),
            (
                "maGuAc",
                json!([1]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
            ),
            (
                "Te6DCf",
                json!([[bootstrap.language], [1]]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
            ),
            (
                "mhs1xe",
                json!([[1, 3]]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER,
            ),
            (
                "K4WWud",
                json!([[0], [bootstrap.language]]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER,
            ),
            (
                "ku4Jyf",
                json!([
                    bootstrap.language,
                    null,
                    null,
                    null,
                    4,
                    null,
                    null,
                    [2, 3, 7, 17],
                    null,
                    []
                ]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER,
            ),
            (
                "ozz5Z",
                json!([[
                    [[null, "1", 447]],
                    [[null, "1", 448]],
                    [[null, "1", 702]],
                    [[null, "1", 961]],
                    [[null, "1", 960]],
                    [[null, "1", 1062]],
                    [[null, "1", 1240]],
                    [[null, "1", 1237]],
                    [[null, "1", 1238]],
                    [[null, "1", 1239]],
                    [[null, "1", 1241]]
                ]]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER,
            ),
            (
                "CNgdBe",
                json!([1, [bootstrap.language], 0]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER,
            ),
        ] {
            let result = self
                .execute_gemini_canvas_text_generic_preflight(
                    payload,
                    model,
                    bootstrap,
                    source_path,
                    session,
                    rpcid,
                    rpc_payload,
                    model_header,
                    timeout,
                )
                .await;
            if let Err(error) = result {
                if continue_on_error {
                    debug!(
                        provider = "gemini_canvas_compatible",
                        error = %summarize_gateway_error(&error),
                        rpcid,
                        source_path,
                        "gemini canvas page-init preflight failed; continuing"
                    );
                } else {
                    return Err(error);
                }
            }
        }

        let (state_len, tail_index, tail_value, marker) =
            gemini_canvas_direct_http_text_fast_version_preflight_spec();
        let result = self
            .execute_gemini_canvas_text_state_variant_preflight(
                payload,
                model,
                bootstrap,
                source_path,
                session,
                state_len,
                tail_index,
                tail_value,
                marker,
                timeout,
            )
            .await;
        if let Err(error) = result {
            if continue_on_error {
                debug!(
                    provider = "gemini_canvas_compatible",
                    error = %summarize_gateway_error(&error),
                    marker,
                    source_path,
                    "gemini canvas page-init state variant failed; continuing"
                );
            } else {
                return Err(error);
            }
        }

        Ok(())
    }

    async fn execute_gemini_canvas_mode_selection_preflight(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        selected_id: &str,
        batchexecute_header_id: Option<&str>,
        timeout: Duration,
    ) -> Result<(), GatewayError> {
        let request = gemini_canvas::build_mode_selection_preflight_request(
            bootstrap,
            source_path,
            selected_id,
        )?;
        let model_header =
            gemini_canvas::build_text_batchexecute_model_header(None, batchexecute_header_id);

        if let Err(mut classified) = self
            .send_gemini_canvas_text_batchexecute_request(
                payload,
                model,
                &request,
                session,
                &model_header,
                timeout,
            )
            .await
        {
            let rpcids = request
                .query
                .iter()
                .find(|(key, _)| key == "rpcids")
                .map(|(_, value)| value.as_str())
                .unwrap_or("<unknown>");
            let source_path = request
                .query
                .iter()
                .find(|(key, _)| key == "source-path")
                .map(|(_, value)| value.as_str())
                .unwrap_or("<unknown>");
            let reqid = request
                .query
                .iter()
                .find(|(key, _)| key == "_reqid")
                .map(|(_, value)| value.as_str())
                .unwrap_or("<unknown>");
            classified.message = format!(
                "Gemini Canvas batchexecute failed. rpcids={rpcids}; source_path={source_path}; reqid={reqid}; {}",
                classified.message
            );
            return Err(classified);
        }

        Ok(())
    }

    async fn execute_gemini_canvas_text_mode_selection_preflight(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        _origin: &str,
        _authorization: &str,
        timeout: Duration,
    ) -> Result<(), GatewayError> {
        self.execute_gemini_canvas_mode_selection_preflight(
            payload,
            model,
            bootstrap,
            source_path,
            session,
            gemini_canvas::GEMINI_CANVAS_TEXT_LAST_SELECTED_MODE_ID,
            None,
            timeout,
        )
        .await
    }

    async fn execute_gemini_canvas_media_operation_selection_preflight(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        mode_index: i64,
        batchexecute_header_id: Option<&str>,
        timeout: Duration,
    ) -> Result<(), GatewayError> {
        let request = gemini_canvas::build_media_operation_selection_preflight_request(
            bootstrap,
            source_path,
            mode_index,
        )?;
        let model_header =
            gemini_canvas::build_text_batchexecute_model_header(None, batchexecute_header_id);
        self.send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &request,
            session,
            &model_header,
            timeout,
        )
        .await
        .map(|_| ())
    }

    async fn execute_gemini_canvas_media_state_preflight_sequence(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        mode_index: i64,
        batchexecute_header_id: Option<&str>,
        timeout: Duration,
    ) -> Result<(), GatewayError> {
        let language = bootstrap.language.clone();
        let primary_mode_index = gemini_canvas::media_primary_ku4jyf_mode_index(mode_index);
        let xhau0b_payload =
            if mode_index == gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_MUSIC_MODE_INDEX {
                json!([4, [3], 3])
            } else {
                json!([4, [2], 3])
            };
        let model_header =
            gemini_canvas::build_text_batchexecute_model_header(None, batchexecute_header_id);
        let preflights = vec![
            (
                "o30O0e",
                json!([
                    ["me"],
                    [
                        [["person.photo", "person.name", "person.email"]],
                        null,
                        [1, 7]
                    ]
                ]),
            ),
            ("K4WWud", json!([[1], [language.clone()]])),
            ("CNgdBe", json!([1, [language.clone()], 0])),
            (
                "ku4Jyf",
                json!([
                    language.clone(),
                    null,
                    null,
                    null,
                    primary_mode_index,
                    null,
                    null,
                    [32],
                    null,
                    []
                ]),
            ),
            ("CNgdBe", json!([2, [language.clone()], 0, null, [2]])),
            ("ESY5D", json!([[["bard_activity_enabled"]]])),
            ("MaZiqc", json!([13, null, [1, null, 1]])),
            ("MaZiqc", json!([13, null, [0, null, 1]])),
            (
                "ku4Jyf",
                json!([
                    language,
                    null,
                    null,
                    null,
                    4,
                    null,
                    null,
                    [2, 4, 7, 17],
                    null,
                    []
                ]),
            ),
            ("XhaU0b", xhau0b_payload),
        ];

        for (rpcid, rpc_payload) in preflights {
            self.execute_gemini_canvas_text_generic_preflight(
                payload,
                model,
                bootstrap,
                source_path,
                session,
                rpcid,
                rpc_payload,
                &model_header,
                timeout,
            )
            .await?;
        }

        Ok(())
    }

    async fn execute_gemini_canvas_image_capture_parity_preflight_sequence(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        batchexecute_header_id: Option<&str>,
        timeout: Duration,
    ) -> Result<(), GatewayError> {
        let neutral_header = gemini_canvas::build_text_batchexecute_model_header_variant(
            None,
            batchexecute_header_id,
            false,
            false,
        );
        let empty_model_header = gemini_canvas::build_text_batchexecute_model_header_variant(
            None,
            batchexecute_header_id,
            false,
            true,
        );
        let flagged_neutral_header = gemini_canvas::build_text_batchexecute_model_header_variant(
            None,
            batchexecute_header_id,
            true,
            false,
        );
        let flagged_selected_header = gemini_canvas::build_text_batchexecute_model_header_variant(
            Some(gemini_canvas::GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID),
            batchexecute_header_id,
            true,
            false,
        );
        let language = bootstrap.language.clone();

        for (rpcid, rpc_payload, model_header) in [
            ("otAQ7b", json!([]), neutral_header.as_str()),
            ("sJBwce", json!([[1, 2]]), neutral_header.as_str()),
            ("DYBcR", json!([language.clone()]), neutral_header.as_str()),
            ("aPya6c", json!([]), empty_model_header.as_str()),
            ("cYRIkd", json!([language.clone()]), neutral_header.as_str()),
        ] {
            let request = gemini_canvas::build_text_batchexecute_request(
                rpcid,
                rpc_payload,
                bootstrap,
                source_path,
            )?;
            self.send_gemini_canvas_text_batchexecute_request(
                payload,
                model,
                &request,
                session,
                model_header,
                timeout,
            )
            .await?;
        }

        let early_side_nav = gemini_canvas::build_text_state_variant_preflight_request(
            bootstrap,
            source_path,
            41usize,
            40usize,
            Value::from(0),
            "side_nav_open_by_default",
            gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
        )?;
        self.send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &early_side_nav,
            session,
            &neutral_header,
            timeout,
        )
        .await?;

        let image_state_keys =
            gemini_canvas::build_image_state_keys_preflight_request(bootstrap, source_path)?;
        self.send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &image_state_keys,
            session,
            &neutral_header,
            timeout,
        )
        .await?;

        for (rpcid, rpc_payload) in [
            ("ESY5D", json!([[["bard_activity_enabled"]]])),
            ("MaZiqc", json!([13, null, [1, null, 1]])),
            ("MaZiqc", json!([13, null, [0, null, 1]])),
            ("GPRiHf", json!([])),
            ("maGuAc", json!([0])),
            ("maGuAc", json!([1])),
            (
                "qpEbW",
                json!([[
                    [
                        1,
                        gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX
                    ],
                    [
                        2,
                        gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX
                    ],
                    [
                        6,
                        gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX
                    ]
                ]]),
            ),
            ("mhs1xe", json!([[1, 3]])),
            (
                "o30O0e",
                json!([
                    ["me"],
                    [
                        [["person.photo", "person.name", "person.email"]],
                        null,
                        [1, 7]
                    ]
                ]),
            ),
            ("Te6DCf", json!([[language.clone()], [1]])),
            ("K4WWud", json!([[0], [language.clone()]])),
            ("CNgdBe", json!([1, [language.clone()], 0])),
            (
                "ozz5Z",
                json!([[
                    [[null, "1", 447]],
                    [[null, "1", 448]],
                    [[null, "1", 702]],
                    [[null, "1", 961]],
                    [[null, "1", 960]],
                    [[null, "1", 1062]],
                    [[null, "1", 1240]],
                    [[null, "1", 1237]],
                    [[null, "1", 1238]],
                    [[null, "1", 1239]],
                    [[null, "1", 1241]],
                ]]),
            ),
            ("CNgdBe", json!([2, [language.clone()], 0])),
        ] {
            let request = gemini_canvas::build_text_batchexecute_request(
                rpcid,
                rpc_payload,
                bootstrap,
                source_path,
            )?;
            self.send_gemini_canvas_text_batchexecute_request(
                payload,
                model,
                &request,
                session,
                &neutral_header,
                timeout,
            )
            .await?;
        }

        let selected_mode = gemini_canvas::build_mode_selection_preflight_request(
            bootstrap,
            source_path,
            gemini_canvas::GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID,
        )?;
        self.send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &selected_mode,
            session,
            &flagged_neutral_header,
            timeout,
        )
        .await?;

        let selected_bootstrap =
            gemini_canvas::build_text_bootstrap_preflight_request(bootstrap, source_path)?;
        self.send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &selected_bootstrap,
            session,
            &flagged_selected_header,
            timeout,
        )
        .await?;

        for request in [
            gemini_canvas::build_text_state_variant_preflight_request(
                bootstrap,
                source_path,
                41usize,
                40usize,
                Value::from(0),
                "side_nav_open_by_default",
                gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
            )?,
            gemini_canvas::build_text_state_variant_preflight_request(
                bootstrap,
                source_path,
                41usize,
                40usize,
                Value::from(0),
                "side_nav_open_by_default",
                gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
            )?,
            gemini_canvas::build_text_state_variant_preflight_request(
                bootstrap,
                source_path,
                87usize,
                86usize,
                Value::from(14),
                "popup_zs_visits_cooldown",
                gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
            )?,
        ] {
            self.send_gemini_canvas_text_batchexecute_request(
                payload,
                model,
                &request,
                session,
                &flagged_neutral_header,
                timeout,
            )
            .await?;
        }

        let ku4jyf_first = gemini_canvas::build_text_batchexecute_request(
            "ku4Jyf",
            json!([[
                language.clone(),
                null,
                null,
                null,
                gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
                null,
                null,
                [32],
                null,
                []
            ]]),
            bootstrap,
            source_path,
        )?;
        self.send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &ku4jyf_first,
            session,
            &flagged_neutral_header,
            timeout,
        )
        .await?;

        for request in [
            gemini_canvas::build_text_state_variant_preflight_request(
                bootstrap,
                source_path,
                94usize,
                93usize,
                Value::String("NULL".to_string()),
                "current_popup_id",
                gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
            )?,
            gemini_canvas::build_text_state_variant_preflight_request(
                bootstrap,
                source_path,
                87usize,
                86usize,
                Value::from(15),
                "popup_zs_visits_cooldown",
                gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
            )?,
            gemini_canvas::build_text_state_variant_preflight_request(
                bootstrap,
                source_path,
                192usize,
                191usize,
                json!([["image_generation_soft:1", "music_generation_soft:1"]]),
                "tool_menu_soft_badge_impression_counts",
                gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
            )?,
        ] {
            self.send_gemini_canvas_text_batchexecute_request(
                payload,
                model,
                &request,
                session,
                &flagged_neutral_header,
                timeout,
            )
            .await?;
        }

        for (rpcid, rpc_payload) in [
            ("CNgdBe", json!([2, [language], 0, null, [2]])),
            ("XhaU0b", json!([4, [2], 3])),
        ] {
            let request = gemini_canvas::build_text_batchexecute_request(
                rpcid,
                rpc_payload,
                bootstrap,
                source_path,
            )?;
            self.send_gemini_canvas_text_batchexecute_request(
                payload,
                model,
                &request,
                session,
                &flagged_neutral_header,
                timeout,
            )
            .await?;
        }

        Ok(())
    }

    async fn execute_gemini_canvas_media_capture_parity_preflight_sequence(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        mode_index: i64,
        batchexecute_header_id: Option<&str>,
        is_image_edit_request: bool,
        timeout: Duration,
    ) -> Result<GeminiCanvasMediaCaptureParityPreflightResult, GatewayError> {
        let neutral_header = gemini_canvas::build_text_batchexecute_model_header_variant(
            None,
            batchexecute_header_id,
            false,
            false,
        );
        let empty_model_header = gemini_canvas::build_text_batchexecute_model_header_variant(
            None,
            batchexecute_header_id,
            false,
            true,
        );
        let flagged_neutral_header = gemini_canvas::build_text_batchexecute_model_header_variant(
            None,
            batchexecute_header_id,
            true,
            false,
        );
        let flagged_selected_header = gemini_canvas::build_text_batchexecute_model_header_variant(
            Some(gemini_canvas::GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID),
            batchexecute_header_id,
            true,
            false,
        );
        let selected_header = gemini_canvas::build_text_batchexecute_model_header_variant(
            Some(gemini_canvas::GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID),
            batchexecute_header_id,
            false,
            false,
        );
        let language = bootstrap.language.clone();
        let selection_mode_index =
            gemini_canvas::media_operation_selection_preflight_mode_index(mode_index);
        let primary_mode_index = gemini_canvas::media_primary_ku4jyf_mode_index(mode_index);
        let tool_menu_soft_badge_impression_counts =
            if mode_index == gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_MUSIC_MODE_INDEX {
                json!([["image_generation_soft:4", "music_generation_soft:4"]])
            } else {
                json!([["image_generation_soft:1", "music_generation_soft:1"]])
            };
        let popup_zs_visits_cooldown =
            if mode_index == gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_MUSIC_MODE_INDEX {
                Value::from(4)
            } else {
                Value::from(15)
            };
        let xhau0b_payload =
            if mode_index == gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_MUSIC_MODE_INDEX {
                json!([4, [3], 3])
            } else {
                json!([4, [2], 3])
            };
        let is_image_mode =
            mode_index == gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX;
        let initial_header2 = "[]";
        let selected_header2 = gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_MODEL_HEADER_2;
        let mut maziqc_probe_body = None;
        let mut maziqc_full_body = None;
        let mut o30o0e_body = None;
        let mut k4wwud_body = None;

        if is_image_mode && is_image_edit_request {
            let ozz5z_payload = json!([[
                [[null, "1", 447]],
                [[null, "1", 448]],
                [[null, "1", 702]],
                [[null, "1", 961]],
                [[null, "1", 960]],
                [[null, "1", 1062]],
                [[null, "1", 1240]],
                [[null, "1", 1237]],
                [[null, "1", 1238]],
                [[null, "1", 1239]],
                [[null, "1", 1241]],
            ]]);
            let ku4jyf_payload = json!([[
                language.clone(),
                null,
                null,
                null,
                primary_mode_index,
                null,
                null,
                [32],
                null,
                []
            ]]);
            for (rpcid, rpc_payload, model_header) in [
                ("otAQ7b", json!([]), neutral_header.as_str()),
                ("GPRiHf", json!([]), neutral_header.as_str()),
                ("DYBcR", json!([language.clone()]), neutral_header.as_str()),
                (
                    "Te6DCf",
                    json!([[language.clone()], [1]]),
                    neutral_header.as_str(),
                ),
            ] {
                let request = gemini_canvas::build_text_batchexecute_request(
                    rpcid,
                    rpc_payload,
                    bootstrap,
                    source_path,
                )?;
                self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                    payload,
                    model,
                    &request,
                    session,
                    model_header,
                    None,
                    timeout,
                )
                .await?;
            }

            let media_state_keys =
                gemini_canvas::build_image_state_keys_preflight_request(bootstrap, source_path)?;
            self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                payload,
                model,
                &media_state_keys,
                session,
                &neutral_header,
                None,
                timeout,
            )
            .await?;

            for (rpcid, rpc_payload, model_header) in [
                ("aPya6c", json!([]), empty_model_header.as_str()),
                (
                    "ESY5D",
                    json!([[["bard_activity_enabled"]]]),
                    neutral_header.as_str(),
                ),
                (
                    "MaZiqc",
                    json!([13, null, [1, null, 1]]),
                    neutral_header.as_str(),
                ),
                ("sJBwce", json!([[1, 2]]), neutral_header.as_str()),
                ("maGuAc", json!([1]), neutral_header.as_str()),
                ("mhs1xe", json!([[1, 3]]), neutral_header.as_str()),
                (
                    "MaZiqc",
                    json!([13, null, [0, null, 1]]),
                    neutral_header.as_str(),
                ),
                ("ozz5Z", ozz5z_payload.clone(), neutral_header.as_str()),
                ("cYRIkd", json!([language.clone()]), neutral_header.as_str()),
                (
                    "qpEbW",
                    json!([[
                        [1, selection_mode_index],
                        [2, selection_mode_index],
                        [6, selection_mode_index]
                    ]]),
                    neutral_header.as_str(),
                ),
                (
                    "o30O0e",
                    json!([
                        ["me"],
                        [
                            [["person.photo", "person.name", "person.email"]],
                            null,
                            [1, 7]
                        ]
                    ]),
                    neutral_header.as_str(),
                ),
                (
                    "K4WWud",
                    json!([[1], [language.clone()]]),
                    neutral_header.as_str(),
                ),
            ] {
                let request = gemini_canvas::build_text_batchexecute_request(
                    rpcid,
                    rpc_payload,
                    bootstrap,
                    source_path,
                )?;
                let body = self
                    .send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                        payload,
                        model,
                        &request,
                        session,
                        model_header,
                        None,
                        timeout,
                    )
                    .await?;
                match rpcid {
                    "MaZiqc" if maziqc_probe_body.is_none() => {
                        maziqc_probe_body = Some(body);
                    }
                    "MaZiqc" if maziqc_full_body.is_none() => {
                        maziqc_full_body = Some(body);
                    }
                    "o30O0e" => {
                        o30o0e_body = Some(body);
                    }
                    "K4WWud" => {
                        k4wwud_body = Some(body);
                    }
                    _ => {}
                }
            }

            let side_nav_request = gemini_canvas::build_text_state_variant_preflight_request(
                bootstrap,
                source_path,
                41usize,
                40usize,
                Value::from(0),
                "side_nav_open_by_default",
                gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
            )?;
            self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                payload,
                model,
                &side_nav_request,
                session,
                &neutral_header,
                None,
                timeout,
            )
            .await?;

            let request = gemini_canvas::build_text_batchexecute_request(
                "CNgdBe",
                json!([1, [bootstrap.language.clone()], 0]),
                bootstrap,
                source_path,
            )?;
            self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                payload,
                model,
                &request,
                session,
                &neutral_header,
                None,
                timeout,
            )
            .await?;

            let selected_bootstrap_request =
                gemini_canvas::build_text_bootstrap_preflight_request(bootstrap, source_path)?;
            let selected_bootstrap_body = self
                .send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                    payload,
                    model,
                    &selected_bootstrap_request,
                    session,
                    &selected_header,
                    None,
                    timeout,
                )
                .await?;

            for (rpcid, rpc_payload) in [
                ("ku4Jyf", ku4jyf_payload.clone()),
                ("CNgdBe", json!([2, [bootstrap.language.clone()], 0])),
            ] {
                let request = gemini_canvas::build_text_batchexecute_request(
                    rpcid,
                    rpc_payload,
                    bootstrap,
                    source_path,
                )?;
                self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                    payload,
                    model,
                    &request,
                    session,
                    &neutral_header,
                    None,
                    timeout,
                )
                .await?;
            }

            let selected_mode = gemini_canvas::build_mode_selection_preflight_request(
                bootstrap,
                source_path,
                gemini_canvas::GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID,
            )?;
            self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                payload,
                model,
                &selected_mode,
                session,
                &neutral_header,
                None,
                timeout,
            )
            .await?;

            for request in [
                gemini_canvas::build_text_state_variant_preflight_request(
                    bootstrap,
                    source_path,
                    94usize,
                    93usize,
                    Value::String("NULL".to_string()),
                    "current_popup_id",
                    gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
                )?,
                gemini_canvas::build_text_state_variant_preflight_request(
                    bootstrap,
                    source_path,
                    41usize,
                    40usize,
                    Value::from(0),
                    "side_nav_open_by_default",
                    gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
                )?,
                gemini_canvas::build_text_state_variant_preflight_request(
                    bootstrap,
                    source_path,
                    192usize,
                    191usize,
                    json!([["image_generation_soft:1", "music_generation_soft:1"]]),
                    "tool_menu_soft_badge_impression_counts",
                    gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
                )?,
            ] {
                self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                    payload,
                    model,
                    &request,
                    session,
                    &neutral_header,
                    None,
                    timeout,
                )
                .await?;
            }

            let xhau0b_request = gemini_canvas::build_text_batchexecute_request(
                "XhaU0b",
                json!([4, [2], 3]),
                bootstrap,
                source_path,
            )?;
            self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                payload,
                model,
                &xhau0b_request,
                session,
                &neutral_header,
                None,
                timeout,
            )
            .await?;

            let final_activity_request = gemini_canvas::build_text_batchexecute_request(
                "ESY5D",
                json!([[["bard_activity_enabled"]]]),
                bootstrap,
                source_path,
            )?;
            self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                payload,
                model,
                &final_activity_request,
                session,
                &neutral_header,
                None,
                timeout,
            )
            .await?;

            return Ok(GeminiCanvasMediaCaptureParityPreflightResult {
                selected_bootstrap_body,
                maziqc_probe_body,
                maziqc_full_body,
                o30o0e_body,
                k4wwud_body,
            });
        }

        let initial_requests: Vec<(&str, Value, &str)> = if is_image_mode {
            vec![
                ("aPya6c", json!([]), empty_model_header.as_str()),
                (
                    "o30O0e",
                    json!([
                        ["me"],
                        [
                            [["person.photo", "person.name", "person.email"]],
                            null,
                            [1, 7]
                        ]
                    ]),
                    neutral_header.as_str(),
                ),
                ("GPRiHf", json!([]), neutral_header.as_str()),
                ("otAQ7b", json!([]), neutral_header.as_str()),
                (
                    "qpEbW",
                    json!([[
                        [1, selection_mode_index],
                        [2, selection_mode_index],
                        [6, selection_mode_index]
                    ]]),
                    neutral_header.as_str(),
                ),
                ("cYRIkd", json!([language.clone()]), neutral_header.as_str()),
            ]
        } else {
            vec![
                ("otAQ7b", json!([]), neutral_header.as_str()),
                ("sJBwce", json!([[1, 2]]), neutral_header.as_str()),
                ("DYBcR", json!([language.clone()]), neutral_header.as_str()),
                ("aPya6c", json!([]), empty_model_header.as_str()),
                ("cYRIkd", json!([language.clone()]), neutral_header.as_str()),
            ]
        };

        for (rpcid, rpc_payload, model_header) in initial_requests {
            let request = gemini_canvas::build_text_batchexecute_request(
                rpcid,
                rpc_payload,
                bootstrap,
                source_path,
            )?;
            let body = self
                .send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                    payload,
                    model,
                    &request,
                    session,
                    model_header,
                    Some(initial_header2),
                    timeout,
                )
                .await?;
            if rpcid == "o30O0e" {
                o30o0e_body = Some(body);
            }
        }

        let early_side_nav = gemini_canvas::build_text_state_variant_preflight_request(
            bootstrap,
            source_path,
            41usize,
            40usize,
            Value::from(0),
            "side_nav_open_by_default",
            gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
        )?;
        self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
            payload,
            model,
            &early_side_nav,
            session,
            &neutral_header,
            Some(initial_header2),
            timeout,
        )
        .await?;

        let middle_initial_requests: Vec<(&str, Value)> = if is_image_mode {
            vec![
                ("K4WWud", json!([[0], [language.clone()]])),
                (
                    "ozz5Z",
                    json!([[
                        [[null, "1", 447]],
                        [[null, "1", 448]],
                        [[null, "1", 702]],
                        [[null, "1", 961]],
                        [[null, "1", 960]],
                        [[null, "1", 1062]],
                        [[null, "1", 1240]],
                        [[null, "1", 1237]],
                        [[null, "1", 1238]],
                        [[null, "1", 1239]],
                        [[null, "1", 1241]],
                    ]]),
                ),
                ("sJBwce", json!([[1, 2]])),
                ("DYBcR", json!([language.clone()])),
                ("maGuAc", json!([1])),
                ("Te6DCf", json!([[language.clone()], [1]])),
                ("CNgdBe", json!([1, [language.clone()], 0])),
                ("CNgdBe", json!([2, [language.clone()], 0])),
            ]
        } else {
            vec![
                ("GPRiHf", json!([])),
                ("maGuAc", json!([0])),
                ("maGuAc", json!([1])),
                (
                    "qpEbW",
                    json!([[
                        [1, selection_mode_index],
                        [2, selection_mode_index],
                        [6, selection_mode_index]
                    ]]),
                ),
                ("Te6DCf", json!([[language.clone()], [1]])),
                ("K4WWud", json!([[0], [language.clone()]])),
                ("CNgdBe", json!([1, [language.clone()], 0])),
                (
                    "ozz5Z",
                    json!([[
                        [[null, "1", 447]],
                        [[null, "1", 448]],
                        [[null, "1", 702]],
                        [[null, "1", 961]],
                        [[null, "1", 960]],
                        [[null, "1", 1062]],
                        [[null, "1", 1240]],
                        [[null, "1", 1237]],
                        [[null, "1", 1238]],
                        [[null, "1", 1239]],
                        [[null, "1", 1241]],
                    ]]),
                ),
                ("CNgdBe", json!([2, [language.clone()], 0])),
            ]
        };

        for (rpcid, rpc_payload) in middle_initial_requests {
            let request = gemini_canvas::build_text_batchexecute_request(
                rpcid,
                rpc_payload,
                bootstrap,
                source_path,
            )?;
            let body = self
                .send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                    payload,
                    model,
                    &request,
                    session,
                    &neutral_header,
                    Some(initial_header2),
                    timeout,
                )
                .await?;
            match rpcid {
                "o30O0e" => {
                    o30o0e_body = Some(body);
                }
                "K4WWud" => {
                    k4wwud_body = Some(body);
                }
                _ => {}
            }
        }

        let media_state_keys =
            gemini_canvas::build_image_state_keys_preflight_request(bootstrap, source_path)?;
        self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
            payload,
            model,
            &media_state_keys,
            session,
            &neutral_header,
            Some(selected_header2),
            timeout,
        )
        .await?;

        for (rpcid, rpc_payload) in [
            ("ESY5D", json!([[["bard_activity_enabled"]]])),
            ("MaZiqc", json!([13, null, [1, null, 1]])),
            ("MaZiqc", json!([13, null, [0, null, 1]])),
            ("mhs1xe", json!([[1, 3]])),
        ] {
            let request = gemini_canvas::build_text_batchexecute_request(
                rpcid,
                rpc_payload,
                bootstrap,
                source_path,
            )?;
            let body = self
                .send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                    payload,
                    model,
                    &request,
                    session,
                    &neutral_header,
                    Some(selected_header2),
                    timeout,
                )
                .await?;
            match rpcid {
                "MaZiqc" if maziqc_probe_body.is_none() => {
                    maziqc_probe_body = Some(body);
                }
                "MaZiqc" if maziqc_full_body.is_none() => {
                    maziqc_full_body = Some(body);
                }
                _ => {}
            }
        }

        let selected_mode = gemini_canvas::build_mode_selection_preflight_request(
            bootstrap,
            source_path,
            gemini_canvas::GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID,
        )?;
        self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
            payload,
            model,
            &selected_mode,
            session,
            &flagged_neutral_header,
            Some(selected_header2),
            timeout,
        )
        .await?;

        let selected_bootstrap =
            gemini_canvas::build_text_bootstrap_preflight_request(bootstrap, source_path)?;
        let selected_bootstrap_body = self
            .send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                payload,
                model,
                &selected_bootstrap,
                session,
                &flagged_selected_header,
                Some(selected_header2),
                timeout,
            )
            .await?;

        for request in [
            gemini_canvas::build_text_state_variant_preflight_request(
                bootstrap,
                source_path,
                41usize,
                40usize,
                Value::from(0),
                "side_nav_open_by_default",
                gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
            )?,
            gemini_canvas::build_text_state_variant_preflight_request(
                bootstrap,
                source_path,
                94usize,
                93usize,
                Value::String("NULL".to_string()),
                "current_popup_id",
                gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
            )?,
            gemini_canvas::build_text_state_variant_preflight_request(
                bootstrap,
                source_path,
                87usize,
                86usize,
                popup_zs_visits_cooldown,
                "popup_zs_visits_cooldown",
                gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
            )?,
            gemini_canvas::build_text_state_variant_preflight_request(
                bootstrap,
                source_path,
                192usize,
                191usize,
                tool_menu_soft_badge_impression_counts,
                "tool_menu_soft_badge_impression_counts",
                gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
            )?,
        ] {
            self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                payload,
                model,
                &request,
                session,
                &flagged_neutral_header,
                Some(selected_header2),
                timeout,
            )
            .await?;
        }

        let ku4jyf_first = gemini_canvas::build_text_batchexecute_request(
            "ku4Jyf",
            json!([[
                language,
                null,
                null,
                null,
                primary_mode_index,
                null,
                null,
                [32],
                null,
                []
            ]]),
            bootstrap,
            source_path,
        )?;
        self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
            payload,
            model,
            &ku4jyf_first,
            session,
            &flagged_neutral_header,
            Some(selected_header2),
            timeout,
        )
        .await?;

        for (rpcid, rpc_payload) in [
            (
                "CNgdBe",
                json!([2, [bootstrap.language.clone()], 0, null, [2]]),
            ),
            ("XhaU0b", xhau0b_payload),
        ] {
            let request = gemini_canvas::build_text_batchexecute_request(
                rpcid,
                rpc_payload,
                bootstrap,
                source_path,
            )?;
            self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                payload,
                model,
                &request,
                session,
                &flagged_neutral_header,
                Some(selected_header2),
                timeout,
            )
            .await?;
        }

        Ok(GeminiCanvasMediaCaptureParityPreflightResult {
            selected_bootstrap_body,
            maziqc_probe_body,
            maziqc_full_body,
            o30o0e_body,
            k4wwud_body,
        })
    }

    async fn execute_gemini_canvas_media_legacy_preflight_sequence(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        mode_index: i64,
        batchexecute_header_id: Option<&str>,
        timeout: Duration,
    ) -> Result<(), GatewayError> {
        self.execute_gemini_canvas_page_init_preflight_sequence(
            payload,
            model,
            bootstrap,
            source_path,
            session,
            timeout,
            true,
        )
        .await?;
        self.execute_gemini_canvas_mode_selection_preflight(
            payload,
            model,
            bootstrap,
            source_path,
            session,
            gemini_canvas::GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID,
            batchexecute_header_id,
            timeout,
        )
        .await?;
        self.execute_gemini_canvas_media_operation_selection_preflight(
            payload,
            model,
            bootstrap,
            source_path,
            session,
            mode_index,
            batchexecute_header_id,
            timeout,
        )
        .await?;
        self.execute_gemini_canvas_media_state_preflight_sequence(
            payload,
            model,
            bootstrap,
            source_path,
            session,
            mode_index,
            batchexecute_header_id,
            timeout,
        )
        .await?;
        self.execute_gemini_canvas_text_bootstrap_preflight(
            payload,
            model,
            bootstrap,
            source_path,
            session,
            "",
            "",
            timeout,
        )
        .await?;
        self.execute_gemini_canvas_text_state_preflight(
            payload,
            model,
            bootstrap,
            source_path,
            session,
            "",
            "",
            timeout,
        )
        .await
    }

    async fn execute_gemini_canvas_text_bootstrap_preflight(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        _origin: &str,
        _authorization: &str,
        timeout: Duration,
    ) -> Result<(), GatewayError> {
        let request =
            gemini_canvas::build_text_bootstrap_preflight_request(bootstrap, source_path)?;
        self.send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &request,
            session,
            gemini_canvas::GEMINI_CANVAS_TEXT_BOOTSTRAP_MODEL_HEADER,
            timeout,
        )
        .await
        .map(|_| ())
    }

    async fn execute_gemini_canvas_text_state_preflight(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        _origin: &str,
        _authorization: &str,
        timeout: Duration,
    ) -> Result<(), GatewayError> {
        let request = gemini_canvas::build_text_state_preflight_request(bootstrap, source_path)?;
        self.send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &request,
            session,
            gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_MODEL_HEADER,
            timeout,
        )
        .await
        .map(|_| ())
    }

    async fn execute_gemini_canvas_text_state_variant_preflight(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        state_len: usize,
        tail_index: usize,
        tail_value: Value,
        marker: &str,
        timeout: Duration,
    ) -> Result<(), GatewayError> {
        let request = gemini_canvas::build_text_state_variant_preflight_request(
            bootstrap,
            source_path,
            state_len,
            tail_index,
            tail_value,
            marker,
            gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
        )?;
        self.send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &request,
            session,
            gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_MODEL_HEADER,
            timeout,
        )
        .await
        .map(|_| ())
    }

    async fn execute_gemini_canvas_text_generic_preflight(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        rpcid: &str,
        rpc_payload: Value,
        model_header: &str,
        timeout: Duration,
    ) -> Result<(), GatewayError> {
        let request = gemini_canvas::build_text_batchexecute_request(
            rpcid,
            rpc_payload,
            bootstrap,
            source_path,
        )?;
        self.send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &request,
            session,
            model_header,
            timeout,
        )
        .await
        .map(|_| ())
    }

    pub(crate) async fn send_gemini_canvas_text_batchexecute_request(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        request: &gemini_web::GeminiWebRequest,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        model_header: &str,
        timeout: Duration,
    ) -> Result<String, GatewayError> {
        let mut session_clone = session.clone();
        self.send_gemini_canvas_text_batchexecute_request_refreshing_session(
            payload,
            model,
            request,
            &mut session_clone,
            model_header,
            timeout,
        )
        .await
    }

    async fn send_gemini_canvas_text_batchexecute_request_capture_aligned(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        request: &gemini_web::GeminiWebRequest,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        model_header: &str,
        timeout: Duration,
    ) -> Result<String, GatewayError> {
        let mut session_clone = session.clone();
        self.send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
            payload,
            model,
            request,
            &mut session_clone,
            model_header,
            timeout,
        )
        .await
    }

    async fn send_gemini_canvas_text_batchexecute_request_refreshing_session(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        request: &gemini_web::GeminiWebRequest,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        model_header: &str,
        timeout: Duration,
    ) -> Result<String, GatewayError> {
        self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
            payload,
            model,
            request,
            session,
            model_header,
            None,
            timeout,
        )
        .await
    }

    async fn send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        request: &gemini_web::GeminiWebRequest,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        model_header: &str,
        timeout: Duration,
    ) -> Result<String, GatewayError> {
        self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session_internal(
            payload,
            model,
            request,
            session,
            model_header,
            None,
            timeout,
            true,
        )
        .await
    }

    async fn send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        request: &gemini_web::GeminiWebRequest,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        model_header: &str,
        model_header_2_override: Option<&str>,
        timeout: Duration,
    ) -> Result<String, GatewayError> {
        self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session_internal(
            payload,
            model,
            request,
            session,
            model_header,
            model_header_2_override,
            timeout,
            false,
        )
        .await
    }

    async fn send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session_internal(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        request: &gemini_web::GeminiWebRequest,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        model_header: &str,
        model_header_2_override: Option<&str>,
        timeout: Duration,
        capture_aligned_headers: bool,
    ) -> Result<String, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let configured_base_url = payload.base_url.trim_end_matches('/');
        let effective_base_url = if configured_base_url.is_empty() {
            gemini_canvas_http_origin(payload)
        } else {
            configured_base_url.to_string()
        };
        let batchexecute_url = format!("{effective_base_url}/_/BardChatUi/data/batchexecute");
        let accept_language = gemini_canvas::locale_from_payload(payload);
        let same_origin_referer = format!("{effective_base_url}/");
        let mut effective_request = request.clone();
        let mut xsrf_retry_token: Option<String> = None;
        let header2 = model_header_2_override
            .unwrap_or(gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_MODEL_HEADER_2);

        loop {
            let mut headers = gemini_web_reverse_modular::build_headers(payload, None, model);
            if capture_aligned_headers {
                apply_gemini_canvas_capture_aligned_batchexecute_headers(
                    &mut headers,
                    session,
                    &accept_language,
                    model_header,
                    header2,
                );
            } else {
                apply_gemini_canvas_same_origin_batchexecute_headers(
                    &mut headers,
                    session,
                    &accept_language,
                    model_header,
                    header2,
                );
            }
            insert_header_map_value(&mut headers, "referer", &same_origin_referer);
            insert_header_map_value(
                &mut headers,
                "x-same-domain",
                gemini_web::GEMINI_WEB_DEFAULT_SAME_DOMAIN_HEADER,
            );

            let response = self
                .http
                .request(Method::POST, &batchexecute_url)
                .headers(headers.clone())
                .query(&effective_request.query)
                .timeout(timeout.max(Duration::from_secs(30)))
                .form(&effective_request.form)
                .send()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            apply_gemini_canvas_response_cookies(response.headers(), session);
            let status = response.status().as_u16();
            let content_type = response
                .headers()
                .get(rquest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let body_text = response
                .text()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            if (200..300).contains(&status)
                && !gemini_web::response_indicates_browser_challenge(
                    status,
                    content_type.as_deref(),
                    &body_text,
                )
                && !gemini_web::response_indicates_session_invalid(
                    status,
                    content_type.as_deref(),
                    &body_text,
                )
            {
                return Ok(body_text);
            }

            if status == 400 {
                if let Some(token) = extract_gemini_canvas_batchexecute_xsrf_token(&body_text) {
                    let current_at = header_map_string_from_form(&effective_request.form, "at");
                    if xsrf_retry_token.as_deref() != Some(token.as_str())
                        && current_at.as_deref() != Some(token.as_str())
                    {
                        debug!(
                            provider,
                            xsrf_token_preview = %truncate_response_preview(&token, 24),
                            "retrying gemini canvas batchexecute with xsrf token extracted from upstream error"
                        );
                        upsert_form_field(&mut effective_request.form, "at", &token);
                        xsrf_retry_token = Some(token);
                        continue;
                    }
                }
            }

            let rpcids = effective_request
                .query
                .iter()
                .find(|(key, _)| key == "rpcids")
                .map(|(_, value)| value.as_str());
            if status == 302 {
                let rpcids_log = rpcids.unwrap_or("<unknown>");
                let browser_fallback = async {
                    let runtime = gemini_canvas::runtime_from_payload(payload)?;
                    let browser_runtime_state_object_key =
                        gemini_canvas::browser_runtime_state_object_key_for_browser_operation(
                            payload, "video",
                        )
                        .unwrap_or_else(|| runtime.runtime_state_object_key.clone());
                    let browser_pool_base_url =
                        self.ensure_gemini_canvas_browser_pool(provider).await?;
                    self.execute_gemini_canvas_browser_fetch_form_request(
                        provider,
                        &browser_pool_base_url,
                        &effective_base_url,
                        &runtime.share_id,
                        &browser_runtime_state_object_key,
                        gemini_canvas::browser_cdp_url(payload).as_deref(),
                        gemini_canvas::browser_cookie_header(payload).as_deref(),
                        &batchexecute_url,
                        &effective_request.query,
                        &headers,
                        &effective_request.form,
                        timeout,
                    )
                    .await
                }
                .await;

                match browser_fallback {
                    Ok(invocation) if (200..300).contains(&invocation.status) => {
                        if let Some(browser_body) = invocation.body_text {
                            let browser_content_type = invocation.content_type.as_deref();
                            if !gemini_web::response_indicates_browser_challenge(
                                invocation.status,
                                browser_content_type,
                                &browser_body,
                            ) && !gemini_web::response_indicates_session_invalid(
                                invocation.status,
                                browser_content_type,
                                &browser_body,
                            ) {
                                debug!(
                                    provider,
                                    rpcids = rpcids_log,
                                    "recovered Gemini Canvas batchexecute through browser-backed fetch after upstream challenge redirect"
                                );
                                return Ok(browser_body);
                            }
                        }
                    }
                    Ok(invocation) => {
                        debug!(
                            provider,
                            rpcids = rpcids_log,
                            browser_status = invocation.status,
                            "Gemini Canvas batchexecute browser-backed fetch did not recover the upstream redirect"
                        );
                    }
                    Err(error) => {
                        debug!(
                            provider,
                            rpcids = rpcids_log,
                            error = %summarize_gateway_error(&error),
                            "Gemini Canvas batchexecute browser-backed fetch failed after upstream redirect"
                        );
                    }
                }
            }

            let mut classified =
                classify_gemini_canvas_pure_http_error(status, content_type.as_deref(), &body_text);
            classified.message = format!(
                "Gemini Canvas batchexecute failed. status={status}; body_preview={}",
                compact_response_preview(&body_text, 240)
            );
            return Err(classified);
        }
    }

    async fn collect_gemini_canvas_stream_generate_body(
        &self,
        response: rquest::Response,
        provider: &str,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        allow_early_locator: bool,
    ) -> Result<String, GatewayError> {
        const IMAGE_EDIT_SHORT_ACK_GRACE_SECS: u64 = 30;
        const IMAGE_EDIT_ASYNC_FOLLOWUP_GRACE_SECS: u64 = 45;
        let mut stream = response.bytes_stream();
        let mut body_text = String::new();
        let mut short_ack_seen_at: Option<Instant> = None;
        let mut async_followup_ready_seen_at: Option<Instant> = None;

        loop {
            let handoff_wait = if allow_early_locator
                && operation == gemini_canvas::GeminiCanvasMediaOperation::Image
            {
                if let Some(seen_at) = async_followup_ready_seen_at {
                    Some((
                        "stream.async-followup-handoff",
                        Duration::from_secs(IMAGE_EDIT_ASYNC_FOLLOWUP_GRACE_SECS)
                            .saturating_sub(seen_at.elapsed()),
                    ))
                } else if let Some(seen_at) = short_ack_seen_at {
                    Some((
                        "stream.short-ack-handoff",
                        Duration::from_secs(IMAGE_EDIT_SHORT_ACK_GRACE_SECS)
                            .saturating_sub(seen_at.elapsed()),
                    ))
                } else {
                    None
                }
            } else {
                None
            };

            let next_chunk = if let Some((trace_label, wait)) = handoff_wait {
                if wait.is_zero() {
                    append_gemini_canvas_image_edit_trace(
                        trace_label,
                        compact_response_preview(&body_text, 220),
                    );
                    return Ok(body_text);
                }
                match timeout(wait, stream.next()).await {
                    Ok(chunk_result) => chunk_result,
                    Err(_) => {
                        append_gemini_canvas_image_edit_trace(
                            trace_label,
                            compact_response_preview(&body_text, 220),
                        );
                        return Ok(body_text);
                    }
                }
            } else {
                stream.next().await
            };

            let Some(chunk_result) = next_chunk else {
                break;
            };
            let chunk = match chunk_result {
                Ok(chunk) => chunk,
                Err(error) => {
                    if !body_text.is_empty() {
                        return Ok(body_text);
                    }
                    return Err(classify_network_error(&error, Some(provider)));
                }
            };
            body_text.push_str(String::from_utf8_lossy(&chunk).as_ref());

            if gemini_canvas::extract_stream_generate_media_assets(&body_text, operation).is_ok() {
                if allow_early_locator
                    && operation == gemini_canvas::GeminiCanvasMediaOperation::Image
                {
                    append_gemini_canvas_image_edit_trace(
                        "stream.asset-ready",
                        compact_response_preview(&body_text, 220),
                    );
                }
                return Ok(body_text);
            }
            if allow_early_locator
                && operation == gemini_canvas::GeminiCanvasMediaOperation::Image
                && gemini_canvas::stream_generate_indicates_image_edit_async_followup_ready(
                    &body_text,
                )
            {
                if async_followup_ready_seen_at.is_none() {
                    async_followup_ready_seen_at = Some(Instant::now());
                    append_gemini_canvas_image_edit_trace(
                        "stream.async-followup-ready",
                        compact_response_preview(&body_text, 220),
                    );
                }
            }
            if allow_early_locator
                && operation == gemini_canvas::GeminiCanvasMediaOperation::Image
                && async_followup_ready_seen_at.is_none()
                && gemini_canvas::stream_generate_is_image_edit_short_ack(&body_text)
            {
                if short_ack_seen_at.is_none() {
                    short_ack_seen_at = Some(Instant::now());
                    append_gemini_canvas_image_edit_trace(
                        "stream.short-ack-observed",
                        compact_response_preview(&body_text, 220),
                    );
                }
            }
            if allow_early_locator
                && operation != gemini_canvas::GeminiCanvasMediaOperation::Image
                && gemini_canvas::extract_stream_generate_locator(&body_text).is_ok()
            {
                return Ok(body_text);
            }
        }

        Ok(body_text)
    }

    async fn execute_gemini_canvas_direct_http_stream_generate_text(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        timeout: Duration,
    ) -> Result<CanonicalRelayResponse, GatewayError> {
        let wrap_parse_error = |body_text: &str, error: GatewayError| {
            let preview = |body: &str| {
                let trimmed = body.trim();
                if trimmed.is_empty() {
                    "<empty>".to_string()
                } else {
                    truncate_response_preview(trimmed, 180).to_string()
                }
            };
            let tail_preview = |body: &str| {
                let trimmed = body.trim();
                if trimmed.is_empty() {
                    "<empty>".to_string()
                } else {
                    let chars: Vec<char> = trimmed.chars().collect();
                    let start = chars.len().saturating_sub(180);
                    chars[start..].iter().collect::<String>()
                }
            };
            let stream_head_preview = preview(body_text);
            let stream_tail_preview = tail_preview(body_text);
            let preview = body_text.split_whitespace().collect::<Vec<_>>().join(" ");
            debug!(
                provider = "gemini_canvas_compatible",
                body_preview = %preview.chars().take(400).collect::<String>(),
                "Gemini Canvas StreamGenerate direct HTTP returned an unparseable frame payload"
            );
            let mut wrapped = error.with_provider("gemini_canvas_compatible");
            wrapped.message = format!(
                "{}; stream_head={stream_head_preview}; stream_tail={stream_tail_preview}",
                wrapped.message
            );
            wrapped
        };

        let mut initial_stream_send_failure: Option<String> = None;
        let first_body = match self
            .execute_gemini_canvas_direct_http_stream_generate_body(
                payload,
                model,
                runtime,
                gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX,
                prompt,
                timeout,
                true,
                None,
                None,
            )
            .await
        {
            Ok(body) => body,
            Err(first_send_error) => {
                let first_send_summary = summarize_gateway_error(&first_send_error);
                initial_stream_send_failure = Some(first_send_summary.clone());
                debug!(
                    provider = "gemini_canvas_compatible",
                    error = %first_send_summary,
                    "Gemini Canvas StreamGenerate primary direct HTTP send failed before parsing; retrying legacy pure HTTP builder"
                );
                self.execute_gemini_canvas_direct_http_stream_generate_body(
                    payload,
                    model,
                    runtime,
                    gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX,
                    prompt,
                    timeout,
                    false,
                    None,
                    None,
                )
                .await
                .map_err(|retry_error| {
                    let mut retry_error = retry_error;
                    retry_error.message = format!(
                        "{}; initial_stream_send_failure={first_send_summary}",
                        retry_error.message
                    );
                    retry_error
                })?
            }
        };
        let canonical = match gemini_web::accumulate_gemini_web_response(&first_body, model) {
            Ok(canonical) => canonical,
            Err(first_error) => {
                let first_error = wrap_parse_error(&first_body, first_error);
                debug!(
                    provider = "gemini_canvas_compatible",
                    error = %summarize_gateway_error(&first_error),
                    "Gemini Canvas StreamGenerate replay did not yield a usable text candidate; retrying legacy pure HTTP builder"
                );
                let retry_body = self
                    .execute_gemini_canvas_direct_http_stream_generate_body(
                        payload,
                        model,
                        runtime,
                        gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX,
                        prompt,
                        timeout,
                        false,
                        None,
                        None,
                    )
                    .await
                    .map_err(|retry_error| {
                        let mut retry_error = retry_error;
                        let mut retry_detail = format!(
                            "initial_stream_parse_failure={}",
                            summarize_gateway_error(&first_error)
                        );
                        if let Some(initial_send_failure) = initial_stream_send_failure.as_deref() {
                            retry_detail = format!(
                                "{retry_detail}; initial_stream_send_failure={initial_send_failure}"
                            );
                        }
                        retry_error.message = format!("{}; {retry_detail}", retry_error.message);
                        retry_error
                    })?;
                gemini_web::accumulate_gemini_web_response(&retry_body, model).map_err(
                    |retry_parse_error| {
                        let mut retry_parse_error =
                            wrap_parse_error(&retry_body, retry_parse_error);
                        let mut retry_detail = format!(
                            "initial_stream_parse_failure={}",
                            summarize_gateway_error(&first_error)
                        );
                        if let Some(initial_send_failure) = initial_stream_send_failure.as_deref() {
                            retry_detail = format!(
                                "{retry_detail}; initial_stream_send_failure={initial_send_failure}"
                            );
                        }
                        retry_parse_error.message =
                            format!("{}; {retry_detail}", retry_parse_error.message);
                        retry_parse_error
                    },
                )?
            }
        };
        let _ = req;
        Ok(canonical)
    }

    async fn execute_gemini_canvas_browser_fetch_form_request(
        &self,
        provider: &str,
        browser_pool_base_url: &str,
        base_url: &str,
        share_id: &str,
        runtime_state_object_key: &str,
        browser_cdp_url: Option<&str>,
        cookie_header: Option<&str>,
        request_url: &str,
        query: &[(String, String)],
        headers: &HeaderMap,
        form: &[(String, String)],
        timeout: Duration,
    ) -> Result<GeminiCanvasBrowserFetchInvocationResult, GatewayError> {
        let final_url = append_query_pairs_to_url(request_url, query);
        let mut request_form = form.to_vec();
        let mut xsrf_retry_token: Option<String> = None;
        let referrer = header_map_string(headers, "referer")
            .or_else(|| {
                if base_url.contains("gemini.google.com") {
                    Some(format!("{}/", base_url.trim_end_matches('/')))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| base_url.to_string());
        loop {
            let input =
                gemini_canvas_web_reverse_modular::build_connected_fetch_form_invocation_input(
                    base_url,
                    share_id,
                    runtime_state_object_key,
                    browser_cdp_url,
                    cookie_header,
                    &final_url,
                    &gemini_canvas_browser_fetch_headers_from_header_map(headers),
                    &serialize_form_urlencoded_pairs(&request_form),
                    &referrer,
                    timeout,
                );
            let response = self
                .http
                .request(Method::POST, format!("{}/fetch", browser_pool_base_url))
                .header(rquest::header::CONTENT_TYPE, "application/json")
                .timeout(timeout.max(Duration::from_secs(30)))
                .json(&input)
                .send()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;

            let status = response.status().as_u16();
            let body_text = response
                .text()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            match gemini_canvas_program_web_reverse_modular::parse_connected_fetch_invocation_response(
                provider, status, &body_text,
            ) {
                Ok(invocation) => return Ok(invocation),
                Err(error) => {
                    if let Some(token) = maybe_retry_gemini_canvas_form_xsrf_token(
                        &mut request_form,
                        &body_text,
                        &mut xsrf_retry_token,
                    ) {
                        debug!(
                            provider,
                            xsrf_token_preview = %truncate_response_preview(&token, 24),
                            "retrying gemini canvas browser-backed connected fetch with xsrf token extracted from upstream error"
                        );
                        continue;
                    }
                    return Err(error);
                }
            }
        }
    }

    async fn execute_gemini_canvas_connected_fetch_invocation_with_program_context(
        &self,
        payload: &ProviderAccountPayload,
        provider: &str,
        browser_pool_base_url: &str,
        base_url: &str,
        config: &crate::protocol::gemini::canvas_program_web_reverse::GeminiCanvasProgramRelayConfig,
        runtime_api: Option<&GeminiCanvasRuntimeApiContext>,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
        request_url: &str,
        method: Method,
        request_body: Option<&Value>,
        google_fetch_mode: &str,
        timeout: Duration,
    ) -> Result<GeminiCanvasBrowserFetchInvocationResult, GatewayError> {
        let mut input = gemini_canvas_program_web_reverse_modular::build_connected_fetch_invocation_input_with_method(
                base_url,
                config,
                request_url,
                method.as_str(),
                request_body,
                google_fetch_mode,
                timeout,
            );
        input["authUser"] = Value::String(gemini_canvas::direct_http_auth_user(payload));
        if gemini_canvas_program_web_reverse_modular::connected_fetch_mode_is_canvas_proxy(
            google_fetch_mode,
        ) {
            gemini_canvas_program_web_reverse_modular::apply_program_connected_fetch_identity_contract(
                &mut input,
                runtime_api.map(|context| context.session.auth_user.as_str()),
                runtime_api.map(|context| context.payload.api_key.as_str()),
                extra_headers,
            )?;
        }
        let response = self
            .http
            .request(Method::POST, format!("{}/fetch", browser_pool_base_url))
            .header(rquest::header::CONTENT_TYPE, "application/json")
            .timeout(timeout.max(Duration::from_secs(30)))
            .json(&input)
            .send()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;

        let status = response.status().as_u16();
        let body_text = response
            .text()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        let invocation =
            gemini_canvas_program_web_reverse_modular::parse_connected_fetch_invocation_response(
                provider, status, &body_text,
            )?;
        persist_gemini_canvas_program_runtime_material(
            self.redis_pool.as_ref(),
            self.pg_pool.as_ref(),
            payload,
            gemini_canvas_program_web_reverse_modular::runtime_patch_from_browser_fetch(
                &invocation,
            ),
        )
        .await;
        Ok(invocation)
    }

    async fn execute_gemini_canvas_connected_fetch_get_bytes_with_program_context(
        &self,
        payload: &ProviderAccountPayload,
        provider: &str,
        browser_pool_base_url: &str,
        base_url: &str,
        config: &crate::protocol::gemini::canvas_program_web_reverse::GeminiCanvasProgramRelayConfig,
        runtime_api: Option<&GeminiCanvasRuntimeApiContext>,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
        request_url: &str,
        google_fetch_mode: &str,
        timeout: Duration,
    ) -> Result<(bytes::Bytes, Option<String>), GatewayError> {
        let invocation = self
            .execute_gemini_canvas_connected_fetch_invocation_with_program_context(
                payload,
                provider,
                browser_pool_base_url,
                base_url,
                config,
                runtime_api,
                extra_headers,
                request_url,
                Method::GET,
                None,
                google_fetch_mode,
                timeout,
            )
            .await?;
        gemini_canvas_program_web_reverse_modular::decode_connected_fetch_body_bytes(
            provider,
            &invocation,
        )
    }

    async fn execute_gemini_canvas_browser_request_with_recovery(
        &self,
        provider: &str,
        browser_pool_base_url: &str,
        base_url: &str,
        share_id: &str,
        runtime_state_object_key: &str,
        browser_cdp_url: Option<&str>,
        cookie_header: Option<&str>,
        operation: &str,
        prompt: &str,
        locale: &str,
        timeout: Duration,
    ) -> Result<GeminiCanvasBrowserInvocationResult, GatewayError> {
        gemini_canvas_web_reverse_modular::execute_browser_request_with_recovery(
            &self.http,
            timeout,
            provider,
            browser_pool_base_url,
            base_url,
            share_id,
            runtime_state_object_key,
            browser_cdp_url,
            cookie_header,
            operation,
            prompt,
            locale,
        )
        .await
    }

    async fn persist_gemini_canvas_program_runtime_result_if_needed(
        &self,
        payload: &ProviderAccountPayload,
        result: &GeminiCanvasBrowserInvocationResult,
    ) {
        if payload.adapter != "gemini_canvas_program_web_reverse_compatible" {
            return;
        }
        persist_gemini_canvas_program_runtime_material(
            self.redis_pool.as_ref(),
            self.pg_pool.as_ref(),
            payload,
            gemini_canvas_program_web_reverse_modular::runtime_patch_from_browser_invocation(
                result,
            ),
        )
        .await;
    }

    async fn execute_gemini_canvas_owned_browser_invocation(
        &self,
        payload: &ProviderAccountPayload,
        provider: &str,
        browser_pool_base_url: &str,
        base_url: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        program_config: Option<
            &crate::protocol::gemini::canvas_program_web_reverse::GeminiCanvasProgramRelayConfig,
        >,
        operation: &str,
        prompt: &str,
        locale: &str,
        timeout: Duration,
    ) -> Result<GeminiCanvasBrowserInvocationResult, GatewayError> {
        let browser_runtime_state_object_key =
            gemini_canvas::browser_runtime_state_object_key_for_browser_operation(
                payload, operation,
            )
            .unwrap_or_else(|| runtime.runtime_state_object_key.clone());
        let browser_cookie_header = gemini_canvas::browser_cookie_header(payload);
        let result = if let Some(config) = program_config {
            self.execute_gemini_canvas_browser_request_with_program_context_recovery(
                provider,
                browser_pool_base_url,
                base_url,
                config,
                operation,
                prompt,
                locale,
                timeout,
            )
            .await?
        } else {
            self.execute_gemini_canvas_browser_request_with_recovery(
                provider,
                browser_pool_base_url,
                base_url,
                &runtime.share_id,
                &browser_runtime_state_object_key,
                gemini_canvas::browser_cdp_url(payload).as_deref(),
                browser_cookie_header.as_deref(),
                operation,
                prompt,
                locale,
                timeout,
            )
            .await?
        };
        self.persist_gemini_canvas_program_runtime_result_if_needed(payload, &result)
            .await;
        Ok(result)
    }

    async fn execute_gemini_canvas_remote_or_owned_browser_invocation(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        provider: &str,
        endpoint_kind: EndpointKind,
        invocation_input: Value,
        browser_pool_base_url: &str,
        base_url: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        program_config: Option<
            &crate::protocol::gemini::canvas_program_web_reverse::GeminiCanvasProgramRelayConfig,
        >,
        operation: &str,
        prompt: &str,
        locale: &str,
        timeout: Duration,
    ) -> Result<GeminiCanvasBrowserInvocationResult, GatewayError> {
        let skip_remote_browser_executor = payload.adapter
            == "gemini_canvas_program_web_reverse_compatible"
            && endpoint_kind == EndpointKind::VideosGenerations;

        if !skip_remote_browser_executor {
            if let Some(result) = self
                .execute_remote_browser_executor(
                    "gemini_canvas",
                    provider_account_id,
                    endpoint_kind,
                    invocation_input,
                )
                .await?
            {
                return gemini_canvas_web_reverse_modular::parse_remote_modular_media_browser_invocation_value(
                    provider,
                    result,
                    operation,
                );
            }
        }
        self.execute_gemini_canvas_owned_browser_invocation(
            payload,
            provider,
            browser_pool_base_url,
            base_url,
            runtime,
            program_config,
            operation,
            prompt,
            locale,
            timeout,
        )
        .await
    }

    async fn execute_gemini_canvas_modular_media_browser_result(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        provider: &str,
        endpoint_kind: EndpointKind,
        base_url: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        program_config: Option<
            &crate::protocol::gemini::canvas_program_web_reverse::GeminiCanvasProgramRelayConfig,
        >,
        operation: &str,
        prompt: &str,
        locale: &str,
        request_timeout: Duration,
    ) -> Result<GeminiCanvasBrowserInvocationResult, GatewayError> {
        let invocation_input = if payload.adapter == "gemini_canvas_program_web_reverse_compatible"
        {
            gemini_canvas_program_web_reverse_modular::build_browser_operation_invocation_input(
                payload,
                operation,
                prompt,
                locale,
                request_timeout,
            )?
        } else {
            gemini_canvas_web_reverse_modular::build_browser_operation_invocation_input(
                payload,
                operation,
                prompt,
                locale,
                request_timeout,
            )?
        };
        let browser_pool_base_url = self.ensure_gemini_canvas_browser_pool(provider).await?;
        self.execute_gemini_canvas_remote_or_owned_browser_invocation(
            provider_account_id,
            payload,
            provider,
            endpoint_kind,
            invocation_input,
            &browser_pool_base_url,
            base_url,
            runtime,
            program_config,
            operation,
            prompt,
            locale,
            request_timeout,
        )
        .await
    }

    async fn execute_gemini_canvas_browser_request_with_program_context(
        &self,
        provider: &str,
        browser_pool_base_url: &str,
        base_url: &str,
        config: &crate::protocol::gemini::canvas_program_web_reverse::GeminiCanvasProgramRelayConfig,
        operation: &str,
        prompt: &str,
        locale: &str,
        timeout: Duration,
    ) -> Result<GeminiCanvasBrowserInvocationResult, GatewayError> {
        let input = gemini_canvas_program_web_reverse_modular::build_browser_operation_invocation_input_from_config(
            base_url,
            config,
            operation,
            prompt,
            locale,
            timeout,
        );
        let response = self
            .http
            .request(Method::POST, format!("{}/invoke", browser_pool_base_url))
            .header(rquest::header::CONTENT_TYPE, "application/json")
            .timeout(timeout.max(Duration::from_secs(30)))
            .json(&input)
            .send()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;

        let status = response.status().as_u16();
        let body_text = response
            .text()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        gemini_canvas_program_web_reverse_modular::parse_program_browser_invocation_response(
            provider, status, &body_text, operation,
        )
    }

    async fn execute_gemini_canvas_browser_request_with_program_context_recovery(
        &self,
        provider: &str,
        browser_pool_base_url: &str,
        base_url: &str,
        config: &crate::protocol::gemini::canvas_program_web_reverse::GeminiCanvasProgramRelayConfig,
        operation: &str,
        prompt: &str,
        locale: &str,
        timeout: Duration,
    ) -> Result<GeminiCanvasBrowserInvocationResult, GatewayError> {
        let mut attempts = 0usize;
        loop {
            match self
                .execute_gemini_canvas_browser_request_with_program_context(
                    provider,
                    browser_pool_base_url,
                    base_url,
                    config,
                    operation,
                    prompt,
                    locale,
                    timeout,
                )
                .await
            {
                Ok(result) => return Ok(result),
                Err(error) => {
                    let code = error.code.as_deref();
                    let explicit_quota_gate =
                        matches!(code, Some("gemini_canvas_video_quota_reached"));
                    let retryable = matches!(
                        code,
                        Some("gemini_canvas_context_busy")
                            | Some("gemini_canvas_auth_required")
                            | Some("gemini_canvas_browser_worker_failed")
                            | Some("gemini_canvas_program_auth_redirect")
                    ) || (error.http_status == Some(429) && !explicit_quota_gate);
                    if !retryable || attempts >= 2 {
                        return Err(error);
                    }
                    let delay_ms = match code {
                        Some("gemini_canvas_auth_required")
                        | Some("gemini_canvas_program_auth_redirect") => 2_000,
                        Some("gemini_canvas_context_busy") => 3_000 + (attempts as u64 * 1_500),
                        _ => 2_500,
                    };
                    attempts += 1;
                    sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }

    async fn execute_gemini_canvas_program_bootstrap_request(
        &self,
        provider: &str,
        browser_pool_base_url: &str,
        payload: &ProviderAccountPayload,
        bootstrap_operation: &str,
        bootstrap_prompt: &str,
        locale: &str,
        timeout: Duration,
    ) -> Result<GeminiCanvasBrowserInvocationResult, GatewayError> {
        let input =
            gemini_canvas_program_web_reverse_modular::build_program_bootstrap_invocation_input(
                payload,
                bootstrap_operation,
                bootstrap_prompt,
                locale,
                timeout,
            )?;
        let response = self
            .http
            .request(Method::POST, format!("{}/invoke", browser_pool_base_url))
            .header(rquest::header::CONTENT_TYPE, "application/json")
            .timeout(timeout.max(Duration::from_secs(30)))
            .json(&input)
            .send()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;

        let status = response.status().as_u16();
        let body_text = response
            .text()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        gemini_canvas_program_web_reverse_modular::parse_program_bootstrap_invocation_response(
            provider, status, &body_text,
        )
    }

    async fn try_ensure_gemini_canvas_program_payload_handle_pure_http(
        &self,
        payload: &ProviderAccountPayload,
        bootstrap_operation: &str,
        locale: &str,
        timeout: Duration,
    ) -> Result<ProviderAccountPayload, GatewayError> {
        let provider = "gemini_canvas_program_web_reverse_compatible";
        let runtime = gemini_canvas::runtime_from_payload(payload)?;
        let config = gemini_canvas_program_web_reverse_modular::relay_config_from_payload(payload)?;
        let base_url = payload.base_url.trim_end_matches('/');
        let share_url = format!("{base_url}/share/{}", config.bootstrap.share_id);
        let storage_state = gateway_object_storage()?
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
        let mut session = gemini_canvas::storage_state_to_pure_http_session(
            &storage_state,
            &share_url,
            base_url,
            &auth_user,
        )?;

        let mut share_headers = HeaderMap::new();
        apply_gemini_canvas_navigation_headers(&mut share_headers);
        apply_gemini_canvas_cookie_header(&mut share_headers, &session);
        insert_header_map_value(&mut share_headers, "accept-language", locale);

        let share_response = self
            .http
            .request(Method::GET, &share_url)
            .headers(share_headers)
            .timeout(timeout.max(Duration::from_secs(30)))
            .send()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        apply_gemini_canvas_response_cookies(share_response.headers(), &mut session);
        let share_status = share_response.status().as_u16();
        let share_content_type = share_response
            .headers()
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let share_body = share_response
            .text()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        if !(200..300).contains(&share_status)
            || gemini_web::response_indicates_browser_challenge(
                share_status,
                share_content_type.as_deref(),
                &share_body,
            )
            || gemini_web::response_indicates_session_invalid(
                share_status,
                share_content_type.as_deref(),
                &share_body,
            )
        {
            return Err(classify_gemini_canvas_pure_http_error(
                share_status,
                share_content_type.as_deref(),
                &share_body,
            ));
        }

        let fallback_bootstrap =
            gemini_web::bootstrap_from_payload_cache(payload.extra_body.as_ref());
        let bootstrap = gemini_web::parse_bootstrap_from_app_html(
            &share_body,
            payload
                .extra_body
                .as_ref()
                .and_then(|extra| extra.get("language").and_then(Value::as_str)),
        )
        .or_else(|primary_error| fallback_bootstrap.clone().ok_or(primary_error))?;
        let bootstrap =
            gemini_web::merge_bootstrap_from_fallback(bootstrap, fallback_bootstrap.as_ref());
        let mut create_request =
            gemini_canvas_program_web_reverse_modular::build_program_create_pure_http_request(
                &config.bootstrap.share_id,
                &bootstrap,
                gemini_canvas_program_web_reverse_modular::current_program_create_reqid(),
            )?;
        let request_url = format!("{base_url}/_/BardChatUi/data/batchexecute");
        let origin = url::Url::parse(base_url)
            .map(|parsed| parsed.origin().ascii_serialization())
            .unwrap_or_else(|_| base_url.to_string());
        let authorization = gemini_canvas::build_sapisid_authorization(
            &session.sapisid,
            &origin,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        )?;
        let mut xsrf_retry_token: Option<String> = None;

        loop {
            let mut headers = HeaderMap::new();
            insert_header_map_value(&mut headers, "accept", "*/*");
            insert_header_map_value(
                &mut headers,
                "content-type",
                "application/x-www-form-urlencoded;charset=UTF-8",
            );
            insert_header_map_value(&mut headers, "accept-language", &bootstrap.language);
            insert_header_map_value(&mut headers, "origin", &origin);
            insert_header_map_value(&mut headers, "referer", &share_url);
            insert_header_map_value(
                &mut headers,
                "user-agent",
                gemini_web::GEMINI_WEB_DEFAULT_USER_AGENT,
            );
            insert_header_map_value(
                &mut headers,
                "x-same-domain",
                gemini_web::GEMINI_WEB_DEFAULT_SAME_DOMAIN_HEADER,
            );
            apply_browser_fetch_client_hints(&mut headers, &request_url, base_url);
            apply_gemini_canvas_signed_headers(
                &mut headers,
                &session,
                &origin,
                &share_url,
                &authorization,
                true,
            );

            let response = self
                .http
                .request(Method::POST, &request_url)
                .headers(headers)
                .query(&create_request.query)
                .timeout(timeout.max(Duration::from_secs(30)))
                .form(&create_request.form)
                .send()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            apply_gemini_canvas_response_cookies(response.headers(), &mut session);
            let status = response.status().as_u16();
            let content_type = response
                .headers()
                .get(rquest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let body_text = response
                .text()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            if (200..300).contains(&status)
                && !gemini_web::response_indicates_browser_challenge(
                    status,
                    content_type.as_deref(),
                    &body_text,
                )
                && !gemini_web::response_indicates_session_invalid(
                    status,
                    content_type.as_deref(),
                    &body_text,
                )
            {
                let patch = gemini_canvas_program_web_reverse_modular::runtime_patch_from_pure_http_create_response(
                    &config.bootstrap.share_id,
                    Some(bootstrap_operation),
                    &body_text,
                    base_url,
                )
                .ok_or_else(|| {
                    gemini_canvas_program_web_reverse_modular::gemini_canvas_program_create_missing_handle_patch_error(
                        provider,
                    )
                })?;
                let ensured_payload = merge_extra_body_patch_into_payload(payload, &patch);
                persist_gemini_canvas_program_runtime_material(
                    self.redis_pool.as_ref(),
                    self.pg_pool.as_ref(),
                    payload,
                    Some(patch),
                )
                .await;
                return Ok(ensured_payload);
            }

            if status == 400 {
                if let Some(token) = extract_gemini_canvas_batchexecute_xsrf_token(&body_text) {
                    let current_at = header_map_string_from_form(&create_request.form, "at");
                    if xsrf_retry_token.as_deref() != Some(token.as_str())
                        && current_at.as_deref() != Some(token.as_str())
                    {
                        debug!(
                            provider,
                            xsrf_token_preview = %truncate_response_preview(&token, 24),
                            "retrying Gemini Canvas pure HTTP program create with xsrf token extracted from upstream error"
                        );
                        upsert_form_field(&mut create_request.form, "at", &token);
                        xsrf_retry_token = Some(token);
                        continue;
                    }
                }
            }

            return Err(classify_gemini_canvas_pure_http_error(
                status,
                content_type.as_deref(),
                &body_text,
            ));
        }
    }

    async fn ensure_gemini_canvas_program_payload_handle(
        &self,
        payload: &ProviderAccountPayload,
        operation: &str,
        locale: &str,
        timeout: Duration,
    ) -> Result<ProviderAccountPayload, GatewayError> {
        if payload.adapter != "gemini_canvas_program_web_reverse_compatible" {
            return Ok(payload.clone());
        }

        let provider = payload.adapter.as_str();
        let bootstrap_operation =
            gemini_canvas_program_web_reverse_modular::normalize_gemini_canvas_program_bootstrap_operation(
                operation,
            );
        let mut ensured_payload = payload.clone();
        if !gemini_canvas_program_web_reverse_modular::gemini_canvas_program_payload_has_concrete_handle(
            &ensured_payload,
        ) {
            match self
                .try_ensure_gemini_canvas_program_payload_handle_pure_http(
                    &ensured_payload,
                    bootstrap_operation,
                    locale,
                    timeout,
                )
                .await
            {
                Ok(next_payload) => ensured_payload = next_payload,
                Err(error) => {
                    debug!(
                        provider,
                        code = ?error.code,
                        http_status = ?error.http_status,
                        "Gemini Canvas pure HTTP program create did not produce a concrete handle; falling back to browser discovery"
                    );
                }
            }
        }
        if gemini_canvas_program_web_reverse_modular::gemini_canvas_program_payload_handle_matches_operation(
            &ensured_payload,
            bootstrap_operation,
        ) {
            return Ok(ensured_payload);
        }
        let browser_pool_base_url = self.ensure_gemini_canvas_browser_pool(provider).await?;
        let bootstrap_prompt =
            gemini_canvas_program_web_reverse_modular::default_gemini_canvas_program_bootstrap_prompt(
                bootstrap_operation,
            );
        let bootstrap_payload = if gemini_canvas_program_web_reverse_modular::gemini_canvas_program_payload_has_concrete_handle(
            &ensured_payload,
        ) {
            ensured_payload.clone()
        } else {
            gemini_canvas_program_web_reverse_modular::strip_gemini_canvas_program_handle_hints_from_payload(
                &ensured_payload,
            )
        };
        let invocation = self
            .execute_gemini_canvas_program_bootstrap_request(
                provider,
                &browser_pool_base_url,
                &bootstrap_payload,
                bootstrap_operation,
                &bootstrap_prompt,
                locale,
                timeout,
            )
            .await?;
        let patch = gemini_canvas_program_web_reverse_modular::runtime_patch_from_browser_invocation(
            &invocation,
        )
        .ok_or_else(|| {
                gemini_canvas_program_web_reverse_modular::gemini_canvas_program_bootstrap_missing_handle_patch_error(
                    provider,
                )
            })?;
        let ensured_payload = merge_extra_body_patch_into_payload(&ensured_payload, &patch);
        if !gemini_canvas_program_web_reverse_modular::gemini_canvas_program_payload_has_concrete_handle(
            &ensured_payload,
        ) {
            return Err(
                gemini_canvas_program_web_reverse_modular::gemini_canvas_program_bootstrap_incomplete_error(
                    provider,
                ),
            );
        }
        persist_gemini_canvas_program_runtime_material(
            self.redis_pool.as_ref(),
            self.pg_pool.as_ref(),
            &ensured_payload,
            Some(patch),
        )
        .await;
        Ok(ensured_payload)
    }

    // ── streaming execute ─────────────────────────────────────────────────

    /// Initiate a streaming upstream request and return the raw [`rquest::Response`].
    ///
    /// The caller is responsible for reading and parsing the SSE byte stream.
    /// The response is returned before the body is consumed so the caller can
    /// wrap it in a [`TrackedStream`].
    ///
    /// `extra_headers` are merged into the upstream request headers (lower
    /// priority than provider config).  Pass `None` when no additional headers
    /// are needed.
    pub async fn execute_stream(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<UpstreamStreamingResponse, GatewayError> {
        self.execute_stream_with_provider_account_id("", payload, req, model, extra_headers)
            .await
    }

    pub async fn execute_stream_with_provider_account_id(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<UpstreamStreamingResponse, GatewayError> {
        if payload.canonical_adapter() == "xfyun_websocket_compatible" {
            let _ = extra_headers;
            debug!(
                model,
                "sending upstream streaming request (xfyun websocket native)"
            );
            return xfyun_websocket::execute_stream_as_openai_sse(payload, req, model)
                .await
                .map(UpstreamStreamingResponse::Bytes);
        }

        if payload.adapter == "qwen_web_compatible" {
            debug!(
                model,
                "sending upstream streaming request (qwen web: create chat + translate)"
            );
            return self
                .execute_qwen_web_stream(payload, req, model, extra_headers)
                .await
                .map(UpstreamStreamingResponse::Bytes);
        }

        if payload.adapter == "chatgpt_web_reverse_compatible" {
            debug!(
                model,
                "sending upstream streaming request (chatgpt web reverse: bootstrap + sentinel + conversation -> fake openai sse)"
            );
            return chatgpt_upstream::execute_stream(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
                extra_headers,
            )
            .await;
        }

        if aistudio_web_reverse_modular::is_aistudio_web_reverse_adapter(&payload.adapter) {
            debug!(
                model,
                "sending upstream streaming request (aistudio web reverse: browser-owned generateContent -> fake openai sse)"
            );
            return self
                .execute_aistudio_web_stream(
                    provider_account_id,
                    payload,
                    req,
                    model,
                    extra_headers,
                )
                .await
                .map(UpstreamStreamingResponse::Bytes);
        }

        if matches!(payload.adapter.as_str(), "gemini_web_compatible") {
            debug!(
                model,
                "sending upstream streaming request (gemini web: bootstrap app + StreamGenerate -> fake openai sse)"
            );
            return gemini_web_reverse_modular::execute_stream(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
                extra_headers,
            )
            .await
            .map(UpstreamStreamingResponse::Bytes);
        }
        if let Some(legacy_route) = gemini_web_reverse_modular::legacy_mixed_lane_execution_route(
            payload,
            req.endpoint_kind,
            true,
        ) {
            if legacy_route.kind
                == gemini_web_reverse_modular::LegacyMixedLaneExecutionKind::TextStream
            {
                debug!(
                    model,
                    endpoint = ?req.endpoint_kind,
                    "sending upstream streaming request (gemini web reverse modular: legacy mixed-lane text -> fake openai sse)"
                );
                return gemini_web_reverse_modular::execute_legacy_text_stream(
                    self,
                    &legacy_route.payload,
                    req,
                    model,
                    extra_headers,
                )
                .await
                .map(UpstreamStreamingResponse::Bytes);
            }
        }

        if payload.adapter == "gemini_canvas_compatible"
            && matches!(
                req.endpoint_kind,
                EndpointKind::ChatCompletions
                    | EndpointKind::Messages
                    | EndpointKind::Responses
                    | EndpointKind::Completions
            )
        {
            debug!(
                model,
                endpoint = ?req.endpoint_kind,
                "sending upstream streaming request (gemini canvas: browser-backed reverse-web text -> fake openai sse)"
            );
            return self
                .execute_gemini_canvas_text_stream(payload, req, model, extra_headers)
                .await
                .map(UpstreamStreamingResponse::Bytes);
        }
        if payload.adapter == "gemini_canvas_web_reverse_compatible"
            && matches!(
                req.endpoint_kind,
                EndpointKind::ChatCompletions
                    | EndpointKind::Messages
                    | EndpointKind::Responses
                    | EndpointKind::Completions
            )
        {
            let browser_owned_payload =
                gemini_canvas_web_reverse_modular::force_browser_owned_payload(payload);
            debug!(
                model,
                endpoint = ?req.endpoint_kind,
                "sending upstream streaming request (gemini canvas browser relay modular: connected-canvas text -> fake openai sse)"
            );
            return self
                .execute_gemini_canvas_modular_browser_relay_text_stream(
                    &browser_owned_payload,
                    req,
                    model,
                    extra_headers,
                )
                .await
                .map(UpstreamStreamingResponse::Bytes);
        }
        if payload.adapter == "gemini_canvas_program_web_reverse_compatible"
            && matches!(
                req.endpoint_kind,
                EndpointKind::ChatCompletions
                    | EndpointKind::Messages
                    | EndpointKind::Responses
                    | EndpointKind::Completions
            )
        {
            let program_owned_payload =
                gemini_canvas_program_web_reverse_modular::force_program_owned_payload(payload);
            debug!(
                model,
                endpoint = ?req.endpoint_kind,
                "sending upstream streaming request (gemini canvas program modular: concrete-program text -> fake openai sse)"
            );
            return self
                .execute_gemini_canvas_modular_browser_relay_text_stream(
                    &program_owned_payload,
                    req,
                    model,
                    extra_headers,
                )
                .await
                .map(UpstreamStreamingResponse::Bytes);
        }

        if gemini_api_modular::supports_fake_openai_sse_bridge(
            payload.adapter.as_str(),
            req.endpoint_kind,
        ) {
            let canonical = self
                .execute_with_provider_account_id(
                    provider_account_id,
                    payload,
                    req,
                    model,
                    extra_headers,
                )
                .await?;
            return Ok(gemini_api_modular::build_fake_openai_sse_bridge(
                req, model, &canonical,
            ));
        }

        if chatgpt_official_api_modular::owns_payload(payload) {
            return self
                .execute_chatgpt_official_streaming(payload, req, model, extra_headers)
                .await;
        }

        let (plan, headers, provider) = (
            Self::build_request_plan(payload, req, model, true)?,
            build_upstream_headers_with(payload, extra_headers),
            payload.adapter.clone(),
        );

        debug!(url = %plan.url, model, "sending upstream streaming request");

        let response = self
            .send_plan(&plan, headers)
            .send()
            .await
            .map_err(|e| classify_network_error(&e, Some(provider.as_str())))?;

        let status = response.status().as_u16();

        if !response.status().is_success() {
            let body_text = response
                .text()
                .await
                .unwrap_or_else(|_| String::from("<unreadable body>"));
            return Err(classify_upstream_error(
                status,
                &body_text,
                Some(provider.as_str()),
            ));
        }

        Ok(UpstreamStreamingResponse::Http(response))
    }

    // ── helpers ───────────────────────────────────────────────────────────

    async fn execute_qwen_web(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<CanonicalRelayResponse, GatewayError> {
        qwen_web_reverse_modular::execute(self, payload, req, model, extra_headers).await
    }

    async fn execute_qwen_web_stream(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<
        std::pin::Pin<Box<dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>>,
        GatewayError,
    > {
        qwen_web_reverse_modular::execute_stream(self, payload, req, model, extra_headers).await
    }

    pub(crate) fn send_plan(
        &self,
        plan: &RequestPlan,
        headers: rquest::header::HeaderMap,
    ) -> RequestBuilder {
        build_request_builder_from_plan(&self.http, self.timeout, plan, headers)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
struct GeminiCanvasTextDirectHttpFallbackAttempt<'a> {
    label: &'static str,
    api_key_override: Option<&'a str>,
    referer_override: Option<&'a str>,
    preserve_cross_origin_referer: bool,
    include_signed_headers: bool,
}

fn build_gemini_canvas_text_direct_http_fallback_attempts<'a>(
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

fn should_fallback_gemini_canvas_image_to_browser(error: &GatewayError) -> bool {
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
struct GeminiCanvasVideoContinuation {
    conversation_id: String,
    response_id: String,
    app_path: String,
    job_id: Option<String>,
}

fn apply_gemini_canvas_browser_video_continuation(
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

fn build_gemini_canvas_video_continuation_seed_body(
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

fn gemini_canvas_video_continuation_from_request(
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

fn should_preserve_gemini_canvas_video_continuation_as_pending(error: &GatewayError) -> bool {
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

fn gemini_canvas_stream_collection_policy(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{CanonicalMessage, ContentPart, MessageRole, ProtocolFamily};
    use serde_json::json;
    use std::collections::HashMap;

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

    fn make_gemini_canvas_runtime_api_context(
        payload: ProviderAccountPayload,
        api_key_candidates: &[&str],
        page_referer: &str,
    ) -> GeminiCanvasRuntimeApiContext {
        GeminiCanvasRuntimeApiContext {
            payload,
            api_key_candidates: api_key_candidates
                .iter()
                .map(|candidate| (*candidate).to_string())
                .collect(),
            session: gemini_canvas::GeminiCanvasPureHttpSession {
                cookie_header: "SAPISID=session-cookie".to_string(),
                sapisid: "session-cookie".to_string(),
                auth_user: "0".to_string(),
            },
            page_origin: "https://gemini.google.com".to_string(),
            page_referer: page_referer.to_string(),
        }
    }

    fn unused_loopback_base_url() -> String {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("bind unused loopback port");
        let addr = listener.local_addr().expect("read loopback listener addr");
        drop(listener);
        format!("http://{addr}")
    }

    fn single_response_executor_base_url(
        status: u16,
        status_text: &'static str,
        body: &'static str,
    ) -> (String, std::thread::JoinHandle<()>) {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("bind executor response server");
        let addr = listener
            .local_addr()
            .expect("read executor response server addr");
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept executor request");
            let mut buffer = [0_u8; 4096];
            let _ = std::io::Read::read(&mut stream, &mut buffer);
            let response = format!(
                "HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            std::io::Write::write_all(&mut stream, response.as_bytes())
                .expect("write executor response");
        });
        (format!("http://{addr}"), handle)
    }

    #[test]
    fn gemini_canvas_text_direct_http_fallback_prefers_harvested_runtime_api_keys() {
        let mut payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        payload.api_key.clear();
        let mut runtime_payload = payload.clone();
        runtime_payload.api_key = "AIzaHarvestedKeyOne".to_string();
        let runtime_api = make_gemini_canvas_runtime_api_context(
            runtime_payload,
            &[
                "AIzaHarvestedKeyOne",
                "AIzaHarvestedKeyTwo",
                "AIzaHarvestedKeyOne",
            ],
            "https://gemini.google.com/share/demo",
        );

        let attempts =
            build_gemini_canvas_text_direct_http_fallback_attempts(&payload, Some(&runtime_api));

        assert_eq!(attempts.len(), 3);
        assert_eq!(attempts[0].label, "runtime_api_harvested_key");
        assert_eq!(attempts[0].api_key_override, Some("AIzaHarvestedKeyOne"));
        assert_eq!(
            attempts[0].referer_override,
            Some("https://gemini.google.com/share/demo")
        );
        assert!(attempts[0].preserve_cross_origin_referer);
        assert!(!attempts[0].include_signed_headers);
        assert_eq!(attempts[1].api_key_override, Some("AIzaHarvestedKeyTwo"));
        assert_eq!(attempts[2].label, "legacy_payload_direct_http");
        assert_eq!(attempts[2].api_key_override, None);
        assert!(!attempts[2].preserve_cross_origin_referer);
        assert!(attempts[2].include_signed_headers);
    }

    #[test]
    fn gemini_canvas_text_direct_http_fallback_keeps_legacy_only_when_payload_has_api_key() {
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let runtime_api = make_gemini_canvas_runtime_api_context(
            payload.clone(),
            &["AIzaHarvestedKeyOne"],
            "https://gemini.google.com/share/demo",
        );

        let attempts =
            build_gemini_canvas_text_direct_http_fallback_attempts(&payload, Some(&runtime_api));

        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].label, "legacy_payload_direct_http");
        assert_eq!(attempts[0].api_key_override, None);
        assert!(attempts[0].include_signed_headers);
    }

    #[test]
    fn gemini_canvas_image_browser_fallback_treats_auth_and_session_errors_as_retryable() {
        let auth_required =
            GatewayError::unauthorized("auth required").with_code("gemini_canvas_auth_required");
        assert!(should_fallback_gemini_canvas_image_to_browser(
            &auth_required
        ));

        let session_invalid = GatewayError::bad_request("session invalid")
            .with_code("gemini_canvas_pure_http_session_invalid");
        assert!(should_fallback_gemini_canvas_image_to_browser(
            &session_invalid
        ));

        let followup_missing = GatewayError::service_unavailable("missing asset")
            .with_code("gemini_canvas_media_followup_missing_asset");
        assert!(should_fallback_gemini_canvas_image_to_browser(
            &followup_missing
        ));

        let page_missing = GatewayError::server_error("page asset missing")
            .with_code("gemini_canvas_page_missing_image_asset");
        assert!(should_fallback_gemini_canvas_image_to_browser(
            &page_missing
        ));

        let gateway_timeout = GatewayError::service_unavailable("timed out");
        let gateway_timeout = GatewayError {
            http_status: Some(504),
            ..gateway_timeout
        };
        assert!(should_fallback_gemini_canvas_image_to_browser(
            &gateway_timeout
        ));

        let unsupported =
            GatewayError::bad_request("unsupported").with_code("unsupported_image_count");
        assert!(!should_fallback_gemini_canvas_image_to_browser(
            &unsupported
        ));
    }

    #[test]
    fn gemini_canvas_stream_collection_hands_video_off_at_locator() {
        let (music, music_handoff) = gemini_canvas_stream_collection_policy(
            gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_MUSIC_MODE_INDEX,
            false,
        );
        assert_eq!(music, gemini_canvas::GeminiCanvasMediaOperation::Music);
        assert!(!music_handoff);

        let (video, video_handoff) = gemini_canvas_stream_collection_policy(
            gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_VIDEO_MODE_INDEX,
            false,
        );
        assert_eq!(video, gemini_canvas::GeminiCanvasMediaOperation::Video);
        assert!(video_handoff);
    }

    #[test]
    fn gemini_canvas_stream_collection_keeps_image_generation_open_until_asset() {
        let (image, image_handoff) = gemini_canvas_stream_collection_policy(
            gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
            false,
        );
        assert_eq!(image, gemini_canvas::GeminiCanvasMediaOperation::Image);
        assert!(!image_handoff);

        let (image_edit, image_edit_handoff) = gemini_canvas_stream_collection_policy(
            gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
            true,
        );
        assert_eq!(image_edit, gemini_canvas::GeminiCanvasMediaOperation::Image);
        assert!(image_edit_handoff);
    }

    #[test]
    fn gemini_canvas_video_continuation_reads_completed_locator_fields() {
        let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
        req.raw_body = json!({
            "prompt": "video",
            "conversation_id": "c_065d5bde601ff3af",
            "response_id": "r_f7c7e549d11c849b",
            "app_path": "/app/065d5bde601ff3af",
            "job_id": "5cb5f42d-d5a9-4643-99fe-0123456789ab",
        });

        let continuation = gemini_canvas_video_continuation_from_request(&req)
            .expect("valid continuation")
            .expect("continuation fields");

        assert_eq!(continuation.conversation_id, "c_065d5bde601ff3af");
        assert_eq!(continuation.response_id, "r_f7c7e549d11c849b");
        assert_eq!(continuation.app_path, "/app/065d5bde601ff3af");
        assert_eq!(
            continuation.job_id.as_deref(),
            Some("5cb5f42d-d5a9-4643-99fe-0123456789ab")
        );
        let seed_body = build_gemini_canvas_video_continuation_seed_body(&continuation);
        assert_eq!(
            gemini_canvas::extract_video_generation_job_id(&seed_body).as_deref(),
            Some("5cb5f42d-d5a9-4643-99fe-0123456789ab")
        );
        assert!(gemini_canvas::response_indicates_video_generation_pending(
            &seed_body
        ));

        let mut browser_input = json!({ "operation": "video" });
        apply_gemini_canvas_browser_video_continuation(&mut browser_input, &continuation);
        assert_eq!(browser_input["resumeExistingMedia"], true);
        assert_eq!(browser_input["conversationId"], "c_065d5bde601ff3af");
        assert_eq!(browser_input["responseId"], "r_f7c7e549d11c849b");
        assert_eq!(browser_input["appPath"], "/app/065d5bde601ff3af");
        assert_eq!(
            browser_input["jobId"],
            "5cb5f42d-d5a9-4643-99fe-0123456789ab"
        );
    }

    #[test]
    fn gemini_canvas_video_continuation_rejects_partial_or_unsafe_locator_fields() {
        let mut partial = make_request(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
        partial.raw_body = json!({
            "prompt": "video",
            "conversation_id": "c_065d5bde601ff3af",
        });
        let partial_error = gemini_canvas_video_continuation_from_request(&partial)
            .expect_err("partial continuation must fail");
        assert_eq!(partial_error.http_status, Some(400));
        assert_eq!(
            partial_error.code.as_deref(),
            Some("invalid_gemini_canvas_video_continuation")
        );

        let mut unsafe_path = make_request(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
        unsafe_path.raw_body = json!({
            "prompt": "video",
            "conversation_id": "c_065d5bde601ff3af",
            "response_id": "r_f7c7e549d11c849b",
            "app_path": "/app/065d5bde601ff3af?redirect=unsafe",
        });
        let path_error = gemini_canvas_video_continuation_from_request(&unsafe_path)
            .expect_err("unsafe continuation path must fail");
        assert_eq!(path_error.http_status, Some(400));
        assert_eq!(
            path_error.code.as_deref(),
            Some("invalid_gemini_canvas_video_continuation")
        );
    }

    // ── build_request_plan ────────────────────────────────────────────────

    #[tokio::test]
    async fn remote_only_browser_executor_without_base_url_returns_required_unavailable_error() {
        let mut client = UpstreamClient::new(5);
        client.browser_executor_base_url = None;
        client.request_time_browser_policy = RequestTimeBrowserPolicy::RemoteOnly;

        let err = client
            .execute_remote_browser_executor(
                "gemini_canvas",
                "provider-account-1",
                EndpointKind::ImagesGenerations,
                json!({"prompt": "image"}),
            )
            .await
            .expect_err("remote_only policy must fail closed instead of returning Ok(None)");

        assert_eq!(
            err.code.as_deref(),
            Some("browser_executor_required_unavailable")
        );
        assert_eq!(err.http_status, Some(503));
    }

    #[test]
    fn remote_browser_executor_timeout_covers_the_worker_budget() {
        assert_eq!(
            browser_executor_remote_request_timeout(
                Duration::from_secs(120),
                &json!({"timeoutMs": 300_000}),
            ),
            Duration::from_secs(320),
        );
        assert_eq!(
            browser_executor_remote_request_timeout(Duration::from_secs(120), &json!({})),
            Duration::from_secs(140),
        );
    }

    #[tokio::test]
    async fn disabled_browser_executor_without_base_url_returns_forbidden_error() {
        let mut client = UpstreamClient::new(5);
        client.browser_executor_base_url = None;
        client.request_time_browser_policy = RequestTimeBrowserPolicy::Disabled;

        let err = client
            .execute_remote_browser_executor(
                "gemini_canvas",
                "provider-account-1",
                EndpointKind::ImagesGenerations,
                json!({"prompt": "image"}),
            )
            .await
            .expect_err("disabled policy must fail closed instead of allowing local fallback");

        assert_eq!(err.code.as_deref(), Some("request_time_browser_forbidden"));
        assert_eq!(err.http_status, Some(503));
    }

    #[tokio::test]
    async fn remote_only_browser_executor_unreachable_base_url_returns_required_unavailable_error()
    {
        let mut client = UpstreamClient::new(5);
        client.browser_executor_base_url = Some(unused_loopback_base_url());
        client.request_time_browser_policy = RequestTimeBrowserPolicy::RemoteOnly;

        let err = client
            .execute_remote_browser_executor(
                "gemini_canvas",
                "provider-account-1",
                EndpointKind::ImagesGenerations,
                json!({"prompt": "image"}),
            )
            .await
            .expect_err("unreachable remote_only executor must fail closed");

        assert_eq!(
            err.code.as_deref(),
            Some("browser_executor_required_unavailable")
        );
        assert_eq!(err.http_status, Some(503));
    }

    #[tokio::test]
    async fn disabled_browser_executor_unreachable_base_url_returns_required_unavailable_error() {
        let mut client = UpstreamClient::new(5);
        client.browser_executor_base_url = Some(unused_loopback_base_url());
        client.request_time_browser_policy = RequestTimeBrowserPolicy::Disabled;

        let err = client
            .execute_remote_browser_executor(
                "gemini_canvas",
                "provider-account-1",
                EndpointKind::ImagesGenerations,
                json!({"prompt": "image"}),
            )
            .await
            .expect_err("disabled policy must not allow local fallback when remote is unreachable");

        assert_eq!(
            err.code.as_deref(),
            Some("browser_executor_required_unavailable")
        );
        assert_eq!(err.http_status, Some(503));
    }

    #[tokio::test]
    async fn remote_only_browser_executor_non_success_status_returns_required_failed_error() {
        let (base_url, server) = single_response_executor_base_url(
            503,
            "Service Unavailable",
            r#"{"ok":false,"error":{"code":"executor_down","message":"down"}}"#,
        );
        let mut client = UpstreamClient::new(5);
        client.browser_executor_base_url = Some(base_url);
        client.request_time_browser_policy = RequestTimeBrowserPolicy::RemoteOnly;

        let err = client
            .execute_remote_browser_executor(
                "gemini_canvas",
                "provider-account-1",
                EndpointKind::ImagesGenerations,
                json!({"prompt": "image"}),
            )
            .await
            .expect_err("remote_only executor non-success status must fail closed");
        server.join().expect("executor response server exits");

        assert_eq!(
            err.code.as_deref(),
            Some("browser_executor_required_failed")
        );
        assert_eq!(err.http_status, Some(503));
    }

    #[tokio::test]
    async fn disabled_browser_executor_non_success_status_returns_required_failed_error() {
        let (base_url, server) = single_response_executor_base_url(
            503,
            "Service Unavailable",
            r#"{"ok":false,"error":{"code":"executor_down","message":"down"}}"#,
        );
        let mut client = UpstreamClient::new(5);
        client.browser_executor_base_url = Some(base_url);
        client.request_time_browser_policy = RequestTimeBrowserPolicy::Disabled;

        let err = client
            .execute_remote_browser_executor(
                "gemini_canvas",
                "provider-account-1",
                EndpointKind::ImagesGenerations,
                json!({"prompt": "image"}),
            )
            .await
            .expect_err("disabled policy must not allow local fallback on remote non-success");
        server.join().expect("executor response server exits");

        assert_eq!(
            err.code.as_deref(),
            Some("browser_executor_required_failed")
        );
        assert_eq!(err.http_status, Some(503));
    }

    #[tokio::test]
    async fn remote_only_browser_executor_invalid_json_returns_required_failed_error() {
        let (base_url, server) = single_response_executor_base_url(200, "OK", "not-json");
        let mut client = UpstreamClient::new(5);
        client.browser_executor_base_url = Some(base_url);
        client.request_time_browser_policy = RequestTimeBrowserPolicy::RemoteOnly;

        let err = client
            .execute_remote_browser_executor(
                "gemini_canvas",
                "provider-account-1",
                EndpointKind::ImagesGenerations,
                json!({"prompt": "image"}),
            )
            .await
            .expect_err("remote_only executor invalid JSON must fail closed");
        server.join().expect("executor response server exits");

        assert_eq!(
            err.code.as_deref(),
            Some("browser_executor_required_failed")
        );
        assert_eq!(err.http_status, Some(503));
    }

    #[tokio::test]
    async fn disabled_browser_executor_invalid_json_returns_required_failed_error() {
        let (base_url, server) = single_response_executor_base_url(200, "OK", "not-json");
        let mut client = UpstreamClient::new(5);
        client.browser_executor_base_url = Some(base_url);
        client.request_time_browser_policy = RequestTimeBrowserPolicy::Disabled;

        let err = client
            .execute_remote_browser_executor(
                "gemini_canvas",
                "provider-account-1",
                EndpointKind::ImagesGenerations,
                json!({"prompt": "image"}),
            )
            .await
            .expect_err("disabled policy must not allow local fallback on remote invalid JSON");
        server.join().expect("executor response server exits");

        assert_eq!(
            err.code.as_deref(),
            Some("browser_executor_required_failed")
        );
        assert_eq!(err.http_status, Some(503));
    }

    #[cfg(feature = "line-suno-web-reverse-api")]
    #[tokio::test]
    async fn execute_suno_image_inputs_rejected_before_send() {
        let payload = make_payload("suno_compatible", "https://studio-api-prod.suno.com");
        let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
        req.requested_model = Some("chirp-v3-5".to_string());
        req.raw_body = json!({
            "prompt": "cover art",
            "image": "data:image/png;base64,aGVsbG8=",
        });
        let client = UpstreamClient::new(5);
        let err = client
            .execute_suno_media(&payload, &req, "chirp-v3-5", None)
            .await
            .expect_err("uploaded image inputs should be rejected before send");
        assert_eq!(err.http_status, Some(400));
        assert_eq!(err.code.as_deref(), Some("unsupported_suno_image_inputs"));
    }

    #[cfg(feature = "line-suno-web-reverse-api")]
    #[tokio::test]
    async fn execute_suno_video_multiple_outputs_rejected_before_send() {
        let payload = make_payload("suno_compatible", "https://studio-api-prod.suno.com");
        let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
        req.requested_model = Some("chirp-v3-5".to_string());
        req.raw_body = json!({
            "prompt": "cinematic stage clip",
            "n": 2,
        });
        let client = UpstreamClient::new(5);
        let err = client
            .execute_suno_media(&payload, &req, "chirp-v3-5", None)
            .await
            .expect_err("multi-video requests should be rejected before send");
        assert_eq!(err.http_status, Some(400));
        assert_eq!(err.code.as_deref(), Some("unsupported_suno_video_count"));
    }

    // ── resolve_model ─────────────────────────────────────────────────────

    // ── network tests (ignored) ───────────────────────────────────────────

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn execute_real_openai_request() {
        let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
        let mut payload = make_payload("openai_compatible", "https://api.openai.com");
        payload.api_key = api_key;

        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        let client = UpstreamClient::new(30);
        let result = client.execute(&payload, &req, "gpt-4o-mini", None).await;
        assert!(result.is_ok(), "{result:?}");
    }
}
