//! Transaction JSON publication keeps uncertain replacement failures in recovery mode.

use super::file_io::{sync_directory, unique_sibling, write_same_dir_temp};
use super::paths::reject_link_metadata;
use super::{
    PersistenceError, RouteConfigPersistence, WriterLockGuard, MOVEFILE_WRITE_THROUGH_FLAG,
};
use serde::Serialize;
use std::path::Path;
use std::{fs, io};

impl RouteConfigPersistence {
    pub(crate) fn atomic_write_json_locked<T: Serialize>(
        &self,
        guard: &WriterLockGuard,
        destination: &Path,
        value: &T,
    ) -> Result<(), PersistenceError> {
        self.ensure_guard(guard)?;
        let mut bytes = serde_json::to_vec(value)
            .map_err(|error| PersistenceError::io("serializing transaction JSON", error))?;
        bytes.push(b'\n');
        let parent = destination.parent().ok_or_else(|| {
            PersistenceError::new(
                "console_unsupported_platform_operation",
                "Transaction path has no parent directory",
            )
        })?;
        match fs::symlink_metadata(destination) {
            Ok(metadata) => {
                reject_link_metadata(destination, &metadata)?;
                if !metadata.is_file() {
                    return Err(PersistenceError::corruption(
                        "Transaction record path is not a regular file",
                    ));
                }
                let temp_path = write_same_dir_temp(destination, "json", &bytes)?;
                let backup = unique_sibling(destination, "json-bak");
                if let Err(error) =
                    self.replace_existing_with_backup(destination, &temp_path, &backup)
                {
                    self.mark_degraded("console_recovery_required");
                    return Err(PersistenceError::recovery_required(format!(
                        "Transaction JSON replacement has uncertain durable state: {error}"
                    )));
                }
                if let Err(error) = fs::remove_file(&backup) {
                    self.mark_degraded("console_recovery_required");
                    return Err(PersistenceError::recovery_required(format!(
                        "Transaction JSON updated but its temporary backup could not be removed: {error}"
                    )));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let temp_path = write_same_dir_temp(destination, "json", &bytes)?;
                if let Err(error) =
                    self.backend
                        .move_new(&temp_path, destination, MOVEFILE_WRITE_THROUGH_FLAG)
                {
                    self.mark_degraded("console_recovery_required");
                    return Err(PersistenceError::io(
                        "creating transaction JSON atomically",
                        error,
                    ));
                }
            }
            Err(error) => return Err(PersistenceError::io("reading transaction metadata", error)),
        }
        sync_directory(parent)?;
        Ok(())
    }
}
