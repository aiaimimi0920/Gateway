// ---------------------------------------------------------------------------
// http/router.rs — axum Router construction
// ---------------------------------------------------------------------------

use super::middleware::{body_limit_layer, request_logging};
use super::routes::{health, metrics};
use super::ui;
use crate::state::AppState;
use axum::{middleware as axum_mw, routing::get, Router};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

mod analysis_reports;
mod credential_lifecycle;
mod inference;
mod management_access;
mod management_config;
mod request_audits;

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
    if crate::provider_discovery::job::requested(state.route_config.snapshot().document()) {
        crate::provider_discovery::background::schedule(&state);
    }
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let router = Router::new()
        .route("/ui", get(ui::redirect_ui_root))
        .route("/ui/", get(ui::serve_ui_index))
        .route("/ui/*path", get(ui::serve_ui_path));
    let router = inference::mount(router, &state);
    let router = management_access::mount(router);
    let router = management_config::mount(router);
    let router = request_audits::mount(router);
    let router = credential_lifecycle::mount(router);
    let router = analysis_reports::mount(router);
    router
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
