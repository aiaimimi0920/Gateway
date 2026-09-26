//! Filesystem effects of export, independent of database metadata persistence.
use super::{collect_json_files_with_limits, read_material_bytes, sha256_hex};
use crate::error::GatewayError;
use crate::provider_credential_folder_sync::layout::normalize_relative_path;
use crate::provider_credential_folder_sync::limits::{
    ensure_material_size, is_material_capacity_error, ScanLimits, MATERIAL_BYTES,
};
use crate::provider_credential_folder_sync::paths::{self, SyncRoot};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub(in crate::provider_credential_folder_sync) fn write_export_file(
    root_dir: &Path,
    relative: &str,
    serialized: &[u8],
) -> Result<(String, String, bool), GatewayError> {
    write_export_file_with_limit(root_dir, relative, serialized, MATERIAL_BYTES)
}

pub(in crate::provider_credential_folder_sync) fn write_export_file_with_limit(
    root_dir: &Path,
    relative: &str,
    serialized: &[u8],
    limit: usize,
) -> Result<(String, String, bool), GatewayError> {
    let operation_lock = paths::root_operation_lock(root_dir);
    let _operation_guard = operation_lock
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    ensure_material_size(serialized.len() as u64, limit)?;
    // Validate raw source_path before normalization can hide rooted or ambiguous input.
    let path = paths::validated_relative_path(relative)?.ok_or_else(paths::invalid_path)?;
    let relative = paths::relative_key(&path)?;
    let root = SyncRoot::new(root_dir)?;
    let absolute = root.resolve(&path)?;
    if let Some(parent) = absolute.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            GatewayError::server_error(format!(
                "create provider credential directory {}: {error}",
                parent.display()
            ))
        })?;
    }
    let absolute = root.resolve(&path)?;
    let hash = sha256_hex(serialized);
    let needs_write = match read_material_bytes(&absolute, limit) {
        Ok(existing) => sha256_hex(&existing) != hash,
        Err(error) if is_material_capacity_error(&error) => return Err(error),
        Err(_) => true,
    };
    if needs_write {
        let temporary_path = temporary_relative_path(&path)?;
        let temporary = root.resolve(&temporary_path)?;
        if let Err(error) = std::fs::write(&temporary, serialized) {
            let _ = std::fs::remove_file(&temporary);
            return Err(GatewayError::server_error(format!(
                "write provider credential file {}: {error}",
                absolute.display()
            )));
        }
        let absolute = root.resolve(&path)?;
        if let Err(error) = std::fs::rename(&temporary, &absolute) {
            let _ = std::fs::remove_file(&temporary);
            return Err(GatewayError::server_error(format!(
                "replace provider credential file {}: {error}",
                absolute.display()
            )));
        }
    }
    Ok((relative, hash, needs_write))
}

fn temporary_relative_path(path: &Path) -> Result<PathBuf, GatewayError> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(paths::invalid_path)?;
    let mut temporary = path.to_path_buf();
    temporary.set_file_name(format!(".{file_name}.tmp-{}", Uuid::new_v4()));
    Ok(temporary)
}

pub(in crate::provider_credential_folder_sync) fn delete_stale_files(
    root_dir: &Path,
    expected_paths: &HashSet<String>,
    deleted_count: &mut usize,
) -> Result<(), GatewayError> {
    delete_stale_files_with_limits(
        root_dir,
        expected_paths,
        deleted_count,
        ScanLimits::default(),
    )
}

pub(in crate::provider_credential_folder_sync) fn delete_stale_files_with_limits(
    root_dir: &Path,
    expected_paths: &HashSet<String>,
    deleted_count: &mut usize,
    limits: ScanLimits,
) -> Result<(), GatewayError> {
    for file_path in collect_json_files_with_limits(root_dir, limits)? {
        let relative = normalize_relative_path(root_dir, &file_path)?;
        if expected_paths.contains(&relative) {
            continue;
        }
        // Discovery must finish successfully before deletion, then recheck each target.
        if paths::delete_file(root_dir, Path::new(&relative))? {
            *deleted_count += 1;
        }
    }
    Ok(())
}
