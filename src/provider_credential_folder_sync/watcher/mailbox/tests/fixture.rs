use crate::state::ProviderCredentialFolderSyncRuntime;

use super::super::{channel as runtime_channel, SignalReceiver, SignalSender};
use super::FolderWatchSignal;

// Preserve the original protocol assertions while production also returns an epoch envelope.
pub(super) struct Receiver(SignalReceiver);

pub(super) fn channel() -> (SignalSender, Receiver) {
    let (tx, rx) = runtime_channel(ProviderCredentialFolderSyncRuntime::new(true));
    (tx, Receiver(rx))
}

impl Receiver {
    pub(super) async fn recv(&mut self) -> Option<FolderWatchSignal> {
        self.0.recv().await.map(|received| received.signal)
    }

    pub(super) fn len(&self) -> usize {
        self.0.len()
    }
}
