//! Durable regular-file, JSON and same-directory staging primitives preserve flush ordering.

use super::paths::{is_reparse_point, reject_link_metadata};
use super::{PersistenceError, TransactionRecord};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_METADATA_BYTES: u64 = 1024 * 1024;

pub(super) fn read_json_limited<T: for<'de> Deserialize<'de>>(
    path: &Path,
) -> Result<T, PersistenceError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| PersistenceError::io("reading JSON metadata", error))?;
    reject_link_metadata(path, &metadata)?;
    if !metadata.is_file() || metadata.len() > MAX_METADATA_BYTES {
        return Err(PersistenceError::corruption(
            "Console persistence JSON is not a bounded regular file",
        ));
    }
    let bytes = read_regular_file(path)?;
    serde_json::from_slice(&bytes)
        .map_err(|_| PersistenceError::corruption("Console persistence JSON is malformed"))
}

pub(crate) fn read_transaction_json(path: &Path) -> Result<TransactionRecord, PersistenceError> {
    let record: TransactionRecord = read_json_limited(path)?;
    record.validate()?;
    validate_transaction_record_path(path, &record)?;
    Ok(record)
}

fn validate_transaction_record_path(
    path: &Path,
    record: &TransactionRecord,
) -> Result<(), PersistenceError> {
    if path.extension().and_then(|value| value.to_str()) != Some("json") {
        return Err(PersistenceError::corruption(
            "Transaction record path must use the .json extension",
        ));
    }
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            PersistenceError::corruption(
                "Transaction record path must have a valid UTF-8 file stem",
            )
        })?;
    if stem != record.tx_id() {
        return Err(PersistenceError::corruption(
            "Transaction record filename does not match its tx_id",
        ));
    }
    Ok(())
}

pub(super) fn read_regular_file(path: &Path) -> Result<Vec<u8>, PersistenceError> {
    read_regular_file_io(path).map_err(|error| PersistenceError::io("reading durable file", error))
}

pub(super) fn read_regular_file_io(path: &Path) -> io::Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) || !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path is not a non-link regular file",
        ));
    }
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub(super) fn create_new_json<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), PersistenceError> {
    let mut bytes = serde_json::to_vec(value)
        .map_err(|error| PersistenceError::io("serializing durable JSON", error))?;
    bytes.push(b'\n');
    create_new_bytes(path, &bytes)
}

pub(super) fn create_new_bytes(path: &Path, bytes: &[u8]) -> Result<(), PersistenceError> {
    create_new_bytes_io(path, bytes)
        .map_err(|error| PersistenceError::io("creating durable file", error))
}

pub(super) fn create_new_bytes_io(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.flush()?;
    file.sync_all()?;
    drop(file);
    Ok(())
}

pub(super) fn write_same_dir_temp(
    target: &Path,
    label: &str,
    bytes: &[u8],
) -> Result<PathBuf, PersistenceError> {
    let parent = target.parent().ok_or_else(|| {
        PersistenceError::new(
            "console_unsupported_platform_operation",
            "Atomic target has no parent directory",
        )
    })?;
    for _ in 0..8 {
        let path = parent.join(format!(".gateway-console-{}-{label}.tmp", Uuid::new_v4()));
        match create_new_bytes_io(&path, bytes) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(PersistenceError::io("writing same-directory temp", error)),
        }
    }
    Err(PersistenceError::io(
        "allocating same-directory temp",
        "exhausted unpredictable file names",
    ))
}

pub(super) fn unique_sibling(target: &Path, label: &str) -> PathBuf {
    target
        .parent()
        .unwrap()
        .join(format!(".gateway-console-{}-{label}", Uuid::new_v4()))
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(unix)]
pub(super) fn sync_directory(path: &Path) -> Result<(), PersistenceError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| PersistenceError::io("syncing parent directory", error))
}

#[cfg(windows)]
pub(super) fn sync_directory(_path: &Path) -> Result<(), PersistenceError> {
    Ok(())
}
