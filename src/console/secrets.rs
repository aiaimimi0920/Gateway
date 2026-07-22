use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use url::Url;

use super::document::{
    encode_pointer_segment, materialize_credential_ids, MAX_CANONICAL_DOCUMENT_BYTES,
};
use crate::routing::config::{effective_credential_id, RouteConfigYaml};

const MAX_SECRET_PATCHES: usize = 4096;
const MAX_SECRET_REPLACE_BYTES: usize = 1024 * 1024;
const MAX_SECRET_REPLACEMENT_TOTAL_BYTES: usize = MAX_CANONICAL_DOCUMENT_BYTES;
const MAX_SECRET_PATCH_PATH_BYTES: usize = 16 * 1024;
const MAX_SECRET_PATCH_PATHS_BYTES: usize = 1024 * 1024;

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

fn preflight_secret_patch_budget(
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SecretTarget {
    ProviderApiKey,
    OptionalString,
    Header,
    ExtraBody,
}

impl SecretTarget {
    fn requires_string(self) -> bool {
        !matches!(self, Self::ExtraBody)
    }

    fn is_map_value(self) -> bool {
        matches!(self, Self::Header | Self::ExtraBody)
    }
}

#[derive(Clone, Debug)]
struct ParsedPointer {
    segments: Vec<String>,
    canonical: String,
}

impl ParsedPointer {
    fn parse(path: &str) -> Result<Self, SecretPatchError> {
        if path.is_empty() {
            return Ok(Self {
                segments: Vec::new(),
                canonical: String::new(),
            });
        }
        if !path.starts_with('/') {
            return Err(pointer_error(path));
        }
        let mut segments = Vec::new();
        for raw in path[1..].split('/') {
            segments.push(decode_pointer_segment(raw).ok_or_else(|| pointer_error(path))?);
        }
        let canonical = format!(
            "/{}",
            segments
                .iter()
                .map(|segment| encode_pointer_segment(segment))
                .collect::<Vec<_>>()
                .join("/")
        );
        Ok(Self {
            segments,
            canonical,
        })
    }
}

fn pointer_error(path: &str) -> SecretPatchError {
    SecretPatchError::new(
        "secret_pointer_invalid",
        Some(path.to_string()),
        "secret path must be a valid RFC6901 JSON Pointer",
    )
}

fn decode_pointer_segment(segment: &str) -> Option<String> {
    let mut decoded = String::with_capacity(segment.len());
    let mut chars = segment.chars();
    while let Some(character) = chars.next() {
        if character != '~' {
            decoded.push(character);
            continue;
        }
        match chars.next()? {
            '0' => decoded.push('~'),
            '1' => decoded.push('/'),
            _ => return None,
        }
    }
    Some(decoded)
}

fn validate_operation_shape(patch: &SecretPatch) -> Result<(), SecretPatchError> {
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

fn validate_secret_target(
    draft: &RouteConfigYaml,
    draft_value: &Value,
    pointer: &ParsedPointer,
) -> Result<SecretTarget, SecretPatchError> {
    let segments = &pointer.segments;
    if segments.first().map(String::as_str) != Some("providers") || segments.len() < 3 {
        return Err(outside_schema(pointer));
    }
    let provider_index = parse_array_index(&segments[1], pointer)?;
    let provider = draft
        .providers
        .get(provider_index)
        .ok_or_else(|| outside_schema(pointer))?;

    match segments[2].as_str() {
        "api_key" if segments.len() == 3 => Ok(SecretTarget::ProviderApiKey),
        "auth_token" if segments.len() == 3 => Ok(SecretTarget::OptionalString),
        "keepalive"
            if segments.len() == 4
                && segments[3] == "authToken"
                && provider.keepalive.is_some() =>
        {
            Ok(SecretTarget::OptionalString)
        }
        "headers" if segments.len() == 4 && is_sensitive_key(&segments[3]) => {
            Ok(SecretTarget::Header)
        }
        "extra_body" if segments.len() >= 4 => validate_extra_body_target(draft_value, pointer, 3),
        "credentials" if segments.len() >= 5 => {
            let credential_index = parse_array_index(&segments[3], pointer)?;
            let credential = provider
                .credentials
                .get(credential_index)
                .ok_or_else(|| outside_schema(pointer))?;
            match segments[4].as_str() {
                "api_key" | "auth_token" | "refresh_token" if segments.len() == 5 => {
                    Ok(SecretTarget::OptionalString)
                }
                "keepalive"
                    if segments.len() == 6
                        && segments[5] == "authToken"
                        && credential.keepalive.is_some() =>
                {
                    Ok(SecretTarget::OptionalString)
                }
                "headers" if segments.len() == 6 && is_sensitive_key(&segments[5]) => {
                    Ok(SecretTarget::Header)
                }
                "extra_body" if segments.len() >= 6 => {
                    validate_extra_body_target(draft_value, pointer, 5)
                }
                _ => Err(outside_schema(pointer)),
            }
        }
        _ => Err(outside_schema(pointer)),
    }
}

fn validate_extra_body_target(
    draft_value: &Value,
    pointer: &ParsedPointer,
    extra_body_index: usize,
) -> Result<SecretTarget, SecretPatchError> {
    let relative = &pointer.segments[extra_body_index + 1..];
    if relative.is_empty() {
        return Err(outside_schema(pointer));
    }
    for segment in &relative[..relative.len() - 1] {
        if is_sensitive_key(segment) {
            return Err(outside_schema(pointer));
        }
    }
    let parent_segments = &pointer.segments[..pointer.segments.len() - 1];
    if !validated_value_at(draft_value, parent_segments, pointer)?.is_object() {
        return Err(outside_schema(pointer));
    }
    if !is_sensitive_key(relative.last().expect("non-empty")) {
        return Err(outside_schema(pointer));
    }
    Ok(SecretTarget::ExtraBody)
}

fn outside_schema(pointer: &ParsedPointer) -> SecretPatchError {
    SecretPatchError::new(
        "secret_path_outside_schema",
        Some(pointer.canonical.clone()),
        "path does not identify a supported secret field",
    )
}

fn parse_array_index(segment: &str, pointer: &ParsedPointer) -> Result<usize, SecretPatchError> {
    if segment == "-"
        || segment.is_empty()
        || (segment.len() > 1 && segment.starts_with('0'))
        || !segment.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(pointer_error(&pointer.canonical));
    }
    segment
        .parse::<usize>()
        .map_err(|_| pointer_error(&pointer.canonical))
}

fn active_path_for_keep(
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
fn map_array_structural_identity(
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

fn require_operations_for_existing_secrets(
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

fn format_pointer(segments: &[String]) -> String {
    if segments.is_empty() {
        return String::new();
    }
    format!(
        "/{}",
        segments
            .iter()
            .map(|segment| encode_pointer_segment(segment))
            .collect::<Vec<_>>()
            .join("/")
    )
}

fn value_at<'a>(value: &'a Value, segments: &[String]) -> Option<&'a Value> {
    let mut current = value;
    for segment in segments {
        current = match current {
            Value::Object(map) => map.get(segment)?,
            Value::Array(values) => values.get(parse_json_index(segment)?)?,
            _ => return None,
        };
    }
    Some(current)
}

fn validated_value_at<'a>(
    value: &'a Value,
    segments: &[String],
    pointer: &ParsedPointer,
) -> Result<&'a Value, SecretPatchError> {
    let mut current = value;
    for segment in segments {
        current = match current {
            Value::Object(map) => map.get(segment).ok_or_else(|| outside_schema(pointer))?,
            Value::Array(values) => {
                let index = parse_array_index(segment, pointer)?;
                values.get(index).ok_or_else(|| outside_schema(pointer))?
            }
            _ => return Err(outside_schema(pointer)),
        };
    }
    Ok(current)
}

fn parse_json_index(segment: &str) -> Option<usize> {
    if segment == "-"
        || segment.is_empty()
        || (segment.len() > 1 && segment.starts_with('0'))
        || !segment.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    segment.parse().ok()
}

fn set_value(
    value: &mut Value,
    segments: &[String],
    replacement: Value,
) -> Result<(), SecretPatchError> {
    let (last, parents) = segments.split_last().ok_or_else(|| pointer_error(""))?;
    let mut current = value;
    for segment in parents {
        current = match current {
            Value::Object(map) => map.get_mut(segment),
            Value::Array(values) => {
                parse_json_index(segment).and_then(|index| values.get_mut(index))
            }
            _ => None,
        }
        .ok_or_else(|| {
            SecretPatchError::new(
                "secret_path_outside_schema",
                None::<String>,
                "secret path parent does not exist",
            )
        })?;
    }
    match current {
        Value::Object(map) => {
            map.insert(last.clone(), replacement);
            Ok(())
        }
        Value::Array(values) => {
            let index = parse_json_index(last).ok_or_else(|| pointer_error(last))?;
            let slot = values.get_mut(index).ok_or_else(|| {
                SecretPatchError::new(
                    "secret_path_outside_schema",
                    None::<String>,
                    "secret array index is out of bounds",
                )
            })?;
            *slot = replacement;
            Ok(())
        }
        _ => Err(SecretPatchError::new(
            "secret_path_outside_schema",
            None::<String>,
            "secret path parent is not a container",
        )),
    }
}

fn remove_value(value: &mut Value, segments: &[String]) -> Result<(), SecretPatchError> {
    let (last, parents) = segments.split_last().ok_or_else(|| pointer_error(""))?;
    let mut current = value;
    for segment in parents {
        current = match current {
            Value::Object(map) => map.get_mut(segment),
            Value::Array(values) => {
                parse_json_index(segment).and_then(|index| values.get_mut(index))
            }
            _ => None,
        }
        .ok_or_else(|| {
            SecretPatchError::new(
                "secret_path_outside_schema",
                None::<String>,
                "secret path parent does not exist",
            )
        })?;
    }
    match current {
        Value::Object(map) => {
            map.remove(last);
            Ok(())
        }
        _ => Err(SecretPatchError::new(
            "secret_path_outside_schema",
            None::<String>,
            "secret clear only removes object fields",
        )),
    }
}

fn ensure_identity_shape(document: &RouteConfigYaml) -> Result<(), SecretPatchError> {
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

fn ensure_document_is_redacted(document: &RouteConfigYaml) -> Result<(), SecretPatchError> {
    for (provider_index, provider) in document.providers.iter().enumerate() {
        let provider_path = format!("/providers/{provider_index}");
        ensure_url_redacted(&provider.base_url, &format!("{provider_path}/base_url"))?;
        ensure_absent_string(&provider.api_key, &format!("{provider_path}/api_key"))?;
        ensure_absent_option(
            provider.auth_token.as_deref(),
            &format!("{provider_path}/auth_token"),
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

fn redact_url_value(value: &str) -> String {
    let Ok(mut url) = Url::parse(value) else {
        return if raw_url_may_contain_secret(value) {
            "<redacted-url>".to_string()
        } else {
            value.to_string()
        };
    };

    let has_userinfo = !url.username().is_empty() || url.password().is_some();
    let has_fragment = url.fragment().is_some();
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    let has_sensitive_query = pairs.iter().any(|(key, _)| is_sensitive_key(key));
    if !has_userinfo && !has_sensitive_query && !has_fragment {
        return value.to_string();
    }

    if has_userinfo {
        let _ = url.set_password(None);
        let _ = url.set_username("");
    }
    if has_fragment {
        url.set_fragment(None);
    }
    if has_sensitive_query {
        let safe_pairs = pairs
            .iter()
            .filter(|(key, _)| !is_sensitive_key(key))
            .map(|(key, value)| (key.as_str(), value.as_str()));
        url.query_pairs_mut().clear().extend_pairs(safe_pairs);
        if pairs.iter().all(|(key, _)| is_sensitive_key(key)) {
            url.set_query(None);
        }
    }
    url.into()
}

fn ensure_url_redacted(value: &str, path: &str) -> Result<(), SecretPatchError> {
    let unsafe_url = Url::parse(value).map_or_else(
        |_| raw_url_may_contain_secret(value),
        |url| {
            !url.username().is_empty()
                || url.password().is_some()
                || url
                    .query_pairs()
                    .any(|(key, _)| is_sensitive_key(key.as_ref()))
                || url.fragment().is_some()
        },
    );
    if unsafe_url {
        return Err(SecretPatchError::new(
            "secret_document_embedded_value",
            Some(path.to_string()),
            "draft URL still contains userinfo, a sensitive query parameter, or a fragment",
        ));
    }
    Ok(())
}

fn raw_url_may_contain_secret(value: &str) -> bool {
    let authority_has_userinfo = value
        .split_once("://")
        .map(|(_, remainder)| {
            remainder
                .split(['/', '?', '#'])
                .next()
                .is_some_and(|authority| authority.contains('@'))
        })
        .unwrap_or(false);
    let query_has_secret = value
        .split_once('?')
        .map(|(_, query)| query.split('#').next().unwrap_or(query))
        .into_iter()
        .flat_map(|query| url::form_urlencoded::parse(query.as_bytes()))
        .any(|(key, _)| is_sensitive_key(key.as_ref()));
    authority_has_userinfo || query_has_secret || value.split_once('#').is_some()
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

fn redact_string_map(
    map: &mut HashMap<String, String>,
    base_path: &str,
    descriptors: &mut Vec<SecretDescriptor>,
) {
    let mut keys: Vec<String> = map
        .keys()
        .filter(|key| is_sensitive_key(key))
        .cloned()
        .collect();
    keys.sort();
    for key in keys {
        if let Some(value) = map.remove(&key) {
            push_value_descriptor(
                descriptors,
                format!("{base_path}/{}", encode_pointer_segment(&key)),
                &Value::String(value),
                true,
            );
        }
    }
}

fn redact_extra_body_map(
    map: &mut HashMap<String, Value>,
    base_path: &str,
    descriptors: &mut Vec<SecretDescriptor>,
) {
    let mut keys: Vec<String> = map.keys().cloned().collect();
    keys.sort();
    for key in keys {
        let path = format!("{base_path}/{}", encode_pointer_segment(&key));
        if is_sensitive_key(&key) {
            if let Some(value) = map.remove(&key) {
                push_value_descriptor(descriptors, path, &value, true);
            }
        } else if let Some(value) = map.get_mut(&key) {
            redact_json_value(value, &path, descriptors);
        }
    }
}

fn redact_json_value(value: &mut Value, base_path: &str, descriptors: &mut Vec<SecretDescriptor>) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.sort();
            for key in keys {
                let path = format!("{base_path}/{}", encode_pointer_segment(&key));
                if is_sensitive_key(&key) {
                    if let Some(value) = map.remove(&key) {
                        push_value_descriptor(descriptors, path, &value, true);
                    }
                } else if let Some(value) = map.get_mut(&key) {
                    redact_json_value(value, &path, descriptors);
                }
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter_mut().enumerate() {
                redact_json_value(value, &format!("{base_path}/{index}"), descriptors);
            }
        }
        _ => {}
    }
}

fn push_string_descriptor(
    descriptors: &mut Vec<SecretDescriptor>,
    path: String,
    value: &str,
    configured: bool,
) {
    push_value_descriptor(
        descriptors,
        path,
        &Value::String(value.to_string()),
        configured,
    );
}

fn push_optional_string_descriptor(
    descriptors: &mut Vec<SecretDescriptor>,
    path: String,
    value: Option<&str>,
) {
    match value {
        Some(value) => push_string_descriptor(descriptors, path, value, true),
        None => descriptors.push(SecretDescriptor {
            path,
            configured: false,
            fingerprint: None,
            preview: None,
        }),
    }
}

fn push_value_descriptor(
    descriptors: &mut Vec<SecretDescriptor>,
    path: String,
    value: &Value,
    configured: bool,
) {
    let (fingerprint, preview) = if configured {
        (Some(fingerprint_value(value)), preview_value(value))
    } else {
        (None, None)
    };
    descriptors.push(SecretDescriptor {
        path,
        configured,
        fingerprint,
        preview,
    });
}

fn fingerprint_value(value: &Value) -> String {
    let encoded = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(encoded);
    format!("sha256:{}", &hex::encode(hasher.finalize())[..12])
}

fn preview_value(value: &Value) -> Option<String> {
    let value = value.as_str()?;
    let mut prefix = String::new();
    let mut suffix = VecDeque::with_capacity(3);
    let mut count = 0usize;
    for character in value.chars() {
        count += 1;
        if count <= 3 {
            prefix.push(character);
        }
        if suffix.len() == 3 {
            suffix.pop_front();
        }
        suffix.push_back(character);
    }
    if count == 0 {
        return None;
    }
    if count <= 8 {
        return Some("***".to_string());
    }
    let suffix: String = suffix.into_iter().collect();
    Some(format!("{prefix}***{suffix}"))
}

fn contains_mask_sentinel(value: &Value, previews: &HashSet<String>) -> bool {
    match value {
        Value::String(value) => is_basic_mask_sentinel(value) || previews.contains(value),
        Value::Array(values) => values
            .iter()
            .any(|value| contains_mask_sentinel(value, previews)),
        Value::Object(map) => map
            .values()
            .any(|value| contains_mask_sentinel(value, previews)),
        _ => false,
    }
}

fn contains_basic_mask_sentinel(value: &Value) -> bool {
    match value {
        Value::String(value) => is_basic_mask_sentinel(value),
        Value::Array(values) => values.iter().any(contains_basic_mask_sentinel),
        Value::Object(map) => map.values().any(contains_basic_mask_sentinel),
        _ => false,
    }
}

fn is_basic_mask_sentinel(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "***" | "..." | "…" | "[redacted]" | "<redacted>"
    ) || normalized.contains("***")
    {
        return true;
    }

    let has_ellipsis = normalized.contains("...") || normalized.contains('…');
    has_ellipsis
        && [
            "sk-", "pk-", "rk-", "sess-", "token-", "token ", "bearer ", "secret-", "secret ",
            "api-key", "apikey",
        ]
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
}

pub(crate) fn is_sensitive_key(key: &str) -> bool {
    let tokens = key_tokens(key);
    let compact = tokens.join("");
    if is_non_secret_metadata_key(&compact) {
        return false;
    }

    let exact = matches!(
        compact.as_str(),
        "authorization"
            | "proxyauthorization"
            | "cookie"
            | "setcookie"
            | "apikey"
            | "appkey"
            | "accesskey"
            | "secretkey"
            | "privatekey"
            | "token"
            | "session"
            | "sessionid"
            | "sessionkey"
            | "sapisid"
            | "secret"
            | "password"
            | "passwd"
            | "jwt"
            | "csrf"
            | "csrftoken"
            | "xsrf"
            | "xsrftoken"
    );
    let contains_secret_word = tokens
        .iter()
        .any(|token| matches!(token.as_str(), "secret" | "password" | "passwd"));
    let authorization_value = tokens.iter().any(|token| token == "authorization")
        && tokens.last().is_some_and(|token| {
            matches!(
                token.as_str(),
                "authorization" | "header" | "value" | "token"
            )
        });
    let sensitive_key_pair = tokens.windows(2).any(|pair| {
        matches!(
            (pair[0].as_str(), pair[1].as_str()),
            ("api", "key")
                | ("app", "key")
                | ("access", "key")
                | ("secret", "key")
                | ("private", "key")
        )
    });
    let token_value = tokens.iter().any(|token| token == "token")
        && tokens
            .last()
            .is_some_and(|token| matches!(token.as_str(), "token" | "value" | "secret"));
    let session_value = tokens.iter().any(|token| token == "session")
        && tokens.last().is_some_and(|token| {
            matches!(
                token.as_str(),
                "session" | "id" | "key" | "token" | "cookie" | "secret" | "value"
            )
        });
    let cookie_composite = tokens.iter().any(|token| token == "cookie")
        && tokens
            .last()
            .is_some_and(|token| matches!(token.as_str(), "value" | "header" | "token" | "secret"));

    exact
        || contains_secret_word
        || authorization_value
        || sensitive_key_pair
        || token_value
        || session_value
        || cookie_composite
        || compact.contains("awssecretaccesskey")
        || compact.contains("clientsecret")
        || compact.contains("authorizationheadervalue")
        || compact.ends_with("authorization")
        || compact.ends_with("cookie")
        || compact.ends_with("apikey")
        || compact.ends_with("token")
        || compact.ends_with("session")
        || compact.ends_with("secret")
        || compact.ends_with("password")
        || compact.ends_with("jwt")
}

fn key_tokens(key: &str) -> Vec<String> {
    let characters: Vec<char> = key.chars().collect();
    let mut tokens = Vec::new();
    let mut current = String::new();

    for (index, character) in characters.iter().copied().enumerate() {
        if !character.is_ascii_alphanumeric() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            continue;
        }

        let previous = index
            .checked_sub(1)
            .and_then(|previous| characters.get(previous));
        let next = characters.get(index + 1);
        let camel_boundary = character.is_ascii_uppercase()
            && !current.is_empty()
            && (previous.is_some_and(|previous| {
                previous.is_ascii_lowercase() || previous.is_ascii_digit()
            }) || (previous.is_some_and(|previous| previous.is_ascii_uppercase())
                && next.is_some_and(|next| next.is_ascii_lowercase())));
        if camel_boundary {
            tokens.push(std::mem::take(&mut current));
        }
        current.push(character.to_ascii_lowercase());
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn is_non_secret_metadata_key(compact: &str) -> bool {
    matches!(
        compact,
        "maxtokens"
            | "maxoutputtokens"
            | "maxcompletiontokens"
            | "inputtokens"
            | "outputtokens"
            | "tokencount"
            | "tokentype"
            | "tokenlimit"
            | "sessiontimeout"
    ) || compact.starts_with("tokenexpires")
        || compact.ends_with("tokenexpires")
        || compact.ends_with("tokenexpiresinsecs")
        || compact.ends_with("tokenendpoint")
        || compact.ends_with("tokencount")
        || compact.ends_with("sessiontimeout")
}
