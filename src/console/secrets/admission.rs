//! Patch shape and byte budgets are checked before per-patch resolution work.

use super::{SecretOperation, SecretPatch, SecretPatchError};
use crate::console::document::MAX_CANONICAL_DOCUMENT_BYTES;
use crate::routing::config::RouteConfigYaml;

pub(super) const MAX_SECRET_PATCHES: usize = 4096;
const MAX_SECRET_REPLACE_BYTES: usize = 1024 * 1024;
const MAX_SECRET_REPLACEMENT_TOTAL_BYTES: usize = MAX_CANONICAL_DOCUMENT_BYTES;
const MAX_SECRET_PATCH_PATH_BYTES: usize = 16 * 1024;
const MAX_SECRET_PATCH_PATHS_BYTES: usize = 1024 * 1024;

pub(super) fn preflight_secret_patch_budget(
    active: &RouteConfigYaml,
    draft: &RouteConfigYaml,
    patches: &[SecretPatch],
) -> Result<(), SecretPatchError> {
    ensure_secret_document_size(active, "active")?;
    ensure_secret_document_size(draft, "draft")?;

    let mut total_path_bytes = 0usize;
    let mut total_replacement_bytes = 0usize;
    for patch in patches {
        if patch.path.len() > MAX_SECRET_PATCH_PATH_BYTES {
            return Err(SecretPatchError::new(
                "secret_patch_path_too_large",
                None::<String>,
                format!("a secret patch path exceeds the {MAX_SECRET_PATCH_PATH_BYTES}-byte limit"),
            ));
        }
        total_path_bytes = total_path_bytes
            .checked_add(patch.path.len())
            .ok_or_else(|| {
                SecretPatchError::new(
                    "secret_patch_paths_too_large",
                    None::<String>,
                    "total secret patch path bytes overflowed",
                )
            })?;
        if total_path_bytes > MAX_SECRET_PATCH_PATHS_BYTES {
            return Err(SecretPatchError::new(
                "secret_patch_paths_too_large",
                None::<String>,
                format!(
                    "secret patch paths exceed the {MAX_SECRET_PATCH_PATHS_BYTES}-byte total limit"
                ),
            ));
        }

        validate_operation_shape(patch)?;
        if let Some(value) = &patch.value {
            let encoded = serde_json::to_vec(value).map_err(|error| {
                SecretPatchError::new(
                    "secret_patch_value_invalid",
                    None::<String>,
                    error.to_string(),
                )
            })?;
            if encoded.len() > MAX_SECRET_REPLACE_BYTES {
                return Err(SecretPatchError::new(
                    "secret_patch_value_too_large",
                    None::<String>,
                    format!("replacement value exceeds the {MAX_SECRET_REPLACE_BYTES}-byte limit"),
                ));
            }
            total_replacement_bytes = total_replacement_bytes
                .checked_add(encoded.len())
                .ok_or_else(|| {
                    SecretPatchError::new(
                        "secret_patch_value_total_too_large",
                        None::<String>,
                        "total replacement bytes overflowed",
                    )
                })?;
            if total_replacement_bytes > MAX_SECRET_REPLACEMENT_TOTAL_BYTES {
                return Err(SecretPatchError::new(
                    "secret_patch_value_total_too_large",
                    None::<String>,
                    format!(
                        "replacement values exceed the {MAX_SECRET_REPLACEMENT_TOTAL_BYTES}-byte total limit"
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn ensure_secret_document_size(
    document: &RouteConfigYaml,
    label: &str,
) -> Result<(), SecretPatchError> {
    let serialized = serde_json::to_vec(document).map_err(|error| {
        SecretPatchError::new("secret_document_invalid", None::<String>, error.to_string())
    })?;
    if serialized.len() > MAX_CANONICAL_DOCUMENT_BYTES {
        return Err(SecretPatchError::new(
            "secret_document_too_large",
            None::<String>,
            format!("{label} route document exceeds the {MAX_CANONICAL_DOCUMENT_BYTES}-byte limit"),
        ));
    }
    Ok(())
}

pub(super) fn validate_operation_shape(patch: &SecretPatch) -> Result<(), SecretPatchError> {
    match (patch.operation, patch.value.is_some()) {
        (SecretOperation::Replace, false) => Err(SecretPatchError::new(
            "secret_patch_value_missing",
            Some(patch.path.clone()),
            "replace operation requires a value",
        )),
        (SecretOperation::Keep | SecretOperation::Clear, true) => Err(SecretPatchError::new(
            "secret_patch_value_unexpected",
            Some(patch.path.clone()),
            "keep and clear operations must not include a value",
        )),
        _ => Ok(()),
    }
}
