use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use time::OffsetDateTime;
use tokio::sync::mpsc;

use super::control::{effective_enabled, spawn_enabled_listener};
use super::mailbox::channel;
use super::native::NativeWatchOwner;
use super::work::PendingWatchWork;
use crate::state::AppState;

use super::super::status::update_watch_runtime_state;
use super::super::{
    folder_sync_root_available, resolve_root_dir, run_folder_sync_once,
    run_folder_sync_once_with_explicit_deletes, FolderSyncDirection,
};
use super::{start_folder_sync_watchers, FolderSyncWatchHandles, FolderWatchSignal};

const DELETION_INTENT_LIMIT_ERROR: &str =
    "provider credential folder watcher deletion intent exceeded its byte limit; explicit deletion was skipped";
const NATIVE_OWNER_OPERATION_DEADLINE: Duration = Duration::from_secs(10);

pub async fn start_folder_sync_task(state: Arc<AppState>) {
    let (control_task, mut control_rx) = spawn_enabled_listener(state.clone());
    let mut native_owner = None;
    // Keep the task alive until an admitted sync run finishes its status write.
    // Shutdown is observed at loop boundaries instead of cancelling the run future.
    run_folder_sync_task(&state, &mut native_owner, &mut control_rx).await;
    control_task.stop().await;
    if let Some(owner) = native_owner {
        if let Err(error) = owner
            .shutdown_with_deadline(NATIVE_OWNER_OPERATION_DEADLINE)
            .await
        {
            tracing::warn!(kind = ?error.kind, "failed to stop folder watcher native owner");
        }
    }
}

async fn run_folder_sync_task(
    state: &AppState,
    native_owner: &mut Option<NativeWatchOwner>,
    control_rx: &mut mpsc::Receiver<bool>,
) {
    if !folder_sync_root_available(&state.config) {
        tracing::info!("provider credential folder sync root dir is not configured");
        return;
    }

    // Subscribe before status I/O so enablement during startup remains observable.
    let mut enabled_rx = state.provider_credential_folder_sync.subscribe();
    let mut runtime_enabled = enabled_rx.borrow_and_update().enabled();
    if !runtime_enabled {
        tracing::info!(
            root_dir = ?state.config.provider_credential_folder_sync_root_dir,
            import_enabled = state.config.provider_credential_folder_sync_import_enabled,
            export_enabled = state.config.provider_credential_folder_sync_export_enabled,
            "provider credential folder sync runtime is disabled; waiting for enablement"
        );
        let Some(status_result) =
            update_watch_state_or_shutdown(state, false, false, None, None, false).await
        else {
            return;
        };
        if let Err(error) = status_result {
            tracing::warn!(
                error = %error.message,
                "failed to record disabled provider credential folder sync state"
            );
        }
        loop {
            runtime_enabled = enabled_rx.borrow_and_update().enabled();
            if runtime_enabled {
                break;
            }
            tokio::select! {
                biased;
                _ = state.shutdown.wait() => return,
                changed = enabled_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
                Some(enabled) = control_rx.recv() => {
                    state.provider_credential_folder_sync.set_enabled(effective_enabled(state, enabled));
                }
            }
        }
    }

    if state.shutdown.is_requested() {
        return;
    }

    let root_dir = match resolve_root_dir(&state.config) {
        Ok(root_dir) => root_dir,
        Err(error) => {
            tracing::warn!(
                error = %error.message,
                "provider credential folder sync root dir is not configured"
            );
            return;
        }
    };
    let interval_secs = state
        .config
        .provider_credential_folder_sync_interval_secs
        .max(5);
    let watch_enabled = state.config.provider_credential_folder_sync_watch_enabled;
    let watch_debounce_millis = state
        .config
        .provider_credential_folder_sync_watch_debounce_millis
        .max(250);
    let (watch_tx, mut watch_rx) = channel(state.provider_credential_folder_sync.clone());
    let signal_tx = watch_tx.clone();
    let configured_root = state
        .config
        .provider_credential_folder_sync_root_dir
        .clone();
    let import_enabled = state.config.provider_credential_folder_sync_import_enabled;
    let export_enabled = state.config.provider_credential_folder_sync_export_enabled;
    // Store ownership before waiting, so shutdown can drain even unfinished setup.
    *native_owner = match NativeWatchOwner::spawn(move || {
        if let Err(error) = std::fs::create_dir_all(&root_dir) {
            tracing::warn!(
                path = %root_dir.display(),
                error = %error,
                "failed to create provider credential folder sync root directory"
            );
            return None;
        }
        tracing::info!(
            interval_secs,
            watch_enabled,
            watch_debounce_millis,
            runtime_enabled,
            root_dir = ?configured_root,
            import_enabled,
            export_enabled,
            "starting provider credential folder sync task"
        );
        let handles = if watch_enabled {
            start_folder_sync_watchers(&root_dir, signal_tx, watch_debounce_millis)
        } else {
            FolderSyncWatchHandles {
                native: None,
                poll: None,
            }
        };
        let running = handles.is_running();
        Some((handles, running))
    }) {
        Ok(owner) => Some(owner),
        Err(error) => {
            tracing::warn!(kind = ?error.kind, "failed to start folder watcher native owner");
            return;
        }
    };
    let watch_handles = native_owner
        .as_mut()
        .expect("native owner stored before readiness");
    match watch_handles
        .ready_with_deadline(NATIVE_OWNER_OPERATION_DEADLINE)
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => return,
        Err(error) => {
            tracing::warn!(kind = ?error.kind, "folder watcher native initialization failed");
            return;
        }
    }
    if watch_enabled {
        let watch_running = watch_handles.is_running();
        let watch_error = (!watch_running).then_some(
            "failed to start provider credential folder watchers; falling back to periodic rescan",
        );
        let Some(status_result) = update_watch_state_or_shutdown(
            state,
            runtime_enabled,
            watch_running,
            None,
            watch_error,
            true,
        )
        .await
        else {
            return;
        };
        if let Err(status_error) = status_result {
            tracing::warn!(
                error = %status_error.message,
                "failed to record folder sync watch startup state"
            );
        }
    } else {
        let Some(status_result) =
            update_watch_state_or_shutdown(state, runtime_enabled, false, None, None, true).await
        else {
            return;
        };
        if let Err(error) = status_result {
            tracing::warn!(
                error = %error.message,
                "failed to record folder sync watch disabled state"
            );
        }
    }

    let (result, shutting_down) = run_sync_and_drain(
        state,
        run_folder_sync_once(state, FolderSyncDirection::Both),
    )
    .await;
    if let Err(error) = result {
        tracing::warn!(error = %error.message, "initial provider credential folder sync failed");
    }

    if shutting_down {
        return;
    }

    let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
    interval.tick().await;
    let mut work = PendingWatchWork::new(state.provider_credential_folder_sync.snapshot());
    loop {
        tokio::select! {
            biased;
            _ = state.shutdown.wait() => return,
            changed = enabled_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                let current = enabled_rx.borrow_and_update().clone();
                let runtime_enabled = current.enabled();
                work.reconcile(&current);
                if runtime_enabled {
                    let (result, shutting_down) = run_sync_and_drain(
                        state,
                        run_folder_sync_once_with_explicit_deletes(
                            state,
                            FolderSyncDirection::Both,
                            work.deleted_paths.as_set(),
                        ),
                    )
                    .await;
                    if let Err(error) = result {
                        tracing::warn!(error = %error.message, "runtime-enabled provider credential folder sync failed");
                    } else {
                        work.deleted_paths.clear();
                    }
                    if shutting_down {
                        return;
                    }
                }
                let Some(status_result) = update_watch_state_or_shutdown(
                    state,
                    runtime_enabled,
                    watch_handles.is_running(),
                    None,
                    None,
                    false,
                )
                .await
                else {
                    return;
                };
                if let Err(error) = status_result {
                    tracing::warn!(
                        error = %error.message,
                        "failed to record provider credential folder sync runtime toggle"
                    );
                }
            }
            Some(enabled) = control_rx.recv() => {
                state.provider_credential_folder_sync.set_enabled(effective_enabled(state, enabled));
            }
            _ = interval.tick() => {
                let current = state.provider_credential_folder_sync.snapshot();
                work.reconcile(&current);
                if !current.enabled() {
                    continue;
                }
                let (result, shutting_down) = run_sync_and_drain(
                    state,
                    run_folder_sync_once_with_explicit_deletes(
                        state,
                        FolderSyncDirection::Both,
                        work.deleted_paths.as_set(),
                    ),
                )
                .await;
                if let Err(error) = result {
                    tracing::warn!(error = %error.message, "provider credential folder sync periodic rescan failed");
                } else {
                    work.deleted_paths.clear();
                }
                if shutting_down {
                    return;
                }
            }
            _ = work.timer.ready() => {
                let current = state.provider_credential_folder_sync.snapshot();
                if !work.reconcile(&current) {
                    continue;
                }
                let (result, shutting_down) = run_sync_and_drain(
                    state,
                    run_folder_sync_once_with_explicit_deletes(
                        state,
                        FolderSyncDirection::Both,
                        work.deleted_paths.as_set(),
                    ),
                )
                .await;
                if let Err(error) = result {
                    tracing::warn!(error = %error.message, "provider credential folder sync watch-triggered run failed");
                } else {
                    work.deleted_paths.clear();
                }
                if shutting_down {
                    return;
                }
            }
            Some(received) = watch_rx.recv() => {
                let current = state.provider_credential_folder_sync.snapshot();
                work.reconcile(&current);
                if !received.is_current(&current) {
                    continue;
                }
                match received.signal {
                    FolderWatchSignal::FilesystemEvent(event) => {
                        work.merge_deleted_paths(event.deleted_paths);
                        if work.deletion_intent_overflowed() {
                            work.timer.cancel();
                            work.clear_deletion_intent();
                            let Some(status_result) = update_watch_state_or_shutdown(
                                state,
                                true,
                                watch_handles.is_running(),
                                Some(OffsetDateTime::now_utc()),
                                Some(DELETION_INTENT_LIMIT_ERROR),
                                false,
                            )
                            .await
                            else {
                                return;
                            };
                            if let Err(error) = status_result {
                                tracing::warn!(
                                    error = %error.message,
                                    "failed to record bounded folder watcher deletion intent error"
                                );
                            }
                            continue;
                        }
                        let Some(status_result) = update_watch_state_or_shutdown(
                            state,
                            true,
                            watch_handles.is_running(),
                            Some(OffsetDateTime::now_utc()),
                            None,
                            false,
                        )
                        .await
                        else {
                            return;
                        };
                        if let Err(error) = status_result {
                            tracing::warn!(
                                error = %error.message,
                                "failed to record provider credential folder watch event"
                            );
                        }
                        work.timer.schedule(Duration::from_millis(watch_debounce_millis));
                    }
                    FolderWatchSignal::WatcherError(message) => {
                        tracing::warn!(error = %message, "provider credential folder sync watcher error");
                        let runtime_enabled = state.provider_credential_folder_sync.enabled();
                        let Some(status_result) = update_watch_state_or_shutdown(
                            state,
                            runtime_enabled,
                            watch_handles.is_running(),
                            None,
                            Some(message.as_str()),
                            false,
                        )
                        .await
                        else {
                            return;
                        };
                        if let Err(error) = status_result {
                            tracing::warn!(
                                error = %error.message,
                                "failed to record provider credential watcher error"
                            );
                        }
                    }
                }
            }
        }
    }
}

async fn run_sync_and_drain(
    state: &AppState,
    operation: impl Future<
        Output = Result<
            super::super::ProviderCredentialFolderSyncStatusView,
            crate::error::GatewayError,
        >,
    >,
) -> (
    Result<super::super::ProviderCredentialFolderSyncStatusView, crate::error::GatewayError>,
    bool,
) {
    let mut operation = Box::pin(operation);
    let result = tokio::select! {
        biased;
        _ = state.shutdown.wait() => operation.await,
        result = &mut operation => result,
    };
    (result, state.shutdown.is_requested())
}

async fn update_watch_state_or_shutdown(
    state: &AppState,
    runtime_enabled: bool,
    watch_running: bool,
    last_watch_event_at: Option<OffsetDateTime>,
    last_watch_error: Option<&str>,
    drain_on_shutdown: bool,
) -> Option<Result<(), crate::error::GatewayError>> {
    if drain_on_shutdown {
        return Some(
            update_watch_runtime_state(
                &state.redis_pool,
                &state.config,
                runtime_enabled,
                watch_running,
                last_watch_event_at,
                last_watch_error,
            )
            .await,
        );
    }
    tokio::select! {
        biased;
        _ = state.shutdown.wait() => None,
        result = update_watch_runtime_state(
            &state.redis_pool,
            &state.config,
            runtime_enabled,
            watch_running,
            last_watch_event_at,
            last_watch_error,
        ) => Some(result),
    }
}
