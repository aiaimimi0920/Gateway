// ---------------------------------------------------------------------------
// http/router.rs — axum Router construction
// ---------------------------------------------------------------------------

use std::sync::Arc;

use axum::{
    middleware as axum_mw,
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use crate::state::AppState;

use super::middleware::{body_limit_layer, request_logging};
use super::routes::{
    audio, bedrock, browser_executor, cohere, completions, credentials, embeddings, gemini,
    gemini_live, health, images, internal_access, internal_browser_executor, internal_console,
    internal_conversation_archives, internal_credential_stock, internal_gateway,
    internal_provider_accounts, internal_provider_credentials, internal_requests, internal_routing,
    internal_runtime, keepalive, messages, metrics, models, music, realtime, responses, search,
    videos,
};
use super::ui;

/// Build the axum [`Router`] with all gateway routes mounted.
///
/// Layers are applied outside-in (bottom of the chain runs first for requests):
///   1. CORS — must be outermost so preflight OPTIONS are handled before other
///      layers reject them.
///   2. Request lifecycle + logging — reject new work during drain, track
///      in-flight requests, log every completed request, and inject
///      `X-Request-Id`.
///   3. Request body size limit — reject oversized payloads before handlers.
pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let chat_routes = Router::new()
        .route(
            "/v1/chat/completions",
            post(completions::handle_chat_completions),
        )
        .layer(body_limit_layer(
            state.config.max_body_chat_completions_bytes,
        ));

    let completions_routes = Router::new()
        .route(
            "/v1/completions",
            post(completions::handle_legacy_completions),
        )
        .layer(body_limit_layer(state.config.max_body_completions_bytes));

    let messages_routes = Router::new()
        .route("/v1/messages", post(messages::handle_messages))
        .layer(body_limit_layer(state.config.max_body_messages_bytes));

    let responses_routes = Router::new()
        .route("/v1/responses", post(responses::handle_responses))
        .layer(body_limit_layer(state.config.max_body_responses_bytes));

    let gemini_generate_content_routes = Router::new()
        .route("/v1/models/*action", post(gemini::handle_v1_model_action))
        .route(
            "/v1beta/models/*action",
            post(gemini::handle_v1beta_model_action),
        )
        .layer(body_limit_layer(
            state.config.max_body_chat_completions_bytes,
        ));

    let bedrock_converse_routes = Router::new()
        .route("/model/:model/converse", post(bedrock::handle_converse))
        .route(
            "/model/:model/converse-stream",
            post(bedrock::handle_converse_stream),
        )
        .layer(body_limit_layer(
            state.config.max_body_chat_completions_bytes,
        ));

    let cohere_chat_routes = Router::new()
        .route("/v2/chat", post(cohere::handle_chat_v2))
        .layer(body_limit_layer(
            state.config.max_body_chat_completions_bytes,
        ));

    let image_routes = Router::new()
        .route(
            "/v1/images/generations",
            post(images::handle_image_generations),
        )
        .layer(body_limit_layer(
            state.config.max_body_images_generations_bytes,
        ));

    let embeddings_routes = Router::new()
        .route("/v1/embeddings", post(embeddings::handle_embeddings))
        .layer(body_limit_layer(state.config.max_body_embeddings_bytes));

    let audio_transcription_routes = Router::new()
        .route(
            "/v1/audio/transcriptions",
            post(audio::handle_audio_transcriptions),
        )
        .layer(body_limit_layer(
            state.config.max_body_audio_transcriptions_bytes,
        ));

    let audio_speech_routes = Router::new()
        .route("/v1/audio/speech", post(audio::handle_audio_speech))
        .layer(body_limit_layer(state.config.max_body_audio_speech_bytes));

    let image_edit_routes = Router::new()
        .route("/v1/images/edits", post(images::handle_image_edits))
        .layer(body_limit_layer(state.config.max_body_images_edits_bytes));

    let search_routes = Router::new()
        .route("/v1/search", post(search::handle_search))
        .layer(body_limit_layer(state.config.max_body_search_bytes));

    let fetch_routes = Router::new()
        .route("/v1/fetch", post(search::handle_fetch))
        .layer(body_limit_layer(state.config.max_body_fetch_bytes));

    let research_routes = Router::new()
        .route(
            "/v1/research",
            get(search::handle_research_list).post(search::handle_research_create),
        )
        .layer(body_limit_layer(state.config.max_body_research_bytes));

    let music_routes = Router::new()
        .route(
            "/v1/music/generations",
            post(music::handle_music_generations),
        )
        .layer(body_limit_layer(state.config.max_body_music_bytes));

    let video_routes = Router::new()
        .route(
            "/v1/videos/generations",
            post(videos::handle_video_generations),
        )
        .layer(body_limit_layer(state.config.max_body_videos_bytes));

    let new_api_chat_routes = Router::new()
        .route(
            "/v1/new-api/chat/completions",
            post(completions::handle_chat_completions),
        )
        .layer(body_limit_layer(
            state.config.max_body_chat_completions_bytes,
        ));

    let new_api_completions_routes = Router::new()
        .route(
            "/v1/new-api/completions",
            post(completions::handle_legacy_completions),
        )
        .layer(body_limit_layer(state.config.max_body_completions_bytes));

    let new_api_messages_routes = Router::new()
        .route("/v1/new-api/messages", post(messages::handle_messages))
        .layer(body_limit_layer(state.config.max_body_messages_bytes));

    let new_api_responses_routes = Router::new()
        .route("/v1/new-api/responses", post(responses::handle_responses))
        .layer(body_limit_layer(state.config.max_body_responses_bytes));

    let new_api_image_routes = Router::new()
        .route(
            "/v1/new-api/images/generations",
            post(images::handle_image_generations),
        )
        .layer(body_limit_layer(
            state.config.max_body_images_generations_bytes,
        ));

    let new_api_embeddings_routes = Router::new()
        .route(
            "/v1/new-api/embeddings",
            post(embeddings::handle_embeddings),
        )
        .layer(body_limit_layer(state.config.max_body_embeddings_bytes));

    let new_api_audio_transcription_routes = Router::new()
        .route(
            "/v1/new-api/audio/transcriptions",
            post(audio::handle_audio_transcriptions),
        )
        .layer(body_limit_layer(
            state.config.max_body_audio_transcriptions_bytes,
        ));

    let new_api_audio_speech_routes = Router::new()
        .route("/v1/new-api/audio/speech", post(audio::handle_audio_speech))
        .layer(body_limit_layer(state.config.max_body_audio_speech_bytes));

    let new_api_image_edit_routes = Router::new()
        .route("/v1/new-api/images/edits", post(images::handle_image_edits))
        .layer(body_limit_layer(state.config.max_body_images_edits_bytes));

    let new_api_search_routes = Router::new()
        .route("/v1/new-api/search", post(search::handle_search))
        .layer(body_limit_layer(state.config.max_body_search_bytes));

    let new_api_fetch_routes = Router::new()
        .route("/v1/new-api/fetch", post(search::handle_fetch))
        .layer(body_limit_layer(state.config.max_body_fetch_bytes));

    let new_api_research_routes = Router::new()
        .route(
            "/v1/new-api/research",
            get(search::handle_research_list).post(search::handle_research_create),
        )
        .layer(body_limit_layer(state.config.max_body_research_bytes));

    let new_api_music_routes = Router::new()
        .route(
            "/v1/new-api/music/generations",
            post(music::handle_music_generations),
        )
        .layer(body_limit_layer(state.config.max_body_music_bytes));

    let new_api_video_routes = Router::new()
        .route(
            "/v1/new-api/videos/generations",
            post(videos::handle_video_generations),
        )
        .layer(body_limit_layer(state.config.max_body_videos_bytes));

    Router::new()
        .route("/ui", get(ui::redirect_ui_root))
        .route("/ui/", get(ui::serve_ui_index))
        .route("/ui/*path", get(ui::serve_ui_path))
        // -- AI endpoints ------------------------------------------------
        .merge(chat_routes)
        .merge(completions_routes)
        .merge(messages_routes)
        .merge(responses_routes)
        .merge(gemini_generate_content_routes)
        .merge(bedrock_converse_routes)
        .merge(cohere_chat_routes)
        .merge(embeddings_routes)
        .merge(audio_transcription_routes)
        .merge(audio_speech_routes)
        .merge(image_routes)
        .merge(image_edit_routes)
        .merge(search_routes)
        .merge(fetch_routes)
        .merge(research_routes)
        .merge(music_routes)
        .merge(video_routes)
        .merge(new_api_chat_routes)
        .merge(new_api_completions_routes)
        .merge(new_api_messages_routes)
        .merge(new_api_responses_routes)
        .merge(new_api_embeddings_routes)
        .merge(new_api_audio_transcription_routes)
        .merge(new_api_audio_speech_routes)
        .merge(new_api_image_routes)
        .merge(new_api_image_edit_routes)
        .merge(new_api_search_routes)
        .merge(new_api_fetch_routes)
        .merge(new_api_research_routes)
        .merge(new_api_music_routes)
        .merge(new_api_video_routes)
        .route("/v1/research/:id", get(search::handle_research_get))
        .route("/v1/new-api/research/:id", get(search::handle_research_get))
        .route("/v1/credits/balance", get(search::handle_credits_balance))
        .route(
            "/v1/new-api/credits/balance",
            get(search::handle_credits_balance),
        )
        .route(
            "/v1/internal/providers/search/credits/balance",
            get(search::handle_internal_search_provider_balance),
        )
        // Legacy compatibility alias for older operator clients.
        .route(
            "/v1/internal/providers/linkup/credits/balance",
            get(search::handle_internal_search_provider_balance),
        )
        .route("/v1/models", get(models::handle_models))
        .route("/v1/new-api/models", get(models::handle_models))
        .route("/v1/realtime", get(realtime::handle_realtime))
        .route(
            "/ws/google.ai.generativelanguage.v1alpha.GenerativeService/BidiGenerateContent",
            get(gemini_live::handle_bidi_generate_content),
        )
        .route(
            "/ws/google.ai.generativelanguage.v1beta.GenerativeService/BidiGenerateContent",
            get(gemini_live::handle_bidi_generate_content),
        )
        // -- Credential checkout -----------------------------------------
        .route("/v1/credentials/checkout", post(credentials::checkout))
        .route(
            "/v1/credentials/ensure",
            post(keepalive::ensure_credential_runtime),
        )
        .route(
            "/v1/internal/credentials/ensure",
            post(keepalive::ensure_credential_runtime),
        )
        .route(
            "/v1/internal/browser-executor/health",
            get(browser_executor::get_browser_executor_health),
        )
        .route(
            "/v1/internal/browser-executor/execute",
            post(browser_executor::execute_browser_executor),
        )
        // -- Internal gateway management ---------------------------------
        .route(
            "/v1/internal/gateway/api-access/resolve",
            post(internal_gateway::resolve_api_access),
        )
        .route(
            "/v1/internal/gateway/api-access/rotate",
            post(internal_gateway::rotate_api_access),
        )
        .route(
            "/v1/internal/gateway/benefit-projects/ensure",
            post(internal_gateway::ensure_benefit_project),
        )
        .route(
            "/v1/internal/gateway/projects/:project_id/api-access",
            get(internal_gateway::get_project_api_access),
        )
        .route(
            "/v1/internal/gateway/projects/:project_id/api-access/rotate",
            post(internal_gateway::rotate_project_api_access),
        )
        .route(
            "/v1/internal/gateway/projects/:project_id/prompt-cache/summary",
            get(internal_gateway::summarize_project_prompt_cache),
        )
        .route(
            "/v1/internal/gateway/projects/:project_id/prompt-cache/trend-report",
            get(internal_gateway::get_project_prompt_cache_trend_report),
        )
        .route(
            "/v1/internal/gateway/user-credentials/issue",
            post(internal_gateway::issue_user_credential),
        )
        .route(
            "/v1/internal/gateway/user-credentials/verify",
            post(internal_gateway::verify_user_credential),
        )
        .route(
            "/v1/internal/gateway/user-credentials/revoke",
            post(internal_gateway::revoke_user_credential),
        )
        .route(
            "/v1/internal/user-credentials/issue",
            post(internal_gateway::issue_user_credential),
        )
        .route(
            "/v1/internal/user-credentials/verify",
            post(internal_gateway::verify_user_credential),
        )
        .route(
            "/v1/internal/user-credentials/revoke",
            post(internal_gateway::revoke_user_credential),
        )
        .route(
            "/v1/internal/gateway/provider-credentials",
            post(internal_provider_credentials::create_provider_credential),
        )
        .route(
            "/v1/internal/gateway/provider-credentials/folder-sync/status",
            get(internal_provider_credentials::get_provider_credential_folder_sync_status)
                .put(internal_provider_credentials::update_provider_credential_folder_sync_status),
        )
        .route(
            "/v1/internal/gateway/provider-credentials/folder-sync/import",
            post(internal_provider_credentials::import_provider_credentials_from_folder),
        )
        .route(
            "/v1/internal/gateway/provider-credentials/folder-sync/export",
            post(internal_provider_credentials::export_provider_credentials_to_folder),
        )
        .route(
            "/v1/internal/gateway/provider-credentials/:provider_credential_id/quota",
            get(internal_provider_credentials::get_provider_credential_quota)
                .post(internal_provider_credentials::refresh_provider_credential_quota),
        )
        .route(
            "/v1/internal/gateway/provider-credentials/:provider_credential_id",
            get(internal_provider_credentials::get_provider_credential)
                .put(internal_provider_credentials::update_provider_credential)
                .delete(internal_provider_credentials::delete_provider_credential),
        )
        .route(
            "/v1/internal/gateway/provider-accounts/:provider_account_id/credentials",
            get(internal_provider_credentials::list_provider_credentials_for_account)
                .post(internal_provider_credentials::create_provider_credential_for_account),
        )
        .route(
            "/v1/internal/gateway/provider-inventory",
            get(internal_provider_accounts::get_provider_inventory),
        )
        .route(
            "/v1/internal/gateway/access/catalog",
            get(internal_access::get_access_catalog),
        )
        .route(
            "/v1/internal/gateway/access/provider-capabilities",
            post(internal_access::create_provider_capability),
        )
        .route(
            "/v1/internal/gateway/access/provider-capabilities/:access_id",
            post(internal_access::update_provider_capability),
        )
        .route(
            "/v1/internal/gateway/access/platform-access",
            post(internal_access::create_platform_access),
        )
        .route(
            "/v1/internal/gateway/access/platform-access/:access_id",
            post(internal_access::update_platform_access),
        )
        .route(
            "/v1/internal/gateway/access/bundles",
            post(internal_access::create_access_bundle),
        )
        .route(
            "/v1/internal/gateway/access/bundles/:bundle_id",
            post(internal_access::update_access_bundle).delete(internal_access::delete_access_bundle),
        )
        .route(
            "/v1/internal/gateway/access/bundles/:bundle_id/items/replace",
            post(internal_access::replace_bundle_items),
        )
        .route(
            "/v1/internal/gateway/access/bundles/:bundle_id/user-keys/ensure",
            post(internal_access::ensure_bundle_user_key),
        )
        .route(
            "/v1/internal/gateway/access/keys",
            post(internal_access::create_access_key),
        )
        .route(
            "/v1/internal/gateway/access/keys/:access_key_id",
            post(internal_access::update_access_key).delete(internal_access::delete_access_key),
        )
        .route(
            "/v1/internal/gateway/access/keys/:access_key_id/rotate",
            post(internal_access::rotate_access_key),
        )
        .route(
            "/v1/internal/gateway/access/keys/:access_key_id/revoke",
            post(internal_access::revoke_access_key),
        )
        .route(
            "/v1/internal/gateway/access/keys/:access_key_id/balance",
            get(internal_access::get_access_key_balance),
        )
        .route(
            "/v1/internal/gateway/access/keys/:access_key_id/balances/adjust",
            post(internal_access::adjust_access_key_balance),
        )
        .route(
            "/v1/internal/gateway/access/keys/:access_key_id/aggregate-memberships/replace",
            post(internal_access::replace_aggregate_memberships),
        )
        .route(
            "/v1/internal/gateway/access/preview/candidates",
            get(internal_access::preview_candidates),
        )
        .route(
            "/v1/internal/gateway/access/preview/route-decision",
            get(internal_access::preview_route_decision),
        )
        .route(
            "/v1/internal/gateway/access/affinity",
            get(internal_access::inspect_affinity).post(internal_access::reset_affinity),
        )
        .route(
            "/v1/internal/gateway/browser-executor/nodes/heartbeat",
            post(internal_browser_executor::heartbeat_browser_executor_node),
        )
        .route(
            "/v1/internal/gateway/browser-executor/nodes",
            get(internal_browser_executor::list_browser_executor_nodes),
        )
        .route(
            "/v1/internal/gateway/browser-executor/slots",
            get(internal_browser_executor::list_browser_capability_slots)
                .post(internal_browser_executor::upsert_browser_capability_slot),
        )
        .route(
            "/v1/internal/gateway/browser-executor/leases/acquire",
            post(internal_browser_executor::acquire_browser_capability_lease),
        )
        .route(
            "/v1/internal/gateway/browser-executor/leases/:lease_id/release",
            post(internal_browser_executor::release_browser_capability_lease),
        )
        .route(
            "/v1/internal/gateway/browser-executor/leases",
            get(internal_browser_executor::list_browser_capability_leases),
        )
        .route(
            "/v1/internal/gateway/browser-executor/health",
            get(internal_browser_executor::get_browser_executor_management_health),
        )
        .route(
            "/v1/internal/gateway/model-associations",
            get(internal_requests::get_model_association_matrix),
        )
        .route(
            "/v1/internal/gateway/costs",
            get(internal_requests::get_cost_overview),
        )
        .route(
            "/v1/internal/gateway/provider-accounts",
            get(internal_provider_accounts::list_provider_accounts)
                .post(internal_provider_accounts::create_provider_account),
        )
        .route(
            "/v1/internal/gateway/provider-accounts/:provider_account_id",
            get(internal_provider_accounts::get_provider_account)
                .post(internal_provider_accounts::update_provider_account)
                .delete(internal_provider_accounts::delete_provider_account),
        )
        .route(
            "/v1/internal/gateway/provider-accounts/:provider_account_id/source-profile",
            post(internal_provider_accounts::patch_provider_source_profile),
        )
        .route(
            "/v1/internal/gateway/provider-accounts/:provider_account_id/model-tiering",
            get(internal_provider_accounts::get_provider_model_tiering)
                .post(internal_provider_accounts::save_provider_model_tiering),
        )
        .route(
            "/v1/internal/gateway/provider-accounts/:provider_account_id/model-pricing",
            post(internal_provider_accounts::patch_provider_model_pricing),
        )
        .route(
            "/v1/internal/gateway/provider-accounts/source-profile/backfill",
            post(internal_provider_accounts::backfill_provider_source_profiles),
        )
        .route(
            "/v1/internal/gateway/provider-accounts/:provider_account_id/probe",
            post(internal_provider_accounts::probe_provider_account),
        )
        .route(
            "/v1/internal/gateway/provider-accounts/:provider_account_id/quota",
            get(internal_provider_accounts::get_provider_quota)
                .post(internal_provider_accounts::refresh_provider_quota),
        )
        .route(
            "/v1/internal/gateway/provider-quotas/:provider_account_id",
            get(internal_provider_accounts::get_provider_quota)
                .post(internal_provider_accounts::refresh_provider_quota),
        )
        .route(
            "/v1/internal/gateway/provider-accounts/sweep-cooling",
            post(internal_provider_accounts::sweep_cooling_provider_accounts),
        )
        .route(
            "/v1/internal/gateway/route-policies",
            get(internal_routing::list_route_policies).post(internal_routing::create_route_policy),
        )
        .route(
            "/v1/internal/gateway/route-policies/:policy_id",
            post(internal_routing::update_route_policy),
        )
        .route(
            "/v1/internal/gateway/model-aliases",
            get(internal_routing::list_model_aliases).post(internal_routing::create_model_alias),
        )
        .route(
            "/v1/internal/gateway/model-aliases/:alias_id",
            post(internal_routing::update_model_alias).delete(internal_routing::delete_model_alias),
        )
        .route(
            "/v1/internal/gateway/readiness",
            get(internal_runtime::get_gateway_readiness),
        )
        .route(
            "/v1/internal/gateway/operations/summary",
            get(internal_runtime::get_gateway_operator_summary),
        )
        .route(
            "/v1/internal/gateway/runtime/drain",
            post(internal_runtime::drain_gateway_runtime),
        )
        .route(
            "/v1/internal/gateway/route-config",
            get(internal_console::get_route_config).post(internal_console::commit_route_config),
        )
        .route(
            "/v1/internal/gateway/route-config/revisions",
            get(internal_console::list_route_config_revisions),
        )
        .route(
            "/v1/internal/gateway/route-config/revisions/:revision_id",
            get(internal_console::get_route_config_revision),
        )
        .route(
            "/v1/internal/gateway/pressure",
            get(internal_requests::get_runtime_pressure),
        )
        .route(
            "/v1/internal/gateway/requests",
            get(internal_requests::list_request_audits),
        )
        .route(
            "/v1/internal/gateway/requests/:requestAuditId",
            get(internal_requests::get_request_audit_by_id),
        )
        .route(
            "/v1/internal/gateway/requests/by-response/:responseId",
            get(internal_requests::get_request_audit_by_response_id),
        )
        .route(
            "/v1/internal/gateway/requests/:requestAuditId/artifacts",
            get(internal_requests::get_request_artifacts_by_id),
        )
        .route(
            "/v1/internal/gateway/requests/by-response/:responseId/artifacts",
            get(internal_requests::get_request_artifacts_by_response_id),
        )
        .route(
            "/v1/internal/gateway/requests/summary",
            get(internal_requests::summarize_request_audits),
        )
        .route(
            "/v1/internal/gateway/conversation-archives",
            get(internal_conversation_archives::list_conversation_archives),
        )
        .route(
            "/v1/internal/gateway/conversation-archives/export",
            post(internal_conversation_archives::export_conversation_archives),
        )
        .route(
            "/v1/internal/gateway/conversation-archives/datasets",
            get(internal_conversation_archives::list_conversation_dataset_exports)
                .post(internal_conversation_archives::create_conversation_dataset_export),
        )
        .route(
            "/v1/internal/gateway/conversation-archives/datasets/:datasetId/review",
            post(internal_conversation_archives::review_conversation_dataset_export),
        )
        .route(
            "/v1/internal/gateway/conversation-archives/datasets/:datasetId/publish",
            post(internal_conversation_archives::publish_conversation_dataset_export),
        )
        .route(
            "/v1/internal/gateway/conversation-archives/:archiveId",
            get(internal_conversation_archives::get_conversation_archive),
        )
        .route(
            "/v1/internal/gateway/conversation-archives/:archiveId/artifacts",
            get(internal_conversation_archives::get_conversation_archive_artifacts),
        )
        .route(
            "/v1/internal/gateway/analysis/samples",
            get(internal_requests::list_analysis_samples),
        )
        .route(
            "/v1/internal/gateway/analysis/summary",
            get(internal_requests::summarize_analysis),
        )
        .route(
            "/v1/internal/gateway/analysis/prompt-cache/summary",
            get(internal_requests::summarize_prompt_cache),
        )
        .route(
            "/v1/internal/gateway/analysis/prompt-cache/trend-report",
            get(internal_requests::get_prompt_cache_trend_report),
        )
        .route(
            "/v1/internal/gateway/provider-credential-model-states",
            get(internal_gateway::list_provider_credential_model_states),
        )
        .route(
            "/v1/internal/gateway/credential-stock/status",
            get(internal_credential_stock::list_credential_stock_status),
        )
        .route(
            "/v1/internal/gateway/credential-stock/policies",
            get(internal_credential_stock::list_credential_stock_policies_route)
                .post(internal_credential_stock::upsert_credential_stock_policy_route),
        )
        .route(
            "/v1/internal/gateway/credential-stock/signals/sweep",
            post(internal_credential_stock::sweep_credential_stock_signals_route),
        )
        .route(
            "/v1/internal/gateway/usage-aggregates",
            get(internal_gateway::list_usage_aggregates),
        )
        .route(
            "/v1/internal/gateway/usage-aggregates/flush",
            post(internal_gateway::flush_usage_aggregates),
        )
        .route(
            "/v1/internal/gateway/usage-aggregates/summary",
            get(internal_gateway::summarize_usage_aggregates),
        )
        .route(
            "/v1/internal/gateway/analysis/export",
            get(internal_requests::export_analysis_rows)
                .post(internal_requests::persist_analysis_export),
        )
        .route(
            "/v1/internal/gateway/analysis/exports",
            get(internal_requests::list_persisted_analysis_exports),
        )
        .route(
            "/v1/internal/gateway/analysis/exports/summary",
            get(internal_requests::summarize_persisted_analysis_exports),
        )
        .route(
            "/v1/internal/gateway/analysis/exports/baseline-report",
            get(internal_requests::get_analysis_export_baseline_report),
        )
        .route(
            "/v1/internal/gateway/analysis/exports/timeline-report",
            get(internal_requests::get_analysis_export_timeline_report),
        )
        .route(
            "/v1/internal/gateway/analysis/exports/trend-report",
            get(internal_requests::get_analysis_export_trend_report),
        )
        .route(
            "/v1/internal/gateway/analysis/exports/anomaly-report",
            get(internal_requests::get_analysis_export_anomaly_report),
        )
        .route(
            "/v1/internal/gateway/analysis/exports/diff",
            get(internal_requests::get_persisted_analysis_export_diff),
        )
        .route(
            "/v1/internal/gateway/analysis/exports/:exportId",
            get(internal_requests::get_persisted_analysis_export)
                .post(internal_requests::update_persisted_analysis_export_metadata),
        )
        .route(
            "/v1/internal/gateway/analysis/exports/cleanup-expired",
            post(internal_requests::cleanup_expired_analysis_exports),
        )
        .route(
            "/v1/internal/gateway/analysis/provider-routing/summary",
            get(internal_requests::summarize_provider_routing_analysis),
        )
        .route(
            "/v1/internal/gateway/analysis/rate-limit-hotspots",
            get(internal_requests::summarize_rate_limit_hotspots),
        )
        .route(
            "/v1/internal/gateway/analysis/rate-limit-hotspots/snapshot",
            post(internal_requests::persist_rate_limit_hotspot_snapshot),
        )
        .route(
            "/v1/internal/gateway/analysis/rate-limit-hotspots/snapshots",
            get(internal_requests::list_rate_limit_hotspot_snapshots),
        )
        .route(
            "/v1/internal/gateway/analysis/rate-limit-hotspots/snapshots/summary",
            get(internal_requests::summarize_rate_limit_hotspot_snapshot_inventory),
        )
        .route(
            "/v1/internal/gateway/analysis/rate-limit-hotspots/snapshots/trend-report",
            get(internal_requests::get_rate_limit_hotspot_snapshot_trend_report),
        )
        .route(
            "/v1/internal/gateway/analysis/rate-limit-hotspots/snapshots/:snapshotId",
            get(internal_requests::get_rate_limit_hotspot_snapshot),
        )
        .route(
            "/v1/internal/gateway/analysis/rate-limit-hotspots/trend-report",
            get(internal_requests::get_rate_limit_hotspot_trend_report),
        )
        .route(
            "/v1/internal/gateway/analysis/rate-limit-hotspots/anomaly-report",
            get(internal_requests::get_rate_limit_hotspot_anomaly_report),
        )
        .route(
            "/v1/internal/gateway/analysis/rate-limit-hotspots/anomaly-snapshot",
            post(internal_requests::persist_rate_limit_hotspot_anomaly_snapshot),
        )
        .route(
            "/v1/internal/gateway/analysis/rate-limit-hotspots/anomaly-snapshots",
            get(internal_requests::list_rate_limit_hotspot_anomaly_snapshots),
        )
        .route(
            "/v1/internal/gateway/analysis/rate-limit-hotspots/anomaly-snapshots/:snapshotId",
            get(internal_requests::get_rate_limit_hotspot_anomaly_snapshot),
        )
        .route(
            "/v1/internal/gateway/analysis/rate-limit-hotspots/anomaly-incidents/sync",
            post(internal_requests::sync_rate_limit_hotspot_anomaly_incidents),
        )
        .route(
            "/v1/internal/gateway/analysis/provider-routing/anomaly-report",
            get(internal_requests::get_provider_routing_anomaly_report),
        )
        .route(
            "/v1/internal/gateway/analysis/provider-routing/anomaly-incidents/sync",
            post(internal_requests::sync_provider_routing_anomaly_incidents),
        )
        .route(
            "/v1/internal/gateway/analysis/anomaly-policies",
            get(internal_requests::list_anomaly_policies)
                .post(internal_requests::save_anomaly_policy),
        )
        .route(
            "/v1/internal/gateway/analysis/anomaly-policies/:policy_id/sync",
            post(internal_requests::sync_anomaly_policy),
        )
        .route(
            "/v1/internal/gateway/analysis/anomaly-policies/sweep-sync",
            post(internal_requests::sweep_anomaly_policies),
        )
        .route(
            "/v1/internal/gateway/analysis/anomaly-policies/summary",
            get(internal_requests::summarize_anomaly_policies),
        )
        .route(
            "/v1/internal/gateway/analysis/anomaly-incidents",
            get(internal_requests::list_anomaly_incidents),
        )
        .route(
            "/v1/internal/gateway/analysis/anomaly-incidents/sync",
            post(internal_requests::sync_anomaly_incidents),
        )
        .route(
            "/v1/internal/gateway/analysis/anomaly-incidents/alert-queue",
            get(internal_requests::list_anomaly_incident_alert_queue),
        )
        .route(
            "/v1/internal/gateway/analysis/anomaly-incidents/:incidentId/history",
            get(internal_requests::list_anomaly_incident_history),
        )
        .route(
            "/v1/internal/gateway/analysis/anomaly-incidents/:incidentId/alert-dispatch",
            post(internal_requests::record_anomaly_incident_alert_dispatch),
        )
        .route(
            "/v1/internal/gateway/analysis/anomaly-incidents/:incidentId/remediation-plan",
            get(internal_requests::get_anomaly_incident_remediation_plan),
        )
        .route(
            "/v1/internal/gateway/analysis/anomaly-incidents/:incidentId/remediation-runs",
            get(internal_requests::list_incident_remediation_runs)
                .post(internal_requests::execute_incident_remediation_run),
        )
        .route(
            "/v1/internal/gateway/analysis/anomaly-incidents/summary",
            get(internal_requests::summarize_anomaly_incidents),
        )
        .route(
            "/v1/internal/gateway/analysis/remediation-runs",
            get(internal_requests::list_remediation_runs),
        )
        .route(
            "/v1/internal/gateway/analysis/remediation-queue",
            get(internal_requests::list_remediation_queue),
        )
        .route(
            "/v1/internal/gateway/analysis/remediation-runs/sweep",
            post(internal_requests::sweep_remediation_runs),
        )
        .route(
            "/v1/internal/gateway/analysis/remediation-runs/summary",
            get(internal_requests::summarize_remediation_runs),
        )
        .route(
            "/v1/internal/gateway/analysis/remediation-runs/effectiveness",
            get(internal_requests::summarize_remediation_effectiveness),
        )
        .route(
            "/v1/internal/gateway/analysis/remediation-runs/effectiveness/snapshot",
            post(internal_requests::persist_remediation_effectiveness_snapshot),
        )
        .route(
            "/v1/internal/gateway/analysis/remediation-runs/effectiveness/snapshots",
            get(internal_requests::list_remediation_effectiveness_snapshots),
        )
        .route(
            "/v1/internal/gateway/analysis/remediation-runs/effectiveness/snapshots/summary",
            get(internal_requests::summarize_remediation_effectiveness_snapshots),
        )
        .route(
            "/v1/internal/gateway/analysis/remediation-runs/effectiveness/snapshots/trend-report",
            get(internal_requests::get_remediation_effectiveness_trend_report),
        )
        .route(
            "/v1/internal/gateway/analysis/remediation-runs/effectiveness/snapshots/anomaly-report",
            get(internal_requests::get_remediation_effectiveness_snapshot_anomaly_report),
        )
        .route(
            "/v1/internal/gateway/analysis/remediation-runs/effectiveness/snapshots/anomaly-snapshots",
            get(internal_requests::list_remediation_effectiveness_anomaly_snapshots)
                .post(internal_requests::persist_remediation_effectiveness_anomaly_snapshot),
        )
        .route(
            "/v1/internal/gateway/analysis/remediation-runs/effectiveness/snapshots/anomaly-snapshots/:snapshotId",
            get(internal_requests::get_remediation_effectiveness_anomaly_snapshot),
        )
        .route(
            "/v1/internal/gateway/analysis/remediation-runs/effectiveness/snapshots/:snapshotId",
            get(internal_requests::get_remediation_effectiveness_snapshot),
        )
        .route(
            "/v1/internal/gateway/analysis/remediation-runs/:runId/impact",
            get(internal_requests::get_remediation_run_impact),
        )
        .route(
            "/v1/internal/gateway/analysis/remediation-runs/:runId/capture-impact",
            post(internal_requests::capture_remediation_run_impact),
        )
        .route(
            "/v1/internal/gateway/analysis/anomaly-incidents/:incidentId/acknowledge",
            post(internal_requests::acknowledge_anomaly_incident),
        )
        .route(
            "/v1/internal/gateway/analysis/anomaly-incidents/:incidentId/resolve",
            post(internal_requests::resolve_anomaly_incident),
        )
        .route(
            "/v1/internal/gateway/analysis/anomaly-incidents/:incidentId/follow-up",
            post(internal_requests::update_anomaly_incident_follow_up),
        )
        // -- Observability -----------------------------------------------
        .route("/healthz", get(health::healthz))
        .route("/readyz", get(health::readyz))
        .route("/metrics", get(metrics::handle_metrics))
        // -- Middleware layers (applied inside-out for requests) ----------
        .layer(body_limit_layer(state.config.max_request_body_bytes))
        .layer(axum_mw::from_fn_with_state(
            Arc::clone(&state),
            request_logging,
        ))
        .layer(cors)
        // -- State -------------------------------------------------------
        .with_state(state)
}
