//! Secret wire contracts and route-document transformation orchestration.

mod admission;
mod classification;
mod descriptors;
mod identity;
mod masks;
mod pointers;
mod urls;
mod validation;

use admission::{preflight_secret_patch_budget, validate_operation_shape, MAX_SECRET_PATCHES};
pub(crate) use classification::is_sensitive_key;
use descriptors::{
    push_optional_string_descriptor, push_string_descriptor, redact_extra_body_map,
    redact_string_map,
};
use identity::{
    active_path_for_keep, map_array_structural_identity, require_operations_for_existing_secrets,
};
use masks::contains_mask_sentinel;
use pointers::{
    remove_value, set_value, validate_secret_target, value_at, ParsedPointer, SecretTarget,
};
pub(crate) use urls::redact_url_value;
use validation::{ensure_document_is_redacted, ensure_identity_shape};

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::document::materialize_credential_ids;
use crate::routing::config::RouteConfigYaml;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SecretDescriptor {
    pub path: String,
    pub configured: bool,
    pub fingerprint: Option<String>,
    pub preview: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RedactedRouteDocument {
    pub document: RouteConfigYaml,
    pub secrets: Vec<SecretDescriptor>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretOperation {
    Keep,
    Replace,
    Clear,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct SecretPatch {
    pub path: String,
    pub operation: SecretOperation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
}

impl fmt::Debug for SecretPatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("SecretPatch");
        debug
            .field("path", &self.path)
            .field("operation", &self.operation);
        if self.value.is_some() {
            debug.field("value", &"<redacted>");
        }
        debug.finish()
    }
}

impl SecretPatch {
    pub fn keep(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            operation: SecretOperation::Keep,
            value: None,
        }
    }

    pub fn replace(path: impl Into<String>, value: Value) -> Self {
        Self {
            path: path.into(),
            operation: SecretOperation::Replace,
            value: Some(value),
        }
    }

    pub fn clear(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            operation: SecretOperation::Clear,
            value: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretPatchError {
    code: &'static str,
    path: Option<String>,
    message: String,
}

impl SecretPatchError {
    fn new(
        code: &'static str,
        path: Option<impl Into<String>>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            path: path.map(Into::into),
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }
}

impl fmt::Display for SecretPatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.path {
            Some(path) => write!(formatter, "{} at {}: {}", self.code, path, self.message),
            None => write!(formatter, "{}: {}", self.code, self.message),
        }
    }
}

impl Error for SecretPatchError {}

pub fn redact_route_document(
    document: &RouteConfigYaml,
) -> Result<RedactedRouteDocument, SecretPatchError> {
    ensure_identity_shape(document)?;
    let mut document = materialize_credential_ids(document.clone());
    let mut secrets = Vec::new();

    for (provider_index, provider) in document.providers.iter_mut().enumerate() {
        let provider_path = format!("/providers/{provider_index}");
        provider.base_url = redact_url_value(&provider.base_url);
        push_string_descriptor(
            &mut secrets,
            format!("{provider_path}/api_key"),
            &provider.api_key,
            !provider.api_key.is_empty(),
        );
        provider.api_key.clear();

        push_optional_string_descriptor(
            &mut secrets,
            format!("{provider_path}/auth_token"),
            provider.auth_token.as_deref(),
        );
        provider.auth_token = None;

        push_optional_string_descriptor(
            &mut secrets,
            format!("{provider_path}/credential_storage_password"),
            provider.credential_storage_password.as_deref(),
        );
        provider.credential_storage_password = None;

        if let Some(keepalive) = provider.keepalive.as_mut() {
            keepalive.service_url = redact_url_value(&keepalive.service_url);
            push_optional_string_descriptor(
                &mut secrets,
                format!("{provider_path}/keepalive/authToken"),
                keepalive.auth_token.as_deref(),
            );
            keepalive.auth_token = None;
        }

        redact_string_map(
            &mut provider.headers,
            &format!("{provider_path}/headers"),
            &mut secrets,
        );
        redact_extra_body_map(
            &mut provider.extra_body,
            &format!("{provider_path}/extra_body"),
            &mut secrets,
        );

        for (credential_index, credential) in provider.credentials.iter_mut().enumerate() {
            let credential_path = format!("{provider_path}/credentials/{credential_index}");
            if let Some(base_url) = credential.base_url.as_mut() {
                *base_url = redact_url_value(base_url);
            }
            if let Some(refresh_endpoint) = credential.refresh_endpoint.as_mut() {
                *refresh_endpoint = redact_url_value(refresh_endpoint);
            }
            push_optional_string_descriptor(
                &mut secrets,
                format!("{credential_path}/api_key"),
                credential.api_key.as_deref(),
            );
            credential.api_key = None;
            push_optional_string_descriptor(
                &mut secrets,
                format!("{credential_path}/auth_token"),
                credential.auth_token.as_deref(),
            );
            credential.auth_token = None;
            push_optional_string_descriptor(
                &mut secrets,
                format!("{credential_path}/refresh_token"),
                credential.refresh_token.as_deref(),
            );
            credential.refresh_token = None;
            if let Some(keepalive) = credential.keepalive.as_mut() {
                keepalive.service_url = redact_url_value(&keepalive.service_url);
                push_optional_string_descriptor(
                    &mut secrets,
                    format!("{credential_path}/keepalive/authToken"),
                    keepalive.auth_token.as_deref(),
                );
                keepalive.auth_token = None;
            }
            redact_string_map(
                &mut credential.headers,
                &format!("{credential_path}/headers"),
                &mut secrets,
            );
            redact_extra_body_map(
                &mut credential.extra_body,
                &format!("{credential_path}/extra_body"),
                &mut secrets,
            );
        }
    }

    secrets.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(RedactedRouteDocument { document, secrets })
}

pub fn resolve_secret_patches(
    active: &RouteConfigYaml,
    draft: RouteConfigYaml,
    patches: &[SecretPatch],
) -> Result<RouteConfigYaml, SecretPatchError> {
    if patches.len() > MAX_SECRET_PATCHES {
        return Err(SecretPatchError::new(
            "secret_patch_count_exceeded",
            None::<String>,
            format!("at most {MAX_SECRET_PATCHES} secret patches are allowed"),
        ));
    }
    preflight_secret_patch_budget(active, &draft, patches)?;
    ensure_identity_shape(active)?;
    ensure_identity_shape(&draft)?;
    ensure_document_is_redacted(&draft)?;

    let active_normalized = materialize_credential_ids(active.clone());
    let active_redacted = redact_route_document(active)?;
    let previews: HashSet<String> = active_redacted
        .secrets
        .iter()
        .filter_map(|descriptor| descriptor.preview.clone())
        .collect();
    let draft_redacted_value = serde_json::to_value(&draft).map_err(|error| {
        SecretPatchError::new("secret_document_invalid", None::<String>, error.to_string())
    })?;
    let mut draft_value = draft_redacted_value.clone();
    let active_value = serde_json::to_value(&active_normalized).map_err(|error| {
        SecretPatchError::new("secret_document_invalid", None::<String>, error.to_string())
    })?;
    let active_redacted_value =
        serde_json::to_value(&active_redacted.document).map_err(|error| {
            SecretPatchError::new("secret_document_invalid", None::<String>, error.to_string())
        })?;
    let mut seen_paths = HashMap::<String, SecretOperation>::new();

    for patch in patches {
        let parsed = ParsedPointer::parse(&patch.path)?;
        if seen_paths
            .insert(parsed.canonical.clone(), patch.operation)
            .is_some()
        {
            return Err(SecretPatchError::new(
                "secret_patch_duplicate",
                Some(parsed.canonical),
                "secret patch path appears more than once",
            ));
        }
        validate_operation_shape(patch)?;
        let target = validate_secret_target(&draft, &draft_value, &parsed)?;

        match patch.operation {
            SecretOperation::Keep => {
                let source_path = active_path_for_keep(&active_normalized, &draft, &parsed)?;
                let source_path = map_array_structural_identity(
                    &active_redacted_value,
                    &draft_redacted_value,
                    source_path,
                    &parsed,
                )?;
                match value_at(&active_value, &source_path.segments).cloned() {
                    Some(value) => set_value(&mut draft_value, &parsed.segments, value)?,
                    None if target.is_map_value() => {
                        remove_value(&mut draft_value, &parsed.segments)?;
                    }
                    None if matches!(target, SecretTarget::OptionalString) => {
                        set_value(&mut draft_value, &parsed.segments, Value::Null)?;
                    }
                    None => {
                        return Err(SecretPatchError::new(
                            "secret_keep_source_missing",
                            Some(parsed.canonical),
                            "active revision does not contain the requested secret",
                        ));
                    }
                }
            }
            SecretOperation::Replace => {
                let value = patch.value.clone().expect("shape validated");
                if contains_mask_sentinel(&value, &previews) {
                    return Err(SecretPatchError::new(
                        "secret_mask_sentinel_rejected",
                        Some(parsed.canonical),
                        "masked previews cannot be stored as secret values",
                    ));
                }
                if target.requires_string() && !value.is_string() {
                    return Err(SecretPatchError::new(
                        "secret_patch_type_mismatch",
                        Some(parsed.canonical),
                        "this secret field requires a string value",
                    ));
                }
                set_value(&mut draft_value, &parsed.segments, value)?;
            }
            SecretOperation::Clear => match target {
                SecretTarget::ProviderApiKey => set_value(
                    &mut draft_value,
                    &parsed.segments,
                    Value::String(String::new()),
                )?,
                SecretTarget::OptionalString => {
                    set_value(&mut draft_value, &parsed.segments, Value::Null)?;
                }
                SecretTarget::Header | SecretTarget::ExtraBody => {
                    remove_value(&mut draft_value, &parsed.segments)?;
                }
            },
        }
    }

    require_operations_for_existing_secrets(
        &active_normalized,
        &draft,
        &active_redacted.secrets,
        &seen_paths,
    )?;

    let resolved: RouteConfigYaml = serde_json::from_value(draft_value).map_err(|error| {
        SecretPatchError::new("secret_document_invalid", None::<String>, error.to_string())
    })?;
    Ok(materialize_credential_ids(resolved))
}

pub(crate) fn route_config_request_requires_secret_grant(
    document: &RouteConfigYaml,
    patches: &[SecretPatch],
) -> bool {
    patches
        .iter()
        .any(|patch| patch.operation != SecretOperation::Keep || patch.value.is_some())
        || ensure_document_is_redacted(document).is_err()
}
