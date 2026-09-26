//! Finite per-scan and per-material budgets for filesystem synchronization.
use crate::error::GatewayError;
use crate::object_storage::STANDARD_OBJECT_LIMIT_BYTES;

// Align encoded credentials with ordinary object storage; datasets have a separate policy.
pub(super) const MATERIAL_BYTES: usize = 32 * 1024 * 1024;
pub(super) const SOURCE_PATH_CHARS: usize = 512;
pub(super) const SOURCE_PATH_RETAINED_BYTES: usize = 32 * 1024 * 1024;
// Account for normalized keys shared by the metadata index and observed paths.
pub(super) const SOURCE_PATH_AGGREGATE_RETAINED_BYTES: usize = 64 * 1024 * 1024;
pub(super) const PROVIDER_ACCOUNT_ROWS: usize = 4_096;
pub(super) const PROVIDER_CREDENTIAL_METADATA_ROWS: usize = 100_000;
pub(super) const PROVIDER_ACCOUNT_INLINE_PAYLOAD_BYTES: usize = STANDARD_OBJECT_LIMIT_BYTES;
pub(super) const MATERIAL_TOO_LARGE: &str = "provider_credential_folder_sync_material_too_large";
pub(super) const MATERIAL_ALLOCATION_FAILED: &str =
    "provider_credential_folder_sync_material_allocation_failed";

#[derive(Clone, Copy)]
pub(super) struct ScanLimits {
    pub max_depth: usize,
    pub max_entries: usize,
    pub max_files: usize,
}

pub(super) fn scan_limit(kind: &str, limit: usize) -> GatewayError {
    GatewayError::bad_request(format!(
        "provider credential folder scan exceeds {kind} limit {limit}"
    ))
    .with_code("provider_credential_folder_sync_scan_limit")
}

pub(super) fn ensure_material_size(size: u64, limit: usize) -> Result<(), GatewayError> {
    if size > limit as u64 {
        return Err(GatewayError::bad_request(format!(
            "provider credential material exceeds {limit} byte limit"
        ))
        .with_code(MATERIAL_TOO_LARGE));
    }
    Ok(())
}

pub(super) fn source_path_retention_limit(max_bytes: usize) -> GatewayError {
    GatewayError::bad_request(format!(
        "provider credential folder sync path retention exceeds {max_bytes} byte limit"
    ))
    .with_code("provider_credential_folder_sync_path_retention_limit")
}

pub(super) fn is_material_capacity_error(error: &GatewayError) -> bool {
    matches!(
        error.code.as_deref(),
        Some(MATERIAL_TOO_LARGE | MATERIAL_ALLOCATION_FAILED)
    )
}

impl Default for ScanLimits {
    fn default() -> Self {
        Self {
            max_depth: 32,
            max_entries: 100_000,
            max_files: 10_000,
        }
    }
}
