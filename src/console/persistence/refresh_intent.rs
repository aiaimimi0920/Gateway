//! Additive, immutable admission evidence; archives/journal remain the configuration authority.
use super::file_io::read_json_limited;
use super::{PersistenceError, RouteConfigPersistence, TransactionRecord, WriterLockGuard};
use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf};

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RefreshIntent {
    version: u32,
    pub(crate) prepared: TransactionRecord,
    pub(crate) provider: String,
    pub(crate) credential: String,
    pub(crate) parent_document_digest: String,
    pub(crate) new_document_digest: String,
}

impl RefreshIntent {
    pub(crate) fn new(
        record: &TransactionRecord,
        provider: &str,
        credential: &str,
        parent_digest: &str,
        new_digest: &str,
    ) -> Result<Self, PersistenceError> {
        Ok(Self {
            version: 1,
            prepared: record.prepared_identity()?,
            provider: provider.into(),
            credential: credential.into(),
            parent_document_digest: parent_digest.into(),
            new_document_digest: new_digest.into(),
        })
    }

    fn validate(&self, record: &TransactionRecord) -> Result<(), PersistenceError> {
        if self.version != 1 || self.prepared != record.prepared_identity()? {
            return Err(PersistenceError::recovery_required(
                "Refresh intent identity does not match the transaction",
            ));
        }
        Ok(())
    }
}

impl RouteConfigPersistence {
    fn refresh_intent_path(&self, tx_id: &str) -> Result<PathBuf, PersistenceError> {
        // Reuse the journal's safe transaction ID boundary, never accept an external path.
        self.transaction_path(tx_id)?;
        Ok(self
            .state_root()
            .join("refresh-intents")
            .join(format!("{tx_id}.json")))
    }

    pub(crate) fn load_refresh_intent_locked(
        &self,
        guard: &WriterLockGuard,
        record: &TransactionRecord,
    ) -> Result<RefreshIntent, PersistenceError> {
        self.ensure_guard(guard)?;
        let intent: RefreshIntent = read_json_limited(&self.refresh_intent_path(record.tx_id())?)?;
        intent.validate(record)?;
        Ok(intent)
    }

    pub(crate) fn store_refresh_intent_locked(
        &self,
        guard: &WriterLockGuard,
        intent: &RefreshIntent,
    ) -> Result<(), PersistenceError> {
        self.ensure_guard(guard)?;
        self.ensure_writable()?;
        intent.validate(&intent.prepared)?;
        let path = self.refresh_intent_path(intent.prepared.tx_id())?;
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                let existing: RefreshIntent = read_json_limited(&path)?;
                if existing != *intent {
                    return Err(PersistenceError::corruption(
                        "An immutable refresh intent already differs",
                    ));
                }
                Ok(())
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // The normal atomic publisher uses non-replacing move_new for this absent path.
                self.atomic_write_json_locked(guard, &path, intent)
            }
            Err(error) => Err(PersistenceError::io(
                "reading refresh intent metadata",
                error,
            )),
        }
    }
}
