//! Secret reuse follows stable provider, credential and unique redacted-array identities.

use super::pointers::{
    format_pointer, outside_schema, parse_array_index, parse_json_index, pointer_error,
    ParsedPointer,
};
use super::{SecretDescriptor, SecretOperation, SecretPatchError};
use crate::console::document::encode_pointer_segment;
use crate::routing::config::RouteConfigYaml;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub(super) fn active_path_for_keep(
    active: &RouteConfigYaml,
    draft: &RouteConfigYaml,
    pointer: &ParsedPointer,
) -> Result<ParsedPointer, SecretPatchError> {
    let draft_provider_index = parse_array_index(&pointer.segments[1], pointer)?;
    let draft_provider = draft
        .providers
        .get(draft_provider_index)
        .ok_or_else(|| outside_schema(pointer))?;
    let active_provider_index = active
        .providers
        .iter()
        .position(|provider| provider.id == draft_provider.id)
        .ok_or_else(|| {
            SecretPatchError::new(
                "secret_keep_identity_changed",
                Some(pointer.canonical.clone()),
                "provider identity does not exist in the active revision",
            )
        })?;

    let mut segments = pointer.segments.clone();
    segments[1] = active_provider_index.to_string();
    if segments.get(2).map(String::as_str) == Some("credentials") {
        let draft_credential_index = parse_array_index(&segments[3], pointer)?;
        let draft_credential = draft_provider
            .credentials
            .get(draft_credential_index)
            .ok_or_else(|| outside_schema(pointer))?;
        let draft_credential_id = draft_credential.id.as_deref().ok_or_else(|| {
            SecretPatchError::new(
                "secret_keep_anonymous_credential",
                Some(pointer.canonical.clone()),
                "anonymous credentials must be normalized or explicitly replaced before keep",
            )
        })?;
        let active_provider = &active.providers[active_provider_index];
        let active_credential_index = active_provider
            .credentials
            .iter()
            .position(|credential| credential.id.as_deref() == Some(draft_credential_id))
            .ok_or_else(|| {
                SecretPatchError::new(
                    "secret_keep_identity_changed",
                    Some(pointer.canonical.clone()),
                    "credential identity does not exist in the active revision",
                )
            })?;
        segments[3] = active_credential_index.to_string();
    }

    Ok(ParsedPointer {
        canonical: format!(
            "/{}",
            segments
                .iter()
                .map(|segment| encode_pointer_segment(segment))
                .collect::<Vec<_>>()
                .join("/")
        ),
        segments,
    })
}

/// Map array indexes in a redacted draft back to the corresponding indexes in
/// the active redacted document. Array positions are not identities: a safe
/// keep requires exactly one structurally equal (non-secret) element.
pub(super) fn map_array_structural_identity(
    active_redacted: &Value,
    draft_redacted: &Value,
    mut active_path: ParsedPointer,
    draft_path: &ParsedPointer,
) -> Result<ParsedPointer, SecretPatchError> {
    let extra_body_index = match draft_path.segments.as_slice() {
        [_, _, field, ..] if field == "extra_body" => Some(2),
        [_, _, field, _, _, ..] if field == "credentials" => Some(4),
        _ => None,
    };
    let Some(extra_body_index) = extra_body_index else {
        return Ok(active_path);
    };

    let mut active_current = active_redacted;
    let mut draft_current = draft_redacted;
    let parent_len = draft_path.segments.len().saturating_sub(1);
    for index in 0..parent_len {
        let draft_segment = &draft_path.segments[index];
        let active_segment = &mut active_path.segments[index];
        let structural_array = index > extra_body_index;

        match (active_current, draft_current) {
            (Value::Object(active_map), Value::Object(draft_map)) => {
                active_current = active_map.get(active_segment).ok_or_else(|| {
                    array_identity_error(
                        draft_path,
                        "active redacted document is missing the keep path",
                    )
                })?;
                draft_current = draft_map.get(draft_segment).ok_or_else(|| {
                    array_identity_error(
                        draft_path,
                        "draft redacted document is missing the keep path",
                    )
                })?;
            }
            (Value::Array(active_array), Value::Array(draft_array)) => {
                let draft_index = parse_json_index(draft_segment)
                    .ok_or_else(|| pointer_error(&draft_path.canonical))?;
                let draft_element = draft_array.get(draft_index).ok_or_else(|| {
                    array_identity_error(draft_path, "draft array index is outside the array")
                })?;

                let active_index = if structural_array {
                    let mut matches = active_array.iter().enumerate().filter_map(
                        |(candidate_index, candidate)| {
                            (candidate == draft_element).then_some(candidate_index)
                        },
                    );
                    let Some(candidate_index) = matches.next() else {
                        return Err(array_identity_error(
                            draft_path,
                            "redacted array element changed and cannot be kept safely",
                        ));
                    };
                    if matches.next().is_some() {
                        return Err(array_identity_error(
                            draft_path,
                            "redacted array element identity is ambiguous",
                        ));
                    }
                    *active_segment = candidate_index.to_string();
                    candidate_index
                } else {
                    parse_json_index(active_segment)
                        .ok_or_else(|| pointer_error(&active_path.canonical))?
                };

                active_current = active_array.get(active_index).ok_or_else(|| {
                    array_identity_error(draft_path, "active array index is outside the array")
                })?;
                draft_current = draft_element;
            }
            _ => {
                return Err(array_identity_error(
                    draft_path,
                    "active and draft keep paths have incompatible structures",
                ));
            }
        }
    }

    active_path.canonical = format_pointer(&active_path.segments);
    Ok(active_path)
}

fn array_identity_error(pointer: &ParsedPointer, message: impl Into<String>) -> SecretPatchError {
    SecretPatchError::new(
        "secret_keep_array_identity_changed",
        Some(pointer.canonical.clone()),
        message,
    )
}

pub(super) fn require_operations_for_existing_secrets(
    active: &RouteConfigYaml,
    draft: &RouteConfigYaml,
    descriptors: &[SecretDescriptor],
    seen_paths: &HashMap<String, SecretOperation>,
) -> Result<(), SecretPatchError> {
    for descriptor in descriptors
        .iter()
        .filter(|descriptor| descriptor.configured)
    {
        let active_pointer = ParsedPointer::parse(&descriptor.path)?;
        match draft_path_for_active_secret(active, draft, &active_pointer)? {
            SecretDisposition::Deleted => {}
            SecretDisposition::Retained(draft_path) => {
                if !seen_paths.contains_key(&draft_path) {
                    return Err(SecretPatchError::new(
                        "secret_patch_missing",
                        Some(draft_path),
                        "every configured secret retained by the draft requires keep, replace, or clear",
                    ));
                }
            }
            SecretDisposition::Replacement(draft_path) => match seen_paths.get(&draft_path) {
                None => {
                    return Err(SecretPatchError::new(
                        "secret_patch_missing",
                        Some(draft_path),
                        "same-slot identity replacement requires an explicit replace for every old secret",
                    ));
                }
                Some(SecretOperation::Replace) => {}
                Some(_) => {
                    return Err(SecretPatchError::new(
                        "secret_identity_replacement_requires_replace",
                        Some(draft_path),
                        "same-slot identity replacement requires an explicit replace for every old secret",
                    ));
                }
            },
            SecretDisposition::ReplacementWithoutTarget(path) => {
                return Err(SecretPatchError::new(
                    "secret_identity_replacement_requires_replace",
                    Some(path),
                    "same-slot identity replacement removed a configured secret without an explicit replace",
                ));
            }
        }
    }
    Ok(())
}

enum SecretDisposition {
    Retained(String),
    Replacement(String),
    ReplacementWithoutTarget(String),
    Deleted,
}

fn draft_path_for_active_secret(
    active: &RouteConfigYaml,
    draft: &RouteConfigYaml,
    pointer: &ParsedPointer,
) -> Result<SecretDisposition, SecretPatchError> {
    let active_provider_index = parse_array_index(&pointer.segments[1], pointer)?;
    let Some(active_provider) = active.providers.get(active_provider_index) else {
        return Ok(SecretDisposition::Deleted);
    };
    let Some(draft_provider_index) = draft
        .providers
        .iter()
        .position(|provider| provider.id == active_provider.id)
    else {
        let Some(slot) = draft.providers.get(active_provider_index) else {
            return Ok(SecretDisposition::Deleted);
        };
        if active
            .providers
            .iter()
            .any(|provider| provider.id == slot.id)
        {
            return Ok(SecretDisposition::Deleted);
        }
        let mut replacement_segments = pointer.segments.clone();
        replacement_segments[1] = active_provider_index.to_string();
        if pointer.segments.get(2).map(String::as_str) == Some("credentials") {
            let active_credential_index = parse_array_index(&pointer.segments[3], pointer)?;
            let Some(credential) = slot.credentials.get(active_credential_index) else {
                return Ok(SecretDisposition::ReplacementWithoutTarget(format_pointer(
                    &replacement_segments,
                )));
            };
            replacement_segments[3] = active_credential_index.to_string();
            let _ = credential;
        }
        return Ok(SecretDisposition::Replacement(format_pointer(
            &replacement_segments,
        )));
    };

    let mut segments = pointer.segments.clone();
    segments[1] = draft_provider_index.to_string();
    if segments.get(2).map(String::as_str) == Some("credentials") {
        let active_credential_index = parse_array_index(&pointer.segments[3], pointer)?;
        let Some(active_credential_id) = active_provider
            .credentials
            .get(active_credential_index)
            .and_then(|credential| credential.id.as_deref())
        else {
            let Some(slot) = draft.providers[draft_provider_index]
                .credentials
                .get(active_credential_index)
            else {
                return Ok(SecretDisposition::Deleted);
            };
            let active_ids: HashSet<&str> = active_provider
                .credentials
                .iter()
                .filter_map(|credential| credential.id.as_deref())
                .collect();
            if active_ids.contains(slot.id.as_deref().unwrap_or_default()) {
                return Ok(SecretDisposition::Deleted);
            }
            segments[3] = active_credential_index.to_string();
            return Ok(SecretDisposition::Replacement(format_pointer(&segments)));
        };
        let Some(draft_credential_index) = draft.providers[draft_provider_index]
            .credentials
            .iter()
            .position(|credential| credential.id.as_deref() == Some(active_credential_id))
        else {
            let Some(slot) = draft.providers[draft_provider_index]
                .credentials
                .get(active_credential_index)
            else {
                return Ok(SecretDisposition::Deleted);
            };
            let active_ids: HashSet<&str> = active_provider
                .credentials
                .iter()
                .filter_map(|credential| credential.id.as_deref())
                .collect();
            if active_ids.contains(slot.id.as_deref().unwrap_or_default()) {
                return Ok(SecretDisposition::Deleted);
            }
            segments[3] = active_credential_index.to_string();
            return Ok(SecretDisposition::Replacement(format_pointer(&segments)));
        };
        segments[3] = draft_credential_index.to_string();
    }
    Ok(SecretDisposition::Retained(format_pointer(&segments)))
}
