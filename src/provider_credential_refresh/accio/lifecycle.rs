//! A single runtime-owned Accio DB worker is disabled in local mode and joined on shutdown.
use super::Worker;
use crate::state::AppState;
use std::{sync::Arc, time::Duration};

pub(crate) fn spawn(state: &Arc<AppState>) {
    if !cfg!(feature = "line-accio-web-reverse-api")
        || state.local_runtime.is_some()
        || !state.config.provider_credential_refresh_enabled
        || state.shutdown.is_requested()
        || state.lifecycle.accio_db_refresh_stop.is_requested()
    {
        return;
    }
    let Some(pool) = &state.pg_pool else {
        return;
    };
    let mut task = state.lifecycle.accio_db_refresh_task.lock();
    if task.is_some() {
        return;
    }
    let worker = Worker::new(
        pool.clone(),
        state.upstream_client.client().clone(),
        state.lifecycle.accio_db_refresh_stop.clone(),
        state.config.provider_credential_refresh_batch_limit,
        Duration::from_secs(state.config.provider_credential_refresh_before_secs),
    );
    let interval = Duration::from_secs(
        state
            .config
            .provider_credential_refresh_interval_secs
            .max(60),
    );
    let work = worker.run(interval);
    // The test-only peer changes transport, not persisted audience or spawn/join ownership.
    #[cfg(all(test, feature = "line-accio-web-reverse-api"))]
    if let Some(origin) = super::server_peer::selected_origin() {
        *task = Some(tokio::spawn(
            crate::protocol::accio_oauth::TEST_REFRESH_ENDPOINT
                .scope(format!("{origin}/oauth/refresh"), work),
        ));
        return;
    }
    *task = Some(tokio::spawn(work));
}

pub(crate) async fn join(state: &AppState) {
    state
        .lifecycle
        .accio_db_refresh_stop
        .request("accio_db_refresh_join");
    let task = state.lifecycle.accio_db_refresh_task.lock().take();
    if let Some(task) = task {
        if task.await.is_err() {
            tracing::error!("Accio DB refresh task did not exit cleanly");
        }
    }
}
