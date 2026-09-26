use crate::state::ProviderCredentialFolderSyncSnapshot;

use super::super::path_budget::BoundedPathSet;
use super::debounce::DebounceTimer;

pub(super) struct PendingWatchWork {
    snapshot: ProviderCredentialFolderSyncSnapshot,
    pub(super) timer: DebounceTimer,
    pub(super) deleted_paths: BoundedPathSet,
}

impl PendingWatchWork {
    pub(super) fn new(snapshot: ProviderCredentialFolderSyncSnapshot) -> Self {
        Self {
            snapshot,
            timer: DebounceTimer::default(),
            deleted_paths: BoundedPathSet::default(),
        }
    }

    // Recheck in every selected branch; control notifications may coalesce or lose selection.
    pub(super) fn reconcile(&mut self, current: &ProviderCredentialFolderSyncSnapshot) -> bool {
        let same_epoch = self.snapshot.same_epoch(current);
        if !current.enabled() || !same_epoch {
            self.timer.cancel();
            self.deleted_paths.clear();
            self.snapshot = current.clone();
        }
        current.enabled() && same_epoch
    }

    pub(super) fn merge_deleted_paths(&mut self, paths: BoundedPathSet) {
        self.deleted_paths.merge(paths);
    }

    pub(super) fn deletion_intent_overflowed(&self) -> bool {
        self.deleted_paths.overflowed()
    }

    pub(super) fn clear_deletion_intent(&mut self) {
        self.deleted_paths.clear();
    }
}

#[cfg(test)]
mod tests;
