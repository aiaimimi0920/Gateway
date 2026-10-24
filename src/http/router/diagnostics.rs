//! Management diagnostics remain available independently of the optional web assets.
use crate::{http::routes::internal_console::diagnostics, state::AppState};
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

pub(super) fn mount(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .route(
            "/v1/internal/gateway/console/diagnostics/settings",
            get(diagnostics::get_settings).put(diagnostics::save_settings),
        )
        .route(
            "/v1/internal/gateway/console/diagnostics/export",
            post(diagnostics::export),
        )
}
