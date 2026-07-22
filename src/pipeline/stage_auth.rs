// ---------------------------------------------------------------------------
// Pipeline stage 1 — authentication
//
// Runs the configured auth adapter chain and populates `ctx.session` plus
// `ctx.credential_ref`.
// ---------------------------------------------------------------------------

use std::sync::Arc;

use crate::auth::adapter::AuthRequest;
use crate::error::GatewayError;
use crate::state::AppState;

use super::{CredentialSource, PipelineContext};

/// Authenticate the incoming request.
///
/// Authentication is resolved in this order:
/// 1. Try the configured auth adapter chain.
/// 2. If no adapter accepts the request, fall back to `gateway_api_key`
///    when configured.
/// 3. If neither a gateway key nor any adapters are configured, skip auth
///    entirely (dev mode) with a warning.
pub async fn run(ctx: &mut PipelineContext, state: &Arc<AppState>) -> Result<(), GatewayError> {
    let (path, method) = match ctx.canonical_req.endpoint_kind {
        crate::protocol::canonical::EndpointKind::ChatCompletions => {
            ("/v1/chat/completions", "POST")
        }
        crate::protocol::canonical::EndpointKind::Completions => ("/v1/completions", "POST"),
        crate::protocol::canonical::EndpointKind::Embeddings => ("/v1/embeddings", "POST"),
        crate::protocol::canonical::EndpointKind::ImagesGenerations => {
            ("/v1/images/generations", "POST")
        }
        crate::protocol::canonical::EndpointKind::ImagesEdits => ("/v1/images/edits", "POST"),
        crate::protocol::canonical::EndpointKind::MusicGenerations => {
            ("/v1/music/generations", "POST")
        }
        crate::protocol::canonical::EndpointKind::VideosGenerations => {
            ("/v1/videos/generations", "POST")
        }
        crate::protocol::canonical::EndpointKind::AudioTranscriptions => {
            ("/v1/audio/transcriptions", "POST")
        }
        crate::protocol::canonical::EndpointKind::AudioSpeech => ("/v1/audio/speech", "POST"),
        crate::protocol::canonical::EndpointKind::Messages => ("/v1/messages", "POST"),
        crate::protocol::canonical::EndpointKind::Responses => ("/v1/responses", "POST"),
        crate::protocol::canonical::EndpointKind::Search => ("/v1/search", "POST"),
        crate::protocol::canonical::EndpointKind::Fetch => ("/v1/fetch", "POST"),
        crate::protocol::canonical::EndpointKind::ResearchCreate => ("/v1/research", "POST"),
        crate::protocol::canonical::EndpointKind::ResearchList => ("/v1/research", "GET"),
        crate::protocol::canonical::EndpointKind::ResearchGet => ("/v1/research/:id", "GET"),
        crate::protocol::canonical::EndpointKind::CreditsBalance => ("/v1/credits/balance", "GET"),
    };

    let auth_req = AuthRequest {
        authorization: ctx.bearer_token.as_ref().map(|t| format!("Bearer {}", t)),
        api_key: ctx.request_headers.get("x-api-key").cloned(),
        path: path.to_string(),
        method: method.to_string(),
    };

    let session = crate::auth::authenticate_request(
        state.as_ref(),
        &auth_req,
        ctx.request_headers.get("x-credential-ref").cloned(),
    )
    .await?;

    ctx.credential_ref = session.credential_ref.clone();
    ctx.requesting_access_key_id = session.access_key_id.clone();
    ctx.session = Some(session);
    populate_credential_routing(ctx);
    Ok(())
}

/// Read platform-injected credential routing headers after successful auth.
///
/// - `X-Neuro-User`: user ID for credential isolation
/// - `X-Neuro-Cred-Source`: "hosted" for user's own credentials, default "platform"
fn populate_credential_routing(ctx: &mut PipelineContext) {
    ctx.neuro_user_id = ctx.request_headers.get("x-neuro-user").cloned();
    ctx.credential_source = match ctx
        .request_headers
        .get("x-neuro-cred-source")
        .map(|s| s.as_str())
    {
        Some("hosted") => CredentialSource::Hosted,
        _ => CredentialSource::Platform,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::adapter::{AuthAdapter, StaticKeyAdapter, StaticKeyEntry};
    use crate::concurrency::aimd::AimdConfig;
    use crate::concurrency::registry::ConcurrencyRegistry;
    use crate::config::Config;
    use crate::pipeline::PipelineContext;
    use crate::protocol::canonical::{
        CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole,
        ProtocolFamily,
    };
    use crate::upstream::client::UpstreamClient;
    use std::collections::HashMap;

    fn make_request() -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("gpt-4o".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hello".to_string(),
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

    fn make_state(adapters: Vec<Box<dyn AuthAdapter>>) -> Arc<AppState> {
        make_state_with_key(adapters, None)
    }

    fn make_state_with_key(
        adapters: Vec<Box<dyn AuthAdapter>>,
        gateway_api_key: Option<String>,
    ) -> Arc<AppState> {
        Arc::new(AppState {
            config: Config {
                console: Default::default(),
                runtime_role: crate::config::GatewayRuntimeRole::Standalone,
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
                gateway_api_key,
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
            upstream_client: UpstreamClient::new(30),
            concurrency_registry: ConcurrencyRegistry::new(AimdConfig::default()),
            auth_adapters: adapters,
            filter_config: None,
            route_config: Arc::new(crate::routing::config::RouteConfigStore::new()),
            route_config_runtime: None,
            credential_cache: crate::credential_store::CredentialMemoryCache::new(30),
            lifecycle: crate::state::GatewayLifecycleState::default(),
            shutdown: crate::state::GatewayShutdownHandle::default(),
            provider_credential_folder_sync: crate::state::ProviderCredentialFolderSyncRuntime::new(
                false,
            ),
        })
    }

    #[tokio::test]
    async fn no_adapters_no_key_dev_mode_succeeds() {
        let state = make_state(vec![]);
        let mut ctx = PipelineContext::new(make_request(), Some("any-token".to_string()));
        let result = run(&mut ctx, &state).await;
        assert!(result.is_ok());
        assert!(ctx.session.is_some());
        assert_eq!(
            ctx.session.as_ref().unwrap().api_key_id.as_deref(),
            Some("dev-mode")
        );
    }

    #[tokio::test]
    async fn gateway_api_key_valid_bearer_authenticates() {
        let state = make_state_with_key(vec![], Some("test-key".to_string()));
        let mut ctx = PipelineContext::new(make_request(), Some("test-key".to_string()));
        let result = run(&mut ctx, &state).await;
        assert!(result.is_ok());
        assert!(ctx.session.is_some());
        assert_eq!(
            ctx.session.as_ref().unwrap().api_key_id.as_deref(),
            Some("gateway-key")
        );
    }

    #[tokio::test]
    async fn gateway_api_key_wrong_bearer_rejects() {
        let state = make_state_with_key(vec![], Some("test-key".to_string()));
        let mut ctx = PipelineContext::new(make_request(), Some("wrong-key".to_string()));
        let result = run(&mut ctx, &state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn gateway_api_key_missing_bearer_rejects() {
        let state = make_state_with_key(vec![], Some("test-key".to_string()));
        let mut ctx = PipelineContext::new(make_request(), None);
        let result = run(&mut ctx, &state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn gateway_api_key_via_x_api_key_header() {
        let state = make_state_with_key(vec![], Some("test-key".to_string()));
        let mut ctx = PipelineContext::new(make_request(), None);
        ctx.request_headers
            .insert("x-api-key".to_string(), "test-key".to_string());
        let result = run(&mut ctx, &state).await;
        assert!(result.is_ok());
        assert!(ctx.session.is_some());
    }

    #[tokio::test]
    async fn gateway_key_populates_platform_routing_metadata() {
        let state = make_state_with_key(vec![], Some("test-key".to_string()));
        let mut ctx = PipelineContext::new(make_request(), None);
        ctx.request_headers
            .insert("x-api-key".to_string(), "test-key".to_string());
        ctx.request_headers
            .insert("x-credential-ref".to_string(), "cred-123".to_string());
        ctx.request_headers
            .insert("x-neuro-user".to_string(), "user-42".to_string());
        ctx.request_headers
            .insert("x-neuro-cred-source".to_string(), "hosted".to_string());

        let result = run(&mut ctx, &state).await;
        assert!(result.is_ok());
        assert_eq!(ctx.credential_ref.as_deref(), Some("cred-123"));
        assert_eq!(ctx.neuro_user_id.as_deref(), Some("user-42"));
        assert_eq!(ctx.credential_source, CredentialSource::Hosted);
    }

    #[tokio::test]
    async fn valid_key_authenticates() {
        let adapter = StaticKeyAdapter::from_pairs([(
            "valid-key".to_string(),
            StaticKeyEntry {
                project_id: "proj-1".to_string(),
                tenant_id: "tenant-1".to_string(),
                scopes: vec!["inference:chat".to_string()],
            },
        )]);
        let state = make_state(vec![Box::new(adapter)]);
        let mut ctx = PipelineContext::new(make_request(), Some("valid-key".to_string()));
        let result = run(&mut ctx, &state).await;
        assert!(result.is_ok());
        assert!(ctx.session.is_some());
        assert_eq!(ctx.session.as_ref().unwrap().project_id, "proj-1");
    }

    #[tokio::test]
    async fn invalid_key_rejected() {
        let adapter = StaticKeyAdapter::from_pairs([(
            "valid-key".to_string(),
            StaticKeyEntry {
                project_id: "proj-1".to_string(),
                tenant_id: "tenant-1".to_string(),
                scopes: vec![],
            },
        )]);
        let state = make_state(vec![Box::new(adapter)]);
        let mut ctx = PipelineContext::new(make_request(), Some("wrong-key".to_string()));
        let result = run(&mut ctx, &state).await;
        assert!(result.is_err());
    }
}
