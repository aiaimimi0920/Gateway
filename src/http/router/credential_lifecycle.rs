//! Credential stock, refill, automation and usage registration.

use crate::http::routes::{
    internal_credential_pool_automation, internal_credential_refill, internal_credential_stock,
    internal_gateway,
};
use crate::state::AppState;
use axum::{
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;

pub(super) fn mount(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
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
            "/v1/internal/gateway/credential-pool-automation",
            get(internal_credential_pool_automation::list_credential_pool_automation_status),
        )
        .route(
            "/v1/internal/gateway/credential-pool-automation/providers/:providerId/run",
            post(internal_credential_pool_automation::run_credential_pool_automation_for_provider),
        )
        .route(
            "/v1/internal/gateway/credential-pool-automation/providers/:providerId/prune",
            post(internal_credential_pool_automation::prune_credential_pool_for_provider),
        )
        .route(
            "/v1/internal/gateway/credential-pool-automation/providers/:providerId/archive",
            delete(internal_credential_pool_automation::purge_credential_pool_archive_for_provider),
        )
        .route(
            "/v1/internal/gateway/credential-pool-refill",
            get(internal_credential_refill::get_credential_refill_status),
        )
        .route(
            "/v1/internal/gateway/credential-pool-refill/tasks",
            get(internal_credential_refill::list_credential_refill_tasks_route),
        )
        .route(
            "/v1/internal/gateway/credential-pool-refill/providers/:providerId/request",
            post(internal_credential_refill::request_credential_refill_for_provider),
        )
        .route(
            "/v1/internal/gateway/credential-pool-refill/providers/:providerId",
            get(internal_credential_refill::get_credential_refill_status_for_provider),
        )
        .route(
            "/v1/internal/gateway/credential-pool-refill/providers/:providerId/tasks/claim",
            post(internal_credential_refill::claim_credential_refill_task_for_provider_route),
        )
        .route(
            "/v1/internal/gateway/credential-pool-refill/tasks/claim",
            post(internal_credential_refill::claim_credential_refill_task_route),
        )
        .route(
            "/v1/internal/gateway/credential-pool-refill/tasks/:taskId/renew",
            post(internal_credential_refill::renew_credential_refill_task_route),
        )
        .route(
            "/v1/internal/gateway/credential-pool-refill/tasks/:taskId/complete",
            post(internal_credential_refill::complete_credential_refill_task_route),
        )
        .route(
            "/v1/internal/gateway/credential-pool-refill/tasks/:taskId/fail",
            post(internal_credential_refill::fail_credential_refill_task_route),
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
}
