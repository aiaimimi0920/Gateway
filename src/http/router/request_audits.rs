//! Request audit, archive and analysis summary registration.

use crate::http::routes::{internal_conversation_archives, internal_requests};
use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

pub(super) fn mount(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
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
}
