//! The first managed save keeps one immutable raw-byte backup and presence record.

use super::file_io::{
    create_new_bytes, create_new_json, read_json_limited, read_regular_file, sha256_hex,
    sync_directory,
};
use super::paths::reject_link_metadata;
use super::validation::is_lowercase_digest;
use super::{
    FirstSavePresence, FirstSaveStatus, PersistenceError, RouteConfigPersistence, WriterLockGuard,
    STATE_VERSION,
};
use std::{fs, io};

impl FirstSaveStatus {
    pub fn presence(&self) -> FirstSavePresence {
        self.presence
    }

    pub fn original_digest(&self) -> Option<&str> {
        self.original_digest.as_deref()
    }

    fn validate(&self) -> Result<(), PersistenceError> {
        if self.version != STATE_VERSION {
            return Err(PersistenceError::corruption(
                "Unsupported first-save status version",
            ));
        }
        match self.presence {
            FirstSavePresence::Present if !is_lowercase_digest(self.original_digest.as_deref()) => {
                Err(PersistenceError::corruption(
                    "Present first-save status has an invalid original digest",
                ))
            }
            FirstSavePresence::Absent if self.original_digest.is_some() => {
                Err(PersistenceError::corruption(
                    "Absent first-save status unexpectedly contains a digest",
                ))
            }
            _ => Ok(()),
        }
    }
}

impl RouteConfigPersistence {
    pub fn ensure_first_save_backup(&self) -> Result<FirstSaveStatus, PersistenceError> {
        self.ensure_writable()?;
        let guard = self.try_writer_lock()?;
        self.ensure_first_save_backup_locked(&guard)
    }

    pub(crate) fn ensure_first_save_backup_locked(
        &self,
        guard: &WriterLockGuard,
    ) -> Result<FirstSaveStatus, PersistenceError> {
        self.ensure_guard(guard)?;
        let status_path = self.first_save_status_path();
        let backup_path = self.first_save_backup_path();

        if status_path.exists() {
            let status: FirstSaveStatus = read_json_limited(&status_path)?;
            status.validate()?;
            self.verify_first_save_files(&status)?;
            return Ok(status);
        }

        if backup_path.exists() {
            let backup = read_regular_file(&backup_path)?;
            let current = read_regular_file(&self.routes_path).map_err(|_| {
                PersistenceError::corruption(
                    "First-save backup exists without status or matching routes file",
                )
            })?;
            if backup != current {
                return Err(PersistenceError::corruption(
                    "First-save backup exists without status and differs from routes YAML",
                ));
            }
            let status = FirstSaveStatus {
                version: STATE_VERSION,
                presence: FirstSavePresence::Present,
                original_digest: Some(sha256_hex(&backup)),
            };
            create_new_json(&status_path, &status)?;
            sync_directory(self.backups_dir().as_path())?;
            return Ok(status);
        }

        let status = match fs::symlink_metadata(&self.routes_path) {
            Ok(metadata) => {
                reject_link_metadata(&self.routes_path, &metadata)?;
                if !metadata.is_file() {
                    return Err(PersistenceError::corruption(
                        "Configured routes path is not a regular file",
                    ));
                }
                let original = read_regular_file(&self.routes_path)?;
                create_new_bytes(&backup_path, &original)?;
                FirstSaveStatus {
                    version: STATE_VERSION,
                    presence: FirstSavePresence::Present,
                    original_digest: Some(sha256_hex(&original)),
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => FirstSaveStatus {
                version: STATE_VERSION,
                presence: FirstSavePresence::Absent,
                original_digest: None,
            },
            Err(error) => return Err(PersistenceError::io("reading routes metadata", error)),
        };

        create_new_json(&status_path, &status)?;
        sync_directory(self.backups_dir().as_path())?;
        Ok(status)
    }

    fn verify_first_save_files(&self, status: &FirstSaveStatus) -> Result<(), PersistenceError> {
        let backup_path = self.first_save_backup_path();
        match status.presence {
            FirstSavePresence::Present => {
                let backup = read_regular_file(&backup_path).map_err(|_| {
                    PersistenceError::corruption(
                        "Present first-save status is missing its immutable backup",
                    )
                })?;
                if status.original_digest.as_deref() != Some(sha256_hex(&backup).as_str()) {
                    return Err(PersistenceError::corruption(
                        "First-save backup digest does not match its status",
                    ));
                }
            }
            FirstSavePresence::Absent if backup_path.exists() => {
                return Err(PersistenceError::corruption(
                    "Absent first-save status must not have a backup file",
                ));
            }
            FirstSavePresence::Absent => {}
        }
        Ok(())
    }
}
