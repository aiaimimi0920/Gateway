//! Management diagnostics remain available independently of the optional web assets.
use crate::http::routes::internal_console::proxy_settings;
use crate::{http::routes::internal_console::diagnostics, state::AppState};
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

pub(super) fn mount(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .route(
            "/v1/internal/gateway/console/lan-sharing",
            get(crate::http::routes::internal_console::lan_sharing::get_settings)
                .put(crate::http::routes::internal_console::lan_sharing::save_settings),
        )
        .route(
            "/v1/internal/gateway/console/proxy/settings",
            get(proxy_settings::get_settings).put(proxy_settings::save_settings),
        )
        .route(
            "/v1/internal/gateway/console/proxy/test",
            post(proxy_settings::test_settings),
        )
        .route(
            "/v1/internal/gateway/console/diagnostics/settings",
            get(diagnostics::get_settings).put(diagnostics::save_settings),
        )
        .route(
            "/v1/internal/gateway/console/diagnostics/export",
            post(diagnostics::export),
        )
}
