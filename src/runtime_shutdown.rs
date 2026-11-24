//! HTTP completion owns worker draining before local finalizers, retirement and pool close.
use crate::state::AppState;
use std::sync::Arc;

pub(super) async fn finish(state: &Arc<AppState>) -> anyhow::Result<()> {
    state.lifecycle.begin_drain("http_server_stopped");
    state.shutdown.request("http_server_stopped");
    state
        .lifecycle
        .accio_db_refresh_stop
        .request("http_server_stopped");
    // Internal shutdown can precede a long HTTP drain. Keep heartbeats until serving actually ends.
    state
        .lifecycle
        .local_maintenance_stop
        .request("http_server_stopped");
    state.lifecycle.dim_oauth.join().await;
    state.lifecycle.luma_oauth.join().await;
    state.lifecycle.nous_oauth.join().await;
    state.lifecycle.lobsterai_oauth.join().await;
    state.lifecycle.quick_oauth.join().await;
    state.lifecycle.grok_oauth.join().await;
    state.lifecycle.accio_oauth.join().await;
    state.lifecycle.kiro_oauth.join().await;
    state.lifecycle.vertex_oauth.join().await;
    state.lifecycle.freebuff_login.join().await;
    crate::token_refresh::join(state).await;
    crate::provider_credential_refresh::accio::join(state).await;
    let background_result = state.lifecycle.background_tasks.join().await;
    state.lifecycle.stored_response_tasks.join().await;
    // A worker panic must not bypass finalizers, retirement or pool closure.
    if let Some(local) = &state.local_runtime {
        local.close().await;
    }
    background_result
}
