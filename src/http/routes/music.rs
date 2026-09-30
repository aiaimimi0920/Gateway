use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    Json,
};

use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::http::request_headers::apply_public_request_headers;
use crate::pipeline::{run_pipeline, PipelineContext, PipelineOutput};
use crate::protocol::producer::normalize_music_generations;
use crate::state::AppState;

pub async fn handle_music_generations(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    crate::http::extractors::JsonBody(body): crate::http::extractors::JsonBody,
) -> Result<Response, GatewayError> {
    let canonical = normalize_music_generations(body)?;
    let mut ctx = PipelineContext::new(canonical, token);
    apply_common_headers(
        &mut ctx,
        &headers,
        &state.config,
        state.console_auth.as_ref(),
    )?;
    ctx.query_params = query_params;

    match run_pipeline(ctx, &state).await? {
        PipelineOutput::Json(value) => Ok(Json(value).into_response()),
        PipelineOutput::Sse(_) => Err(GatewayError::server_error(
            "Music generation endpoints do not support streaming responses.",
        )
        .with_code("music_streaming_not_supported")),
        PipelineOutput::Binary(_) => Err(GatewayError::server_error(
            "unexpected binary response for music generation endpoint",
        )),
    }
}

fn apply_common_headers(
    ctx: &mut PipelineContext,
    headers: &HeaderMap,
    config: &crate::config::Config,
    console_auth: &crate::console::ConsoleAuthRuntime,
) -> Result<(), GatewayError> {
    apply_public_request_headers(
        ctx,
        headers,
        config,
        console_auth,
        &[
            "cookie",
            "origin",
            "referer",
            "user-agent",
            "accept-language",
            "accept",
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::http::request_headers::TestConsoleAuthFixture;
    use crate::protocol::producer::normalize_music_generations;

    fn make_config() -> Config {
        make_config_with_management_token(None)
    }

    fn make_config_with_management_token(gateway_management_token: Option<&str>) -> Config {
        Config {
            console: Default::default(),
            runtime_role: crate::config::GatewayRuntimeRole::Standalone,
            storage_mode: Default::default(),
            port: 4200,
            redis_url: "redis://localhost".to_string(),
            database_url: None,
            upstream_timeout_secs: 30,
            max_request_body_bytes: 1024 * 1024,
            max_body_chat_completions_bytes: 1024 * 1024,
            max_body_completions_bytes: 1024 * 1024,
            max_body_messages_bytes: 1024 * 1024,
            max_body_responses_bytes: 1024 * 1024,
            max_body_embeddings_bytes: 1024 * 1024,
            max_body_audio_transcriptions_bytes: 8 * 1024 * 1024,
            max_body_audio_speech_bytes: 1024 * 1024,
            max_body_search_bytes: 512 * 1024,
            max_body_fetch_bytes: 512 * 1024,
            max_body_research_bytes: 512 * 1024,
            max_body_images_generations_bytes: 1024 * 1024,
            max_body_images_edits_bytes: 1024 * 1024,
            max_body_music_bytes: 1024 * 1024,
            max_body_videos_bytes: 1024 * 1024,
            response_cache_ttl_secs: 300,
            response_cache_max_size_bytes: 512 * 1024,
            quota_pre_deduct_estimate_ratio: 1.2,
            usage_report_batch_size: 100,
            provider_probe_interval_secs: 30,
            log_level: "info".to_string(),
            gateway_api_key: None,
            gateway_api_key_secret: None,
            gateway_management_token: gateway_management_token.map(str::to_string),
            gateway_keepalive_bearer_token: None,
            default_project_id: "platform-default-project".to_string(),
            gateway_inbound_api_key_header_aliases: Vec::new(),
            provider_credential_folder_sync_enabled: false,
            provider_credential_folder_sync_root_dir: None,
            provider_credential_folder_sync_interval_secs: 30,
            provider_credential_folder_sync_import_enabled: true,
            provider_credential_folder_sync_export_enabled: true,
            provider_credential_folder_sync_watch_enabled: true,
            provider_credential_folder_sync_watch_debounce_millis: 1500,
            provider_credential_folder_sync_delete_missing: false,
            provider_credential_refresh_enabled: true,
            provider_credential_refresh_interval_secs: 3600,
            provider_credential_refresh_before_secs: 86_400,
            provider_credential_refresh_batch_limit: 100,
            provider_credential_refresh_lock_ttl_secs: 300,
            credential_stock_monitor_enabled: true,
            credential_stock_monitor_interval_secs: 60,
            credential_pool_automation: Default::default(),
            splitter_worker_executable_path: None,
            splitter_initial_worker_port: 4201,
            splitter_ready_timeout_secs: 120,
            splitter_ready_poll_interval_millis: 500,
            splitter_reload_shutdown_timeout_secs: 600,
        }
    }

    #[test]
    fn apply_common_headers_forwards_music_headers() {
        let canonical = normalize_music_generations(serde_json::json!({
            "prompt": "hybrid orchestral brass"
        }))
        .unwrap();
        let mut ctx = PipelineContext::new(canonical, None);
        let mut headers = HeaderMap::new();
        headers.insert("x-internal-api-key", "management-token".parse().unwrap());
        headers.insert("x-credential-ref", "cred-producer".parse().unwrap());
        headers.insert("x-neuro-user", "user-music".parse().unwrap());
        headers.insert("cookie", "sb-sb-auth-token.0=alpha".parse().unwrap());
        headers.insert("origin", "https://www.producer.ai".parse().unwrap());

        let config = make_config_with_management_token(Some("management-token"));
        let auth = TestConsoleAuthFixture::with_environment_token("management-token");
        apply_common_headers(&mut ctx, &headers, &config, &auth.runtime).unwrap();

        assert_eq!(
            ctx.request_headers
                .get("x-credential-ref")
                .map(String::as_str),
            Some("cred-producer")
        );
        assert_eq!(
            ctx.request_headers.get("x-neuro-user").map(String::as_str),
            Some("user-music")
        );
        assert_eq!(
            ctx.request_headers.get("cookie").map(String::as_str),
            Some("sb-sb-auth-token.0=alpha")
        );
        assert_eq!(
            ctx.request_headers.get("origin").map(String::as_str),
            Some("https://www.producer.ai")
        );
    }

    #[test]
    fn apply_common_headers_does_not_forward_trusted_headers_without_management_token() {
        let canonical = normalize_music_generations(serde_json::json!({
            "prompt": "hybrid orchestral brass"
        }))
        .unwrap();
        let mut ctx = PipelineContext::new(canonical, None);
        let mut headers = HeaderMap::new();
        headers.insert("x-credential-ref", "cred-producer".parse().unwrap());
        headers.insert("x-neuro-user", "user-music".parse().unwrap());
        headers.insert("cookie", "sb-sb-auth-token.0=alpha".parse().unwrap());
        headers.insert("origin", "https://www.producer.ai".parse().unwrap());

        let config = make_config();
        let auth = TestConsoleAuthFixture::without_management_token();
        apply_common_headers(&mut ctx, &headers, &config, &auth.runtime).unwrap();

        assert_eq!(ctx.request_headers.get("x-credential-ref"), None);
        assert_eq!(ctx.request_headers.get("x-neuro-user"), None);
        assert_eq!(
            ctx.request_headers.get("cookie").map(String::as_str),
            Some("sb-sb-auth-token.0=alpha")
        );
        assert_eq!(
            ctx.request_headers.get("origin").map(String::as_str),
            Some("https://www.producer.ai")
        );
    }
}
