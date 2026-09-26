use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::provider_credential_folder_sync::{folder_sync_root_available, load_runtime_enabled};
use crate::redis::keys;
use crate::state::AppState;

const RECONNECT_DELAY: Duration = Duration::from_secs(1);
const RECONCILE_TIMEOUT: Duration = Duration::from_secs(2);

pub(super) struct EnabledListenerTask {
    handle: JoinHandle<()>,
}

impl EnabledListenerTask {
    pub(super) async fn stop(self) {
        let mut task = self;
        task.handle.abort();
        let _ = (&mut task.handle).await;
    }
}

impl Drop for EnabledListenerTask {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

pub(super) fn spawn_enabled_listener(
    state: Arc<AppState>,
) -> (EnabledListenerTask, mpsc::Receiver<bool>) {
    let (tx, rx) = mpsc::channel(8);
    let task = tokio::spawn(run_enabled_listener(state, tx));
    (EnabledListenerTask { handle: task }, rx)
}

async fn run_enabled_listener(state: Arc<AppState>, tx: mpsc::Sender<bool>) {
    let client = match redis::Client::open(state.config.redis_url.as_str()) {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!(error = %error, "failed to create folder sync Redis control client");
            return;
        }
    };
    loop {
        if state.shutdown.is_requested() {
            return;
        }
        let mut pubsub = match client.get_async_pubsub().await {
            Ok(pubsub) => pubsub,
            Err(error) => {
                tracing::warn!(error = %error, "failed to connect folder sync Redis control listener");
                wait_before_reconnect(&state).await;
                continue;
            }
        };
        if let Err(error) = pubsub
            .subscribe(keys::provider_credential_folder_sync_events_channel())
            .await
        {
            tracing::warn!(error = %error, "failed to subscribe folder sync Redis control listener");
            wait_before_reconnect(&state).await;
            continue;
        }
        reconcile_enabled(&state, &tx).await;
        let mut messages = pubsub.on_message();
        loop {
            tokio::select! {
                biased;
                _ = state.shutdown.wait() => return,
                message = messages.next() => {
                    let Some(message) = message else { break };
                    let payload: String = match message.get_payload() {
                        Ok(payload) => payload,
                        Err(error) => {
                            tracing::warn!(error = %error, "invalid folder sync Redis control payload");
                            continue;
                        }
                    };
                    let enabled = match serde_json::from_str::<bool>(&payload) {
                        Ok(enabled) => enabled,
                        Err(error) => {
                            tracing::warn!(error = %error, "invalid folder sync enabled control value");
                            continue;
                        }
                    };
                    if tx.send(enabled).await.is_err() {
                        return;
                    }
                }
            }
        }
        wait_before_reconnect(&state).await;
    }
}

async fn reconcile_enabled(state: &AppState, tx: &mpsc::Sender<bool>) {
    let result = tokio::time::timeout(
        RECONCILE_TIMEOUT,
        load_runtime_enabled(&state.redis_pool, &state.config),
    )
    .await;
    match result {
        Ok(Ok(enabled)) => {
            let _ = tx.send(enabled).await;
        }
        Ok(Err(error)) => tracing::warn!(
            error = %error.message,
            "failed to reconcile folder sync enabled state after Redis control reconnect"
        ),
        Err(_) => tracing::warn!("folder sync Redis control reconciliation timed out"),
    }
}

async fn wait_before_reconnect(state: &AppState) {
    tokio::select! {
        _ = state.shutdown.wait() => {},
        _ = tokio::time::sleep(RECONNECT_DELAY) => {},
    }
}

pub(super) fn effective_enabled(state: &AppState, enabled: bool) -> bool {
    folder_sync_root_available(&state.config) && enabled
}
