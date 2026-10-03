//! Cloud auth participates in the same grant, redaction and patch contracts as API keys.
use super::{
    descriptors::push_optional_string_descriptor, urls::redact_url_value, SecretDescriptor,
    SecretPatchError,
};
use crate::routing::config::{CredentialStorageConnection as Connection, ProviderConfigYaml};

pub(super) fn connection<'a>(
    provider: &'a ProviderConfigYaml,
    field: &str,
) -> Option<&'a Connection> {
    match field {
        "credential_storage_connection" => provider.credential_storage_connection.as_ref(),
        "credential_archive_connection" => provider.credential_archive_connection.as_ref(),
        _ => None,
    }
}

pub(super) fn is_field(field: &str) -> bool {
    matches!(
        field,
        "credential_storage_connection" | "credential_archive_connection"
    )
}

pub(super) fn secret_exists(provider: &ProviderConfigYaml, field: &str, secret: &str) -> bool {
    connection(provider, field)
        .is_some_and(|c| c.secret_fields().iter().any(|(name, _)| *name == secret))
}

pub(super) fn redact(
    provider: &mut ProviderConfigYaml,
    base: &str,
    descriptors: &mut Vec<SecretDescriptor>,
) {
    for (field, connection) in [
        (
            "credential_storage_connection",
            &mut provider.credential_storage_connection,
        ),
        (
            "credential_archive_connection",
            &mut provider.credential_archive_connection,
        ),
    ] {
        let Some(connection) = connection else {
            continue;
        };
        for (name, value) in connection.secret_fields() {
            push_optional_string_descriptor(descriptors, format!("{base}/{field}/{name}"), value);
        }
        connection.clear_secrets();
        match connection {
            Connection::Webdav { endpoint, .. } | Connection::S3 { endpoint, .. } => {
                *endpoint = redact_url_value(endpoint)
            }
            _ => {}
        }
    }
}

pub(super) fn ensure_redacted(
    provider: &ProviderConfigYaml,
    base: &str,
) -> Result<(), SecretPatchError> {
    for field in [
        "credential_storage_connection",
        "credential_archive_connection",
    ] {
        let Some(connection) = connection(provider, field) else {
            continue;
        };
        for (name, value) in connection.secret_fields() {
            super::validation::ensure_absent_option(value, &format!("{base}/{field}/{name}"))?;
        }
        match connection {
            Connection::Webdav { endpoint, .. } | Connection::S3 { endpoint, .. } => {
                super::urls::ensure_url_redacted(endpoint, &format!("{base}/{field}/endpoint"))?
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn require_keep_identity(
    active: &ProviderConfigYaml,
    draft: &ProviderConfigYaml,
    field: &str,
    path: &str,
) -> Result<(), SecretPatchError> {
    let same = match (connection(active, field), connection(draft, field)) {
        (
            Some(Connection::Webdav {
                endpoint: a,
                username: u,
                ..
            }),
            Some(Connection::Webdav {
                endpoint: b,
                username: v,
                ..
            }),
        ) => a == b && u == v,
        (
            Some(Connection::S3 {
                endpoint: a,
                bucket: x,
                region: r,
                ..
            }),
            Some(Connection::S3 {
                endpoint: b,
                bucket: y,
                region: s,
                ..
            }),
        ) => a == b && x == y && r == s,
        _ => false,
    };
    if !same {
        return Err(SecretPatchError::new("secret_keep_storage_identity_changed", Some(path.to_string()), "Changing storage destination or authentication identity requires replacing or clearing its secrets"));
    }
    Ok(())
}
