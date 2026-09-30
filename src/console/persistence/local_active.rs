//! Durable local authority reuses the same protected archive and atomic writer boundary.
use super::file_io::read_json_limited;
use super::{validate_safe_id, PersistenceError, RouteConfigPersistence, WriterLockGuard};

impl RouteConfigPersistence {
    pub(crate) fn local_active_revision(&self) -> Result<Option<String>, PersistenceError> {
        let path = self.state_root.join("local-active.json");
        match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(PersistenceError::io("reading local active revision", error)),
            Ok(_) => {
                let id: String = read_json_limited(&path)?;
                validate_safe_id("revision", &id)?;
                Ok(Some(id))
            }
        }
    }

    pub(crate) fn write_local_active_locked(
        &self,
        guard: &WriterLockGuard,
        revision: &str,
    ) -> Result<(), PersistenceError> {
        validate_safe_id("revision", revision)?;
        self.atomic_write_json_locked(guard, &self.state_root.join("local-active.json"), &revision)
    }
}
