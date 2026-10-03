//! Validate remote destinations before any connection is created. Never echo secret input.
use crate::routing::config::CredentialStorageConnection;
use sha2::{Digest, Sha256};
use url::Url;

pub(crate) fn validate_connection(
    connection: &CredentialStorageConnection,
) -> Result<(), &'static str> {
    match connection {
        CredentialStorageConnection::Local { path } => {
            if let Some(path) = path {
                crate::credential_pool_automation::storage_paths::validate_storage_path(path)?;
            }
        }
        CredentialStorageConnection::Webdav {
            endpoint,
            directory,
            username,
            password,
            allow_insecure_http,
        } => {
            validate_endpoint(endpoint, *allow_insecure_http)?;
            validate_prefix(directory)?;
            if username
                .as_deref()
                .is_some_and(|v| v.contains(':') || v.chars().any(char::is_control))
            {
                return Err("WebDAV username must not contain colons or control characters");
            }
            if password.is_some() && username.as_deref().is_none_or(str::is_empty) {
                return Err("WebDAV password requires a username");
            }
        }
        CredentialStorageConnection::S3 {
            endpoint,
            bucket,
            region,
            prefix,
            ..
        } => {
            let CredentialStorageConnection::S3 {
                allow_insecure_http,
                ..
            } = connection
            else {
                unreachable!()
            };
            validate_endpoint(endpoint, *allow_insecure_http)?;
            validate_prefix(prefix)?;
            if bucket.is_empty()
                || bucket.len() > 63
                || !bucket
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
                || bucket == "."
                || bucket == ".."
            {
                return Err("S3 bucket must be a plain bucket name");
            }
            if region.is_empty()
                || region.len() > 64
                || !region
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            {
                return Err("S3 region must be an explicit region or auto");
            }
        }
    }
    for (_, value) in connection.secret_fields() {
        if value.is_some_and(|v| v.len() > 16_384 || v.chars().any(char::is_control)) {
            return Err(
                "Storage authentication value exceeds limits or contains control characters",
            );
        }
    }
    Ok(())
}

pub(crate) fn validate_endpoint(value: &str, allow_http: bool) -> Result<(), &'static str> {
    if value.len() > 2048
        || value.trim() != value
        || value.contains('\\')
        || value.chars().any(char::is_control)
    {
        return Err("Storage endpoint must be a valid HTTPS URL");
    }
    let url = Url::parse(value).map_err(|_| "Storage endpoint must be a valid HTTPS URL")?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "Storage endpoint must not contain credentials, query parameters, or fragments",
        );
    }
    if url.host_str().is_none()
        || !(url.scheme() == "https" || (allow_http && url.scheme() == "http"))
    {
        return Err("Storage requires HTTPS; insecure HTTP requires explicit opt-in");
    }
    // Inspect the raw path too: URL parsers normalize literal dot components.
    let raw_path = value
        .split_once("://")
        .and_then(|(_, rest)| rest.split_once('/'))
        .map(|(_, path)| path)
        .unwrap_or("");
    validate_prefix(raw_path)?;
    Ok(())
}

pub(crate) fn validate_prefix(value: &str) -> Result<(), &'static str> {
    if value.len() > 1024
        || value.starts_with('/')
        || value.contains(['%', '\\', ':', '?', '#'])
        || value.chars().any(char::is_control)
    {
        return Err(
            "Storage directory or prefix must be relative and cannot contain encoded escapes",
        );
    }
    let value = value.strip_suffix('/').unwrap_or(value);
    if !value.is_empty()
        && (value.split('/').count() > 24
            || value
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == ".."))
    {
        return Err("Storage directory or prefix cannot contain empty or traversal components");
    }
    Ok(())
}

pub(crate) fn namespace(
    connection: &CredentialStorageConnection,
    provider_id: &str,
    archive: bool,
) -> String {
    let prefix = match connection {
        CredentialStorageConnection::Webdav { directory, .. } => directory,
        CredentialStorageConnection::S3 { prefix, .. } => prefix,
        CredentialStorageConnection::Local { .. } => "",
    }
    .trim_end_matches('/');
    let provider_hash = format!("{:x}", Sha256::digest(provider_id.as_bytes()));
    let purpose = if archive { "archive" } else { "refill" };
    let relative = format!("gateway-pool/{provider_hash}/{purpose}");
    if prefix.is_empty() {
        relative
    } else {
        format!("{prefix}/{relative}")
    }
}

pub(crate) fn authentication_configured(connection: Option<&CredentialStorageConnection>) -> bool {
    match connection {
        Some(CredentialStorageConnection::Webdav {
            username, password, ..
        }) => {
            username.as_deref().is_some_and(|v| !v.is_empty())
                && password.as_deref().is_some_and(|v| !v.is_empty())
        }
        Some(CredentialStorageConnection::S3 {
            access_key_id,
            secret_access_key,
            ..
        }) => {
            access_key_id.as_deref().is_some_and(|v| !v.is_empty())
                && secret_access_key.as_deref().is_some_and(|v| !v.is_empty())
        }
        _ => false,
    }
}
