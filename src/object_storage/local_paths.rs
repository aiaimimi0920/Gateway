use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use dashmap::DashMap;
use tokio::fs;
use tokio::sync::OwnedMutexGuard;

use crate::error::GatewayError;

pub(super) async fn local_operation_guard(root: &Path) -> OwnedMutexGuard<()> {
    static ROOT_LOCKS: OnceLock<DashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>> = OnceLock::new();
    let locks = ROOT_LOCKS.get_or_init(DashMap::new);
    let lock = locks
        .entry(root.to_path_buf())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone();
    lock.lock_owned().await
}

pub(crate) fn local_object_path(root: &Path, object_key: &str) -> Result<PathBuf, GatewayError> {
    validate_local_object_key(object_key)?;
    Ok(object_key
        .split('/')
        .fold(root.to_path_buf(), |path, segment| path.join(segment)))
}

fn validate_local_object_key(object_key: &str) -> Result<(), GatewayError> {
    if object_key.is_empty()
        || object_key.contains('\0')
        || object_key.contains('\\')
        || object_key.starts_with('/')
        || object_key.starts_with("//")
        || object_key
            .as_bytes()
            .get(1)
            .is_some_and(|byte| *byte == b':')
    {
        return Err(
            GatewayError::bad_request("invalid local object storage key")
                .with_code("object_storage_key_invalid"),
        );
    }

    let path = Path::new(object_key);
    if path.is_absolute() || path.has_root() {
        return Err(
            GatewayError::bad_request("invalid local object storage key")
                .with_code("object_storage_key_invalid"),
        );
    }

    if object_key.split('/').any(is_invalid_local_object_segment) {
        return Err(
            GatewayError::bad_request("invalid local object storage key")
                .with_code("object_storage_key_invalid"),
        );
    }
    Ok(())
}

fn is_invalid_local_object_segment(segment: &str) -> bool {
    if segment.is_empty()
        || segment == "."
        || segment == ".."
        || segment.contains(':')
        || segment.ends_with('.')
        || segment.ends_with(' ')
        || Path::new(segment)
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return true;
    }

    let device_stem = segment
        .split_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(segment);
    let upper_device_stem = device_stem.to_ascii_uppercase();
    matches!(upper_device_stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || upper_device_stem
            .strip_prefix("COM")
            .is_some_and(is_reserved_device_number)
        || upper_device_stem
            .strip_prefix("LPT")
            .is_some_and(is_reserved_device_number)
}

fn is_reserved_device_number(suffix: &str) -> bool {
    matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
}

pub(super) async fn ensure_local_path_if_root_exists(
    root: &Path,
    candidate: &Path,
) -> Result<(), GatewayError> {
    match fs::symlink_metadata(root).await {
        Ok(_) => ensure_local_path_confined(root, candidate).await,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(GatewayError::service_unavailable(format!(
            "inspect local object storage root: {error}"
        ))),
    }
}

pub(super) async fn ensure_local_path_confined(
    root: &Path,
    candidate: &Path,
) -> Result<(), GatewayError> {
    let canonical_root = fs::canonicalize(root).await.map_err(|error| {
        GatewayError::service_unavailable(format!("resolve local object storage root: {error}"))
    })?;
    let mut existing = candidate.to_path_buf();
    loop {
        match fs::symlink_metadata(&existing).await {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if !existing.pop() {
                    return Err(GatewayError::bad_request(
                        "local object storage path escapes configured root",
                    )
                    .with_code("object_storage_path_escape"));
                }
            }
            Err(error) => {
                return Err(GatewayError::service_unavailable(format!(
                    "inspect local object storage path: {error}"
                )))
            }
        }
    }

    let canonical_existing = fs::canonicalize(&existing).await.map_err(|error| {
        GatewayError::service_unavailable(format!("resolve local object storage path: {error}"))
    })?;
    if !canonical_existing.starts_with(&canonical_root) {
        return Err(
            GatewayError::bad_request("local object storage path escapes configured root")
                .with_code("object_storage_path_escape"),
        );
    }
    Ok(())
}
