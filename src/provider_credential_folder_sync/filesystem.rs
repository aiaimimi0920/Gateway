//! Folder material discovery, retrying JSON reads and byte fingerprints.
//! Incomplete or redirected discovery must fail before missing-file reconciliation.
use super::limits::{is_material_capacity_error, scan_limit, ScanLimits, MATERIAL_BYTES};
use super::paths::{self, SyncRoot};
use crate::config::Config;
use crate::error::GatewayError;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;

mod export;
mod material;
pub(super) use export::{delete_stale_files, write_export_file};
#[cfg(test)]
pub(super) use export::{delete_stale_files_with_limits, write_export_file_with_limit};
#[cfg(test)]
pub(super) use material::read_bounded;
pub(super) use material::{read_material_bytes, serialize_export_payload};

pub(super) async fn read_provider_credential_json_with_retry(
    root_dir: &Path,
    file_path: &Path,
) -> Result<(String, Value), GatewayError> {
    let mut last_error: Option<GatewayError> = None;
    let root = SyncRoot::new(root_dir)?;
    let relative = file_path
        .strip_prefix(root_dir)
        .map_err(|_| paths::invalid_path())?;
    for attempt in 0..5 {
        // Retrying partial writes must not follow a link introduced between attempts.
        let result = {
            let operation_lock = paths::root_operation_lock(root_dir);
            let _operation_guard = operation_lock
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let checked = root.resolve(relative)?;
            read_provider_credential_json_once(&checked)
        };
        match result {
            Ok(result) => return Ok(result),
            Err(error) if is_material_capacity_error(&error) => return Err(error),
            Err(error) => {
                last_error = Some(error);
                if attempt < 4 {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            }
        }
    }
    Err(last_error.unwrap_or_else(|| {
        GatewayError::server_error(format!(
            "read provider credential file {} failed without concrete error",
            file_path.display()
        ))
    }))
}

fn read_provider_credential_json_once(file_path: &Path) -> Result<(String, Value), GatewayError> {
    let bytes = read_material_bytes(file_path, MATERIAL_BYTES)?;
    let raw_payload: Value = serde_json::from_slice(&bytes).map_err(|error| {
        GatewayError::bad_request(format!(
            "provider credential file {} is not valid JSON: {error}",
            file_path.display()
        ))
    })?;
    if !raw_payload.is_object() {
        return Err(GatewayError::bad_request(format!(
            "provider credential file {} 必须是 JSON object",
            file_path.display()
        )));
    }
    Ok((sha256_hex(&bytes), raw_payload))
}

pub(super) fn resolve_root_dir(config: &Config) -> Result<PathBuf, GatewayError> {
    config
        .provider_credential_folder_sync_root_dir
        .as_ref()
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| GatewayError::service_unavailable("未配置服务商凭证文件夹同步根目录"))
}

pub(super) fn collect_json_files(root_dir: &Path) -> Result<Vec<PathBuf>, GatewayError> {
    collect_json_files_with_limits(root_dir, ScanLimits::default())
}

pub(super) fn collect_json_files_with_limits(
    root_dir: &Path,
    limits: ScanLimits,
) -> Result<Vec<PathBuf>, GatewayError> {
    let operation_lock = paths::root_operation_lock(root_dir);
    let _operation_guard = operation_lock
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let root = SyncRoot::new(root_dir)?;
    let mut files = Vec::new();
    let mut entries_seen = 0;
    let mut directories = vec![(root.path().to_path_buf(), read_directory(root.path())?)];
    // Preserve depth-first read_dir order without growing the call stack.
    while let Some((current_dir, entries)) = directories.last_mut() {
        let Some(entry) = entries.next() else {
            directories.pop();
            continue;
        };
        // Count every entry, not only JSON, before inspecting or opening descendants.
        if entries_seen >= limits.max_entries {
            return Err(scan_limit("entries", limits.max_entries));
        }
        entries_seen += 1;
        let entry = entry.map_err(|error| {
            GatewayError::server_error(format!(
                "read provider credential directory entry {}: {error}",
                current_dir.display()
            ))
        })?;
        let path = entry.path();
        let relative = path
            .strip_prefix(root.path())
            .map_err(|_| paths::invalid_path())?;
        let checked = root.resolve(relative)?;
        let metadata = std::fs::symlink_metadata(&checked).map_err(|error| {
            GatewayError::server_error(format!(
                "inspect provider credential file {}: {error}",
                checked.display()
            ))
        })?;
        if metadata.is_dir() {
            if directories.len() > limits.max_depth {
                return Err(scan_limit("directory depth", limits.max_depth));
            }
            let children = read_directory(&checked)?;
            directories.push((checked, children));
        } else if path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            if files.len() >= limits.max_files {
                return Err(scan_limit("JSON files", limits.max_files));
            }
            files.push(root_dir.join(relative));
        }
    }
    Ok(files)
}

fn read_directory(path: &Path) -> Result<std::fs::ReadDir, GatewayError> {
    std::fs::read_dir(path).map_err(|error| {
        GatewayError::server_error(format!(
            "read provider credential directory {}: {error}",
            path.display()
        ))
    })
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}
