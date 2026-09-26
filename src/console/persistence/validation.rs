//! Canonical bytes and durable metadata identifiers are validated before persistence.

use super::PersistenceError;
use std::path::{Component, Path};

pub(super) fn validate_canonical_yaml(bytes: &[u8]) -> Result<(), PersistenceError> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF])
        || bytes.contains(&b'\r')
        || std::str::from_utf8(bytes).is_err()
    {
        return Err(PersistenceError::new(
            "console_persistence_invalid_yaml",
            "Canonical routes YAML must be UTF-8 without BOM and use LF line endings",
        ));
    }
    Ok(())
}

pub(super) fn validate_archive_relative_path(value: &str) -> Result<(), PersistenceError> {
    let path = Path::new(value);
    if path.is_absolute() {
        return Err(PersistenceError::path_escape(
            "Transaction archive path must be relative to the console state root",
        ));
    }
    let mut components = path.components();
    match (components.next(), components.next(), components.next()) {
        (Some(Component::Normal(root)), Some(Component::Normal(_)), None)
            if root == "revisions" =>
        {
            Ok(())
        }
        _ => Err(PersistenceError::path_escape(
            "Transaction archive path must be revisions/<revision-id>",
        )),
    }
}

pub(super) fn validate_safe_leaf(value: &str) -> Result<(), PersistenceError> {
    let path = Path::new(value);
    if path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        return Err(PersistenceError::path_escape(
            "Same-directory backup path must be a single relative leaf",
        ));
    }
    #[cfg(windows)]
    if value.contains(':') {
        return Err(PersistenceError::path_escape(
            "Same-directory backup leaf must not name an NTFS alternate data stream",
        ));
    }
    Ok(())
}

pub(super) fn validate_safe_id(kind: &str, value: &str) -> Result<(), PersistenceError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(PersistenceError::corruption(format!(
            "Invalid {kind} identifier"
        )));
    }
    Ok(())
}

pub(super) fn validate_error_code(value: &str) -> Result<(), PersistenceError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-'))
    {
        return Err(PersistenceError::corruption(
            "Transaction error code must be a bounded stable ASCII code",
        ));
    }
    Ok(())
}

pub(super) fn validate_revision_id(value: &str) -> Result<(), PersistenceError> {
    let Some((sequence, digest)) = value
        .strip_prefix('r')
        .and_then(|value| value.split_once('-'))
    else {
        return Err(PersistenceError::corruption("Invalid revision identifier"));
    };
    let Ok(parsed) = sequence.parse::<u64>() else {
        return Err(PersistenceError::corruption("Invalid revision sequence"));
    };
    if sequence != parsed.to_string()
        || digest.len() != 12
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(PersistenceError::corruption("Invalid revision identifier"));
    }
    Ok(())
}

pub(super) fn revision_sequence(value: &str) -> u64 {
    value
        .strip_prefix('r')
        .and_then(|value| value.split_once('-'))
        .and_then(|(sequence, _)| sequence.parse().ok())
        .unwrap_or(u64::MAX)
}

pub(super) fn normalized_relative_string(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

pub(super) fn is_lowercase_digest(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    })
}
