use std::path::Path;
use std::time::Duration;

use notify::{
    Config as NotifyConfig, Event, EventKind, PollWatcher, RecommendedWatcher, RecursiveMode,
    Watcher,
};

use crate::error::GatewayError;

use super::deletion::deleted_relative_paths_from_event;
use super::path_budget::BoundedPathSet;

mod control;
mod debounce;
mod mailbox;
mod native;
mod runtime;
mod work;
use mailbox::SignalSender;
pub use runtime::start_folder_sync_task;

#[derive(Debug, Clone)]
struct FolderWatchEvent {
    deleted_paths: BoundedPathSet,
}

impl FolderWatchEvent {
    fn from_event(root_dir: &Path, event: &Event) -> Self {
        Self {
            deleted_paths: deleted_relative_paths_from_event(root_dir, event),
        }
    }
}

#[derive(Debug, Clone)]
enum FolderWatchSignal {
    FilesystemEvent(FolderWatchEvent),
    WatcherError(String),
}

struct FolderSyncWatchHandles {
    native: Option<RecommendedWatcher>,
    poll: Option<PollWatcher>,
}

impl FolderSyncWatchHandles {
    fn is_running(&self) -> bool {
        self.native.is_some() || self.poll.is_some()
    }
}

fn start_folder_sync_watchers(
    root_dir: &Path,
    signal_tx: SignalSender,
    watch_debounce_millis: u64,
) -> FolderSyncWatchHandles {
    let native = match start_native_folder_sync_watcher(root_dir, signal_tx.clone()) {
        Ok(watcher) => Some(watcher),
        Err(error) => {
            tracing::warn!(
                error = %error.message,
                path = %root_dir.display(),
                "failed to start native provider credential folder watcher"
            );
            None
        }
    };
    let poll_interval = Duration::from_millis(watch_debounce_millis.max(1000));
    let poll = match start_poll_folder_sync_watcher(root_dir, signal_tx, poll_interval) {
        Ok(watcher) => Some(watcher),
        Err(error) => {
            tracing::warn!(
                error = %error.message,
                path = %root_dir.display(),
                "failed to start poll fallback provider credential folder watcher"
            );
            None
        }
    };
    FolderSyncWatchHandles { native, poll }
}

fn start_native_folder_sync_watcher(
    root_dir: &Path,
    signal_tx: SignalSender,
) -> Result<RecommendedWatcher, GatewayError> {
    let callback_tx = signal_tx.clone();
    let watch_root = root_dir.to_path_buf();
    let mut watcher =
        notify::recommended_watcher(move |result: Result<Event, notify::Error>| match result {
            Ok(event) => {
                if folder_sync_event_should_trigger(&event) {
                    callback_tx.send_lazy(|| {
                        FolderWatchSignal::FilesystemEvent(FolderWatchEvent::from_event(
                            watch_root.as_path(),
                            &event,
                        ))
                    });
                }
            }
            Err(error) => {
                callback_tx.send_lazy(|| FolderWatchSignal::WatcherError(error.to_string()));
            }
        })
        .map_err(|error| {
            GatewayError::server_error(format!(
                "create provider credential folder watcher for {}: {error}",
                root_dir.display()
            ))
        })?;
    watcher
        .watch(root_dir, RecursiveMode::Recursive)
        .map_err(|error| {
            GatewayError::server_error(format!(
                "watch provider credential folder {}: {error}",
                root_dir.display()
            ))
        })?;
    Ok(watcher)
}

fn start_poll_folder_sync_watcher(
    root_dir: &Path,
    signal_tx: SignalSender,
    poll_interval: Duration,
) -> Result<PollWatcher, GatewayError> {
    let callback_tx = signal_tx.clone();
    let watch_root = root_dir.to_path_buf();
    let mut watcher = PollWatcher::new(
        move |result: Result<Event, notify::Error>| match result {
            Ok(event) => {
                if folder_sync_event_should_trigger(&event) {
                    callback_tx.send_lazy(|| {
                        FolderWatchSignal::FilesystemEvent(FolderWatchEvent::from_event(
                            watch_root.as_path(),
                            &event,
                        ))
                    });
                }
            }
            Err(error) => {
                callback_tx.send_lazy(|| FolderWatchSignal::WatcherError(error.to_string()));
            }
        },
        NotifyConfig::default().with_poll_interval(poll_interval),
    )
    .map_err(|error| {
        GatewayError::server_error(format!(
            "create poll provider credential folder watcher for {}: {error}",
            root_dir.display()
        ))
    })?;
    watcher
        .watch(root_dir, RecursiveMode::Recursive)
        .map_err(|error| {
            GatewayError::server_error(format!(
                "watch provider credential folder with poll watcher {}: {error}",
                root_dir.display()
            ))
        })?;
    Ok(watcher)
}

fn folder_sync_event_should_trigger(event: &Event) -> bool {
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }
    if event.paths.is_empty() {
        return true;
    }
    event
        .paths
        .iter()
        .any(|path| folder_sync_path_should_trigger(path.as_path()))
}

pub(super) fn folder_sync_path_should_trigger(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => ext.eq_ignore_ascii_case("json"),
        None => true,
    }
}

#[cfg(test)]
mod tests;
