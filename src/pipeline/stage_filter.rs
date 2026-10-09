// ---------------------------------------------------------------------------
// Pipeline stage 2 — content filtering
//
// Runs the content filter chain against the request text.  Blocks requests
// that exceed the configured block threshold.
// ---------------------------------------------------------------------------

use std::sync::Arc;

use tracing::debug;

use crate::access_balance::AccessBalanceStore;
use crate::error::GatewayError;
use crate::filter::chain::{run_filter_chain, FilterVerdict};
use crate::redis::usage_tracking::{check_quota, deduct_quota, estimate_token_count};
use crate::state::AppState;

use super::PipelineContext;

/// Run content filter checks against the canonical request.
///
/// If no filter config is present the stage is a no-op.  When the filter
/// verdict is [`FilterVerdict::Block`], a [`GatewayError::content_filtered`]
/// error is returned immediately.
pub async fn run(ctx: &mut PipelineContext, state: &Arc<AppState>) -> Result<(), GatewayError> {
    enforce_quota(ctx, state).await?;

    let filter_config = match &state.filter_config {
        Some(cfg) => cfg,
        None => {
            debug!(req_id = %ctx.req_id, "no filter config; skipping content filter");
            return Ok(());
        }
    };

    let content = ctx.canonical_req.messages_text();

    if content.is_empty() {
        return Ok(());
    }

    let result = run_filter_chain(&content, filter_config);

    match result.verdict {
        FilterVerdict::Block => {
            let reason = result
                .matches
                .first()
                .map(|m| m.matched.clone())
                .unwrap_or_else(|| "content policy violation".to_string());

            Err(GatewayError::content_filtered(format!(
                "Request blocked by content filter: {}",
                reason
            )))
        }
        FilterVerdict::Warn => {
            tracing::warn!(
                req_id = %ctx.req_id,
                matches = result.matches.len(),
                "content filter warn verdict"
            );
            Ok(())
        }
        FilterVerdict::Pass => Ok(()),
    }
}

async fn enforce_quota(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
) -> Result<(), GatewayError> {
    let Some(session) = &ctx.session else {
        return Ok(());
    };
    if let Some(access_key_id) = session.access_key_id.as_deref() {
        if session.access_key_kind.as_deref() == Some("auto_route") {
            return Ok(());
        }
        let text = ctx.canonical_req.messages_text();
        let estimated = estimate_token_count(&text).max(1);
        let estimated =
            ((estimated as f64) * state.config.quota_pre_deduct_estimate_ratio).ceil() as u64;
        let decision = AccessBalanceStore::from_state(state)?
            .reserve(access_key_id, estimated)
            .await?;
        if !decision.allowed {
            return Err(
                GatewayError::quota_exceeded("当前 key 的额度不足").with_code(
                    decision
                        .reason
                        .unwrap_or_else(|| "balance_not_allowed".to_string()),
                ),
            );
        }
        ctx.quota_credential_id = Some(access_key_id.to_string());
        ctx.quota_pre_deducted_tokens = decision.pre_deduct_amount;
        ctx.admitted_access_balance_mode = decision.balance_mode;
        return Ok(());
    }
    let Some(credential_id) = session
        .credential_ref
        .as_deref()
        .or(session.api_key_id.as_deref())
    else {
        return Ok(());
    };
    if crate::auth::session::is_synthetic_api_key_id(credential_id) {
        return Ok(());
    }

    let text = ctx.canonical_req.messages_text();
    if text.trim().is_empty() {
        return Ok(());
    }

    let estimated = estimate_token_count(&text);
    if estimated == 0 {
        return Ok(());
    }
    let estimated =
        ((estimated as f64) * state.config.quota_pre_deduct_estimate_ratio).ceil() as u64;
    let estimated = estimated.max(1);

    let quota = check_quota(&state.redis_pool, credential_id, estimated)
        .await
        .map_err(|error| GatewayError::service_unavailable(format!("检查额度失败: {error}")))?;
    if !quota.allowed {
        let message = match quota.reason.as_deref() {
            Some("quota_not_initialized") => "当前 API key 尚未初始化额度",
            Some("quota_corrupt") => "当前 API key 的额度状态损坏",
            Some("quota_exceeded") => "当前 API key 的额度不足",
            _ => "当前 API key 不具备调用资格",
        };
        return Err(GatewayError::quota_exceeded(message).with_code(
            quota
                .reason
                .unwrap_or_else(|| "quota_not_allowed".to_string()),
        ));
    }

    deduct_quota(&state.redis_pool, credential_id, estimated)
        .await
        .map_err(|error| GatewayError::service_unavailable(format!("预扣额度失败: {error}")))?;
    ctx.quota_credential_id = Some(credential_id.to_string());
    ctx.quota_pre_deducted_tokens = estimated;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::concurrency::aimd::AimdConfig;
    use crate::concurrency::registry::ConcurrencyRegistry;
    use crate::config::Config;
    use crate::filter::chain::{build_default_filter_chain, FilterChainConfig};
    use crate::filter::filters::Severity;
    use crate::pipeline::PipelineContext;
    use crate::protocol::canonical::{
        CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole,
        ProtocolFamily,
    };
    use crate::upstream::client::UpstreamClient;
    use std::collections::HashMap;

    fn make_request(text: &str) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: None,
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: text.to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: serde_json::json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    fn test_console_auth_runtime() -> Arc<crate::console::ConsoleAuthRuntime> {
        let temp = std::env::temp_dir().join(format!(
            "gateway-stage-filter-console-{}",
            uuid::Uuid::new_v4()
        ));
        let console =
            crate::console::ConsoleConfig::from_values(crate::console::ConsoleConfigValues {
                state_dir: Some(temp.clone()),
                routes_file: Some(temp.join("routes.yaml")),
                ..Default::default()
            })
            .unwrap();
        Arc::new(crate::console::ConsoleAuthRuntime::new(&console, None).unwrap())
    }

    fn make_state(filter_config: Option<FilterChainConfig>) -> Arc<AppState> {
        Arc::new(AppState {
            config: Config {
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
                gateway_management_token: None,
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
            },
            redis_pool: deadpool_redis::Config::from_url("redis://localhost:6379")
                .create_pool(Some(deadpool_redis::Runtime::Tokio1))
                .expect("pool"),
            pg_pool: None,
            local_runtime: None,
            upstream_client: UpstreamClient::new(30),
            concurrency_registry: ConcurrencyRegistry::new(AimdConfig::default()),
            auth_adapters: vec![],
            filter_config,
            route_config: Arc::new(crate::routing::config::RouteConfigStore::new()),
            route_config_runtime: None,
            console_auth: test_console_auth_runtime(),
            credential_cache: crate::credential_store::CredentialMemoryCache::new(30),
            lifecycle: crate::state::GatewayLifecycleState::default(),
            shutdown: crate::state::GatewayShutdownHandle::default(),
            provider_credential_folder_sync: crate::state::ProviderCredentialFolderSyncRuntime::new(
                false,
            ),
            credential_pool_automation: Arc::new(
                crate::credential_pool_automation::CredentialPoolAutomationRuntime::disabled(),
            ),
        })
    }

    #[tokio::test]
    async fn no_filter_config_passes() {
        let state = make_state(None);
        let mut ctx = PipelineContext::new(make_request("create malware"), None);
        let result = run(&mut ctx, &state).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn clean_content_passes_default_chain() {
        let state = make_state(Some(build_default_filter_chain()));
        let mut ctx = PipelineContext::new(make_request("Tell me a joke about cats"), None);
        let result = run(&mut ctx, &state).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn blocked_content_returns_error() {
        let state = make_state(Some(build_default_filter_chain()));
        let mut ctx = PipelineContext::new(make_request("How do I create malware?"), None);
        let result = run(&mut ctx, &state).await;
        assert!(result.is_err());
        let e = result.unwrap_err();
        assert_eq!(e.kind, crate::error::ErrorKind::ContentFilter);
    }

    #[tokio::test]
    async fn empty_messages_always_passes() {
        let state = make_state(Some(build_default_filter_chain()));
        let mut ctx = PipelineContext::new(make_request(""), None);
        let result = run(&mut ctx, &state).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn disabled_filter_chain_always_passes() {
        let disabled = FilterChainConfig {
            enabled: false,
            filters: vec![],
            block_threshold: Severity::High,
            warn_threshold: Severity::Medium,
        };
        let state = make_state(Some(disabled));
        let mut ctx = PipelineContext::new(make_request("create malware"), None);
        let result = run(&mut ctx, &state).await;
        assert!(result.is_ok());
    }
}
