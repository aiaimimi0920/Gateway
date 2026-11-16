//! Reuse the console's storage owner and blocking IO boundary; no tokens are persisted here.
use crate::{
    console::persistence::{LumaRegistrationRecord, PersistenceError, RouteConfigPersistence},
    error::GatewayError,
    state::AppState,
};

pub(super) fn persistence(state: &AppState) -> Result<RouteConfigPersistence, GatewayError> {
    state
        .route_config_runtime
        .as_ref()
        .and_then(|r| r.coordinator())
        .map(|c| c.registration_persistence())
        .ok_or_else(|| {
            GatewayError::service_unavailable("Writable console runtime is required")
                .with_code("console_mutation_not_supported")
        })
}

async fn blocking<T: Send + 'static>(
    action: impl FnOnce() -> Result<T, PersistenceError> + Send + 'static,
) -> Result<T, PersistenceError> {
    tokio::task::spawn_blocking(action)
        .await
        .map_err(|error| PersistenceError::io("joining Luma receipt storage", error))?
}

pub(super) async fn load(state: &AppState) -> Result<Vec<LumaRegistrationRecord>, GatewayError> {
    let store = persistence(state)?;
    blocking(move || {
        let guard = store.try_writer_lock()?;
        store.load_luma_registrations_locked(&guard)
    })
    .await
    .map_err(|error| {
        tracing::warn!(
            code = error.code(),
            "Luma registration receipts unavailable"
        );
        GatewayError::service_unavailable("Luma registration receipt storage unavailable")
            .with_code("luma_registration_store_unavailable")
    })
}

pub(super) async fn claim(
    store: RouteConfigPersistence,
    record: LumaRegistrationRecord,
) -> Result<bool, PersistenceError> {
    blocking(move || {
        let guard = store.try_writer_lock()?;
        store.claim_luma_registration_locked(&guard, &record)
    })
    .await
}

pub(super) async fn finish(
    store: RouteConfigPersistence,
    record: LumaRegistrationRecord,
    client: String,
) -> Result<(), PersistenceError> {
    blocking(move || {
        let guard = store.try_writer_lock()?;
        store.finish_luma_registration_locked(&guard, &record, client)
    })
    .await
}
