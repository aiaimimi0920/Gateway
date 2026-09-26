use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::mpsc::error::SendError;
use tokio::sync::Notify;

use crate::state::{ProviderCredentialFolderSyncRuntime, ProviderCredentialFolderSyncSnapshot};

use super::FolderWatchSignal;

struct State {
    pending: VecDeque<FolderWatchSignal>,
    epoch: ProviderCredentialFolderSyncSnapshot,
    senders: usize,
    receiver_open: bool,
}

impl State {
    fn discard_obsolete(
        &mut self,
        current: &ProviderCredentialFolderSyncSnapshot,
    ) -> Option<VecDeque<FolderWatchSignal>> {
        let changed = !self.epoch.same_epoch(current);
        if changed {
            self.epoch = current.clone();
        }
        (changed || !current.enabled()).then(|| std::mem::take(&mut self.pending))
    }
}

struct Shared {
    state: Mutex<State>,
    ready: Notify,
    runtime: ProviderCredentialFolderSyncRuntime,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }
}

pub(super) struct SignalSender(Arc<Shared>);
pub(super) struct SignalReceiver(Arc<Shared>);

pub(super) struct ReceivedSignal {
    snapshot: ProviderCredentialFolderSyncSnapshot,
    pub(super) signal: FolderWatchSignal,
}

impl ReceivedSignal {
    pub(super) fn is_current(&self, current: &ProviderCredentialFolderSyncSnapshot) -> bool {
        current.enabled() && self.snapshot.same_epoch(current)
    }
}

pub(super) fn channel(
    runtime: ProviderCredentialFolderSyncRuntime,
) -> (SignalSender, SignalReceiver) {
    let shared = Arc::new(Shared {
        state: Mutex::new(State {
            pending: VecDeque::with_capacity(2),
            epoch: runtime.snapshot(),
            senders: 1,
            receiver_open: true,
        }),
        ready: Notify::new(),
        runtime,
    });
    (SignalSender(Arc::clone(&shared)), SignalReceiver(shared))
}

fn kind(signal: &FolderWatchSignal) -> u8 {
    match signal {
        FolderWatchSignal::FilesystemEvent(_) => 0,
        FolderWatchSignal::WatcherError(_) => 1,
    }
}

impl SignalSender {
    pub(super) fn send_lazy<F>(&self, build: F)
    where
        F: FnOnce() -> FolderWatchSignal,
    {
        let active = {
            let mut state = self.0.lock();
            if !state.receiver_open {
                false
            } else {
                // Drop obsolete queued data before deciding whether the callback should allocate.
                let current = self.0.runtime.snapshot();
                let discarded = state.discard_obsolete(&current);
                let enabled = current.enabled();
                drop(state);
                drop(discarded);
                enabled
            }
        };
        if active {
            let _ = self.send(build());
        }
    }

    pub(super) fn send(
        &self,
        mut signal: FolderWatchSignal,
    ) -> Result<(), SendError<FolderWatchSignal>> {
        let mut state = self.0.lock();
        if !state.receiver_open {
            return Err(SendError(signal));
        }
        // Always lock mailbox before taking an owned runtime snapshot; no watch Ref escapes.
        let current = self.0.runtime.snapshot();
        let discarded = state.discard_obsolete(&current);
        if !current.enabled() {
            drop(state);
            drop(discarded);
            return Ok(());
        }
        if let Some(index) = state
            .pending
            .iter()
            .position(|old| kind(old) == kind(&signal))
        {
            let previous = state.pending.remove(index).expect("located pending signal");
            if let (
                FolderWatchSignal::FilesystemEvent(mut old),
                FolderWatchSignal::FilesystemEvent(new),
            ) = (previous, &mut signal)
            {
                // Retain every distinct deletion intent; never invent an ancestor deletion.
                old.deleted_paths
                    .merge(std::mem::take(&mut new.deleted_paths));
                new.deleted_paths = old.deleted_paths;
            }
        }
        // Latest-kind ordering preserves the final error/clear state. Two nodes
        // bound event amplification; distinct deleted-path bytes remain data-sized.
        state.pending.push_back(signal);
        drop(state);
        drop(discarded);
        self.0.ready.notify_one();
        Ok(())
    }
}

impl Clone for SignalSender {
    fn clone(&self) -> Self {
        self.0.lock().senders += 1;
        Self(Arc::clone(&self.0))
    }
}

impl Drop for SignalSender {
    fn drop(&mut self) {
        let last = {
            let mut state = self.0.lock();
            state.senders -= 1;
            state.senders == 0
        };
        if last {
            self.0.ready.notify_one();
        }
    }
}

impl SignalReceiver {
    pub(super) async fn recv(&mut self) -> Option<ReceivedSignal> {
        loop {
            let (signal, closed, discarded) = {
                let mut state = self.0.lock();
                let current = self.0.runtime.snapshot();
                let discarded = state.discard_obsolete(&current);
                let signal = state.pending.pop_front().map(|signal| ReceivedSignal {
                    snapshot: current,
                    signal,
                });
                let closed = state.senders == 0 || !state.receiver_open;
                (signal, closed, discarded)
            };
            // Old path sets can be large; release them outside the callback mutex.
            drop(discarded);
            if signal.is_some() || closed {
                return signal;
            }
            // A single receiver consumes Notify's saved permit even when the send
            // happened between the empty check and this await; cancellation is safe.
            self.0.ready.notified().await;
        }
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.0.lock().pending.len()
    }
}

impl Drop for SignalReceiver {
    fn drop(&mut self) {
        let pending = {
            let mut state = self.0.lock();
            state.receiver_open = false;
            std::mem::take(&mut state.pending)
        };
        // Free potentially large path sets after releasing the callback mutex.
        drop(pending);
    }
}

#[cfg(test)]
mod tests;
