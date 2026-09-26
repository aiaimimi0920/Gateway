//! Candidate identity and embedded-value validation keep secret writes on the patch API.

use super::classification::is_sensitive_key;
use super::masks::contains_basic_mask_sentinel;
use super::urls::ensure_url_redacted;
use super::SecretPatchError;
use crate::console::document::encode_pointer_segment;
use crate::routing::config::{effective_credential_id, RouteConfigYaml};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub(super) fn ensure_identity_shape(document: &RouteConfigYaml) -> Result<(), SecretPatchError> {
    let mut providers = HashSet::<&str>::new();
    let mut credentials = HashSet::<String>::new();
    for (provider_index, provider) in document.providers.iter().enumerate() {
        if provider.id.trim().is_empty() || !providers.insert(provider.id.as_str()) {
            return Err(SecretPatchError::new(
                "secret_document_identity_invalid",
                Some(format!("/providers/{provider_index}/id")),
                "provider IDs must be non-empty and unique",
            ));
        }
        for (credential_index, credential) in provider.credentials.iter().enumerate() {
            if credential
                .id
                .as_deref()
                .is_some_and(|id| id.trim().is_empty())
            {
                return Err(SecretPatchError::new(
                    "secret_document_identity_invalid",
                    Some(format!(
                        "/providers/{provider_index}/credentials/{credential_index}/id"
                    )),
                    "credential ID must not be empty",
                ));
            }
            let id =
                effective_credential_id(&provider.id, credential_index, credential).into_owned();
            if !credentials.insert(id) {
                return Err(SecretPatchError::new(
                    "secret_document_identity_invalid",
                    Some(format!(
                        "/providers/{provider_index}/credentials/{credential_index}/id"
                    )),
                    "effective credential IDs must be globally unique",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn ensure_document_is_redacted(
    document: &RouteConfigYaml,
) -> Result<(), SecretPatchError> {
    for (provider_index, provider) in document.providers.iter().enumerate() {
        let provider_path = format!("/providers/{provider_index}");
        ensure_url_redacted(&provider.base_url, &format!("{provider_path}/base_url"))?;
        ensure_absent_string(&provider.api_key, &format!("{provider_path}/api_key"))?;
        ensure_absent_option(
            provider.auth_token.as_deref(),
            &format!("{provider_path}/auth_token"),
        )?;
        ensure_absent_option(
            provider.credential_storage_password.as_deref(),
            &format!("{provider_path}/credential_storage_password"),
        )?;
        if let Some(keepalive) = &provider.keepalive {
            ensure_url_redacted(
                &keepalive.service_url,
                &format!("{provider_path}/keepalive/serviceUrl"),
            )?;
            ensure_absent_option(
                keepalive.auth_token.as_deref(),
                &format!("{provider_path}/keepalive/authToken"),
            )?;
        }
        ensure_string_map_redacted(&provider.headers, &format!("{provider_path}/headers"))?;
        ensure_extra_body_redacted(&provider.extra_body, &format!("{provider_path}/extra_body"))?;
        for (credential_index, credential) in provider.credentials.iter().enumerate() {
            let credential_path = format!("{provider_path}/credentials/{credential_index}");
            if let Some(base_url) = &credential.base_url {
                ensure_url_redacted(base_url, &format!("{credential_path}/base_url"))?;
            }
            if let Some(refresh_endpoint) = &credential.refresh_endpoint {
                ensure_url_redacted(
                    refresh_endpoint,
                    &format!("{credential_path}/refresh_endpoint"),
                )?;
            }
            ensure_absent_option(
                credential.api_key.as_deref(),
                &format!("{credential_path}/api_key"),
            )?;
            ensure_absent_option(
                credential.auth_token.as_deref(),
                &format!("{credential_path}/auth_token"),
            )?;
            ensure_absent_option(
                credential.refresh_token.as_deref(),
                &format!("{credential_path}/refresh_token"),
            )?;
            if let Some(keepalive) = &credential.keepalive {
                ensure_url_redacted(
                    &keepalive.service_url,
                    &format!("{credential_path}/keepalive/serviceUrl"),
                )?;
                ensure_absent_option(
                    keepalive.auth_token.as_deref(),
                    &format!("{credential_path}/keepalive/authToken"),
                )?;
            }
            ensure_string_map_redacted(&credential.headers, &format!("{credential_path}/headers"))?;
            ensure_extra_body_redacted(
                &credential.extra_body,
                &format!("{credential_path}/extra_body"),
            )?;
        }
    }
    Ok(())
}

fn ensure_absent_string(value: &str, path: &str) -> Result<(), SecretPatchError> {
    if value.is_empty() {
        return Ok(());
    }
    Err(embedded_secret_error(
        path,
        &Value::String(value.to_string()),
    ))
}

fn ensure_absent_option(value: Option<&str>, path: &str) -> Result<(), SecretPatchError> {
    match value {
        None => Ok(()),
        Some(value) => Err(embedded_secret_error(
            path,
            &Value::String(value.to_string()),
        )),
    }
}

fn ensure_string_map_redacted(
    map: &HashMap<String, String>,
    base_path: &str,
) -> Result<(), SecretPatchError> {
    for (key, value) in map {
        if is_sensitive_key(key) {
            return Err(embedded_secret_error(
                &format!("{base_path}/{}", encode_pointer_segment(key)),
                &Value::String(value.clone()),
            ));
        }
    }
    Ok(())
}

fn ensure_extra_body_redacted(
    map: &HashMap<String, Value>,
    base_path: &str,
) -> Result<(), SecretPatchError> {
    for (key, value) in map {
        let path = format!("{base_path}/{}", encode_pointer_segment(key));
        if is_sensitive_key(key) {
            return Err(embedded_secret_error(&path, value));
        }
        ensure_json_value_redacted(value, &path)?;
    }
    Ok(())
}

fn ensure_json_value_redacted(value: &Value, base_path: &str) -> Result<(), SecretPatchError> {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                let path = format!("{base_path}/{}", encode_pointer_segment(key));
                if is_sensitive_key(key) {
                    return Err(embedded_secret_error(&path, value));
                }
                ensure_json_value_redacted(value, &path)?;
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                ensure_json_value_redacted(value, &format!("{base_path}/{index}"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn embedded_secret_error(path: &str, value: &Value) -> SecretPatchError {
    let code = if contains_basic_mask_sentinel(value) {
        "secret_mask_sentinel_rejected"
    } else {
        "secret_value_must_use_patch"
    };
    SecretPatchError::new(
        code,
        Some(path.to_string()),
        "secret values must be supplied through keep, replace, or clear patches",
    )
}
