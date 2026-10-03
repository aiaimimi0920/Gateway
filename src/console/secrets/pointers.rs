//! Typed RFC 6901 paths validate schema access before JSON mutation.

use super::classification::is_sensitive_key;
use super::SecretPatchError;
use crate::console::document::encode_pointer_segment;
use crate::routing::config::RouteConfigYaml;
use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SecretTarget {
    ProviderApiKey,
    OptionalString,
    Header,
    ExtraBody,
}

impl SecretTarget {
    pub(super) fn requires_string(self) -> bool {
        !matches!(self, Self::ExtraBody)
    }

    pub(super) fn is_map_value(self) -> bool {
        matches!(self, Self::Header | Self::ExtraBody)
    }
}

#[derive(Clone, Debug)]
pub(super) struct ParsedPointer {
    pub(super) segments: Vec<String>,
    pub(super) canonical: String,
}

impl ParsedPointer {
    pub(super) fn parse(path: &str) -> Result<Self, SecretPatchError> {
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

pub(super) fn pointer_error(path: &str) -> SecretPatchError {
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

pub(super) fn validate_secret_target(
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
        field
            if segments.len() == 4
                && super::storage_connections::secret_exists(provider, field, &segments[3]) =>
        {
            Ok(SecretTarget::OptionalString)
        }
        "api_key" if segments.len() == 3 => Ok(SecretTarget::ProviderApiKey),
        "auth_token" if segments.len() == 3 => Ok(SecretTarget::OptionalString),
        "credential_storage_password" if segments.len() == 3 => Ok(SecretTarget::OptionalString),
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
        "extra_body" if segments.len() >= 4 => validate_extra_body_target(draft_value, pointer, 2),
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
                    validate_extra_body_target(draft_value, pointer, 4)
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

pub(super) fn outside_schema(pointer: &ParsedPointer) -> SecretPatchError {
    SecretPatchError::new(
        "secret_path_outside_schema",
        Some(pointer.canonical.clone()),
        "path does not identify a supported secret field",
    )
}

pub(super) fn parse_array_index(
    segment: &str,
    pointer: &ParsedPointer,
) -> Result<usize, SecretPatchError> {
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

pub(super) fn format_pointer(segments: &[String]) -> String {
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

pub(super) fn value_at<'a>(value: &'a Value, segments: &[String]) -> Option<&'a Value> {
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

pub(super) fn parse_json_index(segment: &str) -> Option<usize> {
    if segment == "-"
        || segment.is_empty()
        || (segment.len() > 1 && segment.starts_with('0'))
        || !segment.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    segment.parse().ok()
}

pub(super) fn set_value(
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

pub(super) fn remove_value(value: &mut Value, segments: &[String]) -> Result<(), SecretPatchError> {
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
