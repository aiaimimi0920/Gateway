//! Guarded YAML replacement and recovery verify both current bytes and backup receipts.

use super::file_io::{
    read_regular_file, sha256_hex, sync_directory, unique_sibling, write_same_dir_temp,
};
use super::paths::{reject_existing_links, reject_link_metadata};
use super::validation::{validate_canonical_yaml, validate_safe_leaf};
use super::{
    PersistenceError, RouteConfigPersistence, TransactionPhase, TransactionRecord, WriterLockGuard,
    YamlReplaceReceipt, MOVEFILE_WRITE_THROUGH_FLAG,
};
use std::{fs, io};

impl YamlReplaceReceipt {
    pub fn previous_yaml_present(&self) -> bool {
        self.previous_yaml_present
    }

    pub fn previous_yaml_digest(&self) -> Option<&str> {
        self.previous_yaml_digest.as_deref()
    }

    pub fn new_yaml_digest(&self) -> &str {
        &self.new_yaml_digest
    }

    pub fn backup_leaf(&self) -> Option<&str> {
        self.backup_leaf.as_deref()
    }

    pub(crate) fn from_record_parts(
        previous_yaml_present: bool,
        previous_yaml_digest: Option<&str>,
        new_yaml_digest: &str,
        backup_leaf: Option<&str>,
    ) -> Self {
        Self {
            previous_yaml_present,
            previous_yaml_digest: previous_yaml_digest.map(str::to_string),
            new_yaml_digest: new_yaml_digest.to_string(),
            backup_leaf: backup_leaf.map(str::to_string),
        }
    }
}

impl RouteConfigPersistence {
    pub fn replace_routes_yaml(
        &self,
        canonical_yaml: &[u8],
    ) -> Result<YamlReplaceReceipt, PersistenceError> {
        self.ensure_writable()?;
        validate_canonical_yaml(canonical_yaml)?;
        let guard = self.try_writer_lock()?;
        self.replace_routes_yaml_locked(&guard, canonical_yaml)
    }

    pub fn replace_routes_yaml_for_transaction(
        &self,
        record: &TransactionRecord,
        canonical_yaml: &[u8],
    ) -> Result<YamlReplaceReceipt, PersistenceError> {
        self.ensure_writable()?;
        let guard = self.try_writer_lock()?;
        self.replace_routes_yaml_for_transaction_locked(&guard, record, canonical_yaml)
    }

    pub(crate) fn replace_routes_yaml_for_transaction_locked(
        &self,
        guard: &WriterLockGuard,
        record: &TransactionRecord,
        canonical_yaml: &[u8],
    ) -> Result<YamlReplaceReceipt, PersistenceError> {
        self.ensure_guard(guard)?;
        self.ensure_writable()?;
        record.validate()?;
        if record.phase() != TransactionPhase::Prepared {
            return Err(PersistenceError::invalid_phase(
                "A transaction YAML replacement must start in Prepared",
            ));
        }
        validate_canonical_yaml(canonical_yaml)?;
        let expected_digest = sha256_hex(canonical_yaml);
        if expected_digest != record.new_yaml_digest() {
            return Err(PersistenceError::corruption(
                "Transaction candidate YAML digest does not match its record",
            ));
        }
        self.ensure_prepared_previous_matches_locked(guard, record)?;
        self.replace_routes_yaml_locked_with_backup_leaf(
            guard,
            canonical_yaml,
            record.same_dir_backup_leaf(),
        )
    }

    pub fn restore_routes_yaml(
        &self,
        receipt: &YamlReplaceReceipt,
    ) -> Result<(), PersistenceError> {
        let guard = self.try_writer_lock()?;
        self.restore_routes_yaml_locked(&guard, receipt)
    }

    pub(crate) fn replace_routes_yaml_locked(
        &self,
        guard: &WriterLockGuard,
        canonical_yaml: &[u8],
    ) -> Result<YamlReplaceReceipt, PersistenceError> {
        self.ensure_guard(guard)?;
        self.ensure_writable()?;
        validate_canonical_yaml(canonical_yaml)?;
        reject_existing_links(&self.routes_path)?;
        self.ensure_first_save_backup_locked(guard)?;
        self.replace_routes_yaml_locked_with_backup_leaf(guard, canonical_yaml, None)
    }

    fn replace_routes_yaml_locked_with_backup_leaf(
        &self,
        guard: &WriterLockGuard,
        canonical_yaml: &[u8],
        forced_backup_leaf: Option<&str>,
    ) -> Result<YamlReplaceReceipt, PersistenceError> {
        self.ensure_guard(guard)?;
        self.ensure_writable()?;
        validate_canonical_yaml(canonical_yaml)?;
        reject_existing_links(&self.routes_path)?;
        self.ensure_first_save_backup_locked(guard)?;

        let parent = self.routes_path.parent().ok_or_else(|| {
            PersistenceError::new(
                "console_unsupported_platform_operation",
                "Configured routes path has no parent directory",
            )
        })?;
        let parent_metadata = fs::symlink_metadata(parent)
            .map_err(|error| PersistenceError::io("reading routes parent", error))?;
        reject_link_metadata(parent, &parent_metadata)?;
        if !parent_metadata.is_dir() {
            return Err(PersistenceError::corruption(
                "Configured routes parent is not a directory",
            ));
        }

        let (previous_yaml_present, previous_yaml_digest) =
            match fs::symlink_metadata(&self.routes_path) {
                Ok(metadata) => {
                    reject_link_metadata(&self.routes_path, &metadata)?;
                    if !metadata.is_file() {
                        return Err(PersistenceError::corruption(
                            "Configured routes path is not a regular file",
                        ));
                    }
                    let bytes = read_regular_file(&self.routes_path)?;
                    (true, Some(sha256_hex(&bytes)))
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => (false, None),
                Err(error) => return Err(PersistenceError::io("reading routes metadata", error)),
            };

        let temp_path = write_same_dir_temp(&self.routes_path, "yaml", canonical_yaml)?;
        let backup_path = if previous_yaml_present {
            if let Some(leaf) = forced_backup_leaf {
                validate_safe_leaf(leaf)?;
                Some(self.routes_path.parent().unwrap().join(leaf))
            } else {
                Some(unique_sibling(&self.routes_path, "bak"))
            }
        } else {
            None
        };
        let replace_result = if let Some(backup_path) = backup_path.as_deref() {
            self.replace_existing_with_backup(&self.routes_path, &temp_path, backup_path)
        } else {
            self.backend
                .move_new(&temp_path, &self.routes_path, MOVEFILE_WRITE_THROUGH_FLAG)
        };
        if let Err(error) = replace_result {
            // Keep the same-directory temp/backup evidence. Recovery must inspect an
            // interrupted platform operation rather than guessing or deleting the target.
            self.mark_degraded("console_recovery_required");
            return Err(PersistenceError::io(
                "atomically replacing routes YAML",
                error,
            ));
        }
        sync_directory(parent)?;

        let persisted = read_regular_file(&self.routes_path)?;
        let new_yaml_digest = sha256_hex(canonical_yaml);
        if persisted != canonical_yaml {
            self.mark_degraded("console_corruption");
            return Err(PersistenceError::corruption(
                "Atomic routes replacement produced unexpected bytes",
            ));
        }
        if let (Some(backup_path), Some(expected_digest)) =
            (backup_path.as_deref(), previous_yaml_digest.as_deref())
        {
            let backup = read_regular_file(backup_path)?;
            if sha256_hex(&backup) != expected_digest {
                self.mark_degraded("console_corruption");
                return Err(PersistenceError::corruption(
                    "Same-directory routes backup does not match the previous YAML digest",
                ));
            }
        }

        Ok(YamlReplaceReceipt {
            previous_yaml_present,
            previous_yaml_digest,
            new_yaml_digest,
            backup_leaf: backup_path
                .as_ref()
                .and_then(|path| path.file_name())
                .map(|leaf| leaf.to_string_lossy().into_owned()),
        })
    }

    fn ensure_prepared_previous_matches_locked(
        &self,
        guard: &WriterLockGuard,
        record: &TransactionRecord,
    ) -> Result<(), PersistenceError> {
        self.ensure_guard(guard)?;
        let actual = self.current_routes_state_locked(guard)?;
        if actual.0 != record.previous_yaml_present()
            || actual.1.as_deref() != record.previous_yaml_digest()
        {
            return Err(PersistenceError::new(
                "console_revision_conflict",
                "Prepared transaction previous YAML no longer matches the recorded state",
            ));
        }
        Ok(())
    }

    pub(crate) fn current_routes_state_locked(
        &self,
        guard: &WriterLockGuard,
    ) -> Result<(bool, Option<String>), PersistenceError> {
        self.ensure_guard(guard)?;
        match fs::symlink_metadata(&self.routes_path) {
            Ok(metadata) => {
                reject_link_metadata(&self.routes_path, &metadata)?;
                if !metadata.is_file() {
                    return Err(PersistenceError::recovery_required(
                        "Prepared transaction routes path is not a regular file",
                    ));
                }
                let bytes = read_regular_file(&self.routes_path)?;
                Ok((true, Some(sha256_hex(&bytes))))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok((false, None)),
            Err(error) => Err(PersistenceError::io("reading prepared routes state", error)),
        }
    }

    pub(crate) fn restore_routes_yaml_locked(
        &self,
        guard: &WriterLockGuard,
        receipt: &YamlReplaceReceipt,
    ) -> Result<(), PersistenceError> {
        self.ensure_guard(guard)?;
        let current = match fs::symlink_metadata(&self.routes_path) {
            Ok(metadata) => {
                reject_link_metadata(&self.routes_path, &metadata)?;
                Some(read_regular_file(&self.routes_path)?)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(PersistenceError::io("reading routes metadata", error)),
        };

        if receipt.previous_yaml_present {
            let previous_digest = receipt.previous_yaml_digest.as_deref().ok_or_else(|| {
                PersistenceError::corruption("Present previous YAML is missing its digest")
            })?;
            if current
                .as_deref()
                .is_some_and(|bytes| sha256_hex(bytes) == previous_digest)
            {
                return Ok(());
            }
            let current = current.ok_or_else(|| {
                PersistenceError::recovery_required(
                    "Routes YAML disappeared before the previous file could be restored",
                )
            })?;
            if sha256_hex(&current) != receipt.new_yaml_digest {
                return Err(PersistenceError::recovery_required(
                    "Routes YAML digest changed after replacement; recovery refused to overwrite it",
                ));
            }
            let leaf = receipt.backup_leaf.as_deref().ok_or_else(|| {
                PersistenceError::corruption("Present previous YAML is missing its backup leaf")
            })?;
            validate_safe_leaf(leaf)?;
            let backup_path = self.routes_path.parent().unwrap().join(leaf);
            let previous = read_regular_file(&backup_path)?;
            if sha256_hex(&previous) != previous_digest {
                return Err(PersistenceError::corruption(
                    "Routes recovery backup digest does not match the transaction record",
                ));
            }
            let temp_path = write_same_dir_temp(&self.routes_path, "restore", &previous)?;
            let displaced = unique_sibling(&self.routes_path, "rollback-bak");
            self.replace_existing_with_backup(&self.routes_path, &temp_path, &displaced)
                .map_err(|error| PersistenceError::recovery_required(error.to_string()))?;
            if let Err(error) = fs::remove_file(&displaced) {
                self.mark_degraded("console_recovery_required");
                return Err(PersistenceError::recovery_required(format!(
                    "Restored routes YAML but could not remove displaced recovery backup: {error}"
                )));
            }
            sync_directory(self.routes_path.parent().unwrap())?;
            if sha256_hex(&read_regular_file(&self.routes_path)?) != previous_digest {
                self.mark_degraded("console_corruption");
                return Err(PersistenceError::corruption(
                    "Restored routes YAML does not match the previous digest",
                ));
            }
            Ok(())
        } else {
            let Some(current) = current else {
                return Ok(());
            };
            if sha256_hex(&current) != receipt.new_yaml_digest {
                return Err(PersistenceError::recovery_required(
                    "Routes YAML digest changed after first managed write; recovery refused to remove it",
                ));
            }
            fs::remove_file(&self.routes_path).map_err(|error| {
                PersistenceError::recovery_required(format!(
                    "Failed to restore absent routes state: {error}"
                ))
            })?;
            sync_directory(self.routes_path.parent().unwrap())?;
            Ok(())
        }
    }

    pub(crate) fn install_routes_yaml_recovered_locked(
        &self,
        guard: &WriterLockGuard,
        canonical_yaml: &[u8],
    ) -> Result<(), PersistenceError> {
        let receipt = self.replace_routes_yaml_locked(guard, canonical_yaml)?;
        if let Some(leaf) = receipt.backup_leaf() {
            let parent = self.routes_path.parent().ok_or_else(|| {
                PersistenceError::new(
                    "console_unsupported_platform_operation",
                    "Configured routes path has no parent directory",
                )
            })?;
            let backup = parent.join(leaf);
            fs::remove_file(&backup).map_err(|error| {
                PersistenceError::recovery_required(format!(
                    "Recovered routes YAML but could not remove transient backup '{}': {error}",
                    backup.display()
                ))
            })?;
        }
        Ok(())
    }
}
