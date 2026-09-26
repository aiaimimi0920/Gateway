use url::Url;

use super::api_keys::read_optional_hash_string;
use crate::routing::candidate::ProviderAccountPayload;

pub fn share_id_from_share_url(share_url: &str) -> Option<String> {
    let trimmed = share_url.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed = Url::parse(trimmed).ok()?;
    let mut segments = parsed.path_segments()?;
    let first = segments.next()?;
    let second = segments.next()?;
    if !first.eq_ignore_ascii_case("share") {
        return None;
    }
    let second = second.trim();
    if second.is_empty() {
        return None;
    }
    Some(second.to_string())
}

pub fn image_stream_generate_template_object_key(runtime_state_object_key: &str) -> String {
    let trimmed = runtime_state_object_key.trim();
    if let Some(prefix) = trimmed.strip_suffix("/storage-state.json") {
        return format!("{prefix}/image-stream-generate-template.json");
    }
    if let Some(prefix) = trimmed.strip_suffix("\\storage-state.json") {
        return format!("{prefix}/image-stream-generate-template.json");
    }
    format!("{trimmed}.image-stream-generate-template.json")
}

pub fn image_edit_stream_generate_template_object_key(runtime_state_object_key: &str) -> String {
    let trimmed = runtime_state_object_key.trim();
    if let Some(prefix) = trimmed.strip_suffix("/storage-state.json") {
        return format!("{prefix}/image-edit-stream-generate-template.json");
    }
    if let Some(prefix) = trimmed.strip_suffix("\\storage-state.json") {
        return format!("{prefix}/image-edit-stream-generate-template.json");
    }
    format!("{trimmed}.image-edit-stream-generate-template.json")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeminiCanvasPureHttpMode {
    Disabled,
    Preferred,
    Required,
}

pub fn pure_http_mode(payload: &ProviderAccountPayload) -> GeminiCanvasPureHttpMode {
    payload
        .extra_body
        .as_ref()
        .and_then(|extra| read_optional_hash_string(extra, &["pureHttpMode", "pure_http_mode"]))
        .map(|mode| {
            let normalized = mode.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "required" | "force" | "forced" | "strict" => GeminiCanvasPureHttpMode::Required,
                "0" | "false" | "off" | "disabled" | "none" => GeminiCanvasPureHttpMode::Disabled,
                "1" | "true" | "on" | "enabled" | "preferred" | "yes" => {
                    GeminiCanvasPureHttpMode::Preferred
                }
                _ => GeminiCanvasPureHttpMode::Preferred,
            }
        })
        .unwrap_or_else(|| {
            if std::env::var("GEMINI_CANVAS_PURE_HTTP_ENABLED")
                .ok()
                .map(|value| {
                    matches!(
                        value.trim().to_ascii_lowercase().as_str(),
                        "1" | "true" | "yes" | "on"
                    )
                })
                .unwrap_or(true)
            {
                GeminiCanvasPureHttpMode::Preferred
            } else {
                GeminiCanvasPureHttpMode::Disabled
            }
        })
}

pub fn pure_http_enabled(payload: &ProviderAccountPayload) -> bool {
    pure_http_mode(payload) != GeminiCanvasPureHttpMode::Disabled
}

pub fn pure_http_required(payload: &ProviderAccountPayload) -> bool {
    pure_http_mode(payload) == GeminiCanvasPureHttpMode::Required
}

pub fn browser_runtime_state_object_key(payload: &ProviderAccountPayload) -> Option<String> {
    let primary_runtime_state_object_key = payload
        .runtime_state_object_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if primary_runtime_state_object_key
        .as_deref()
        .is_some_and(runtime_state_object_key_looks_like_storage_state_file)
    {
        return primary_runtime_state_object_key;
    }
    payload
        .extra_body
        .as_ref()
        .and_then(|extra| {
            read_optional_hash_string(
                extra,
                &[
                    "browserRuntimeStateObjectKey",
                    "browserProfileRuntimeStateObjectKey",
                    "browser_runtime_state_object_key",
                    "browser_profile_runtime_state_object_key",
                ],
            )
        })
        .or(primary_runtime_state_object_key)
}

pub fn browser_profile_runtime_state_object_key(
    payload: &ProviderAccountPayload,
) -> Option<String> {
    payload.extra_body.as_ref().and_then(|extra| {
        read_optional_hash_string(
            extra,
            &[
                "browserRuntimeStateObjectKey",
                "browserProfileRuntimeStateObjectKey",
                "browser_runtime_state_object_key",
                "browser_profile_runtime_state_object_key",
            ],
        )
    })
}

pub fn browser_runtime_state_object_key_for_browser_operation(
    payload: &ProviderAccountPayload,
    operation: &str,
) -> Option<String> {
    let normalized_operation = operation.trim().to_ascii_lowercase();
    if matches!(
        normalized_operation.as_str(),
        "image" | "music" | "video" | "bootstrap_program"
    ) {
        return browser_profile_runtime_state_object_key(payload)
            .or_else(|| browser_runtime_state_object_key(payload));
    }
    browser_runtime_state_object_key(payload)
}

fn runtime_state_object_key_looks_like_storage_state_file(value: &str) -> bool {
    value
        .trim()
        .replace('\\', "/")
        .to_ascii_lowercase()
        .ends_with("/storage-state.json")
}

pub fn browser_cdp_url(payload: &ProviderAccountPayload) -> Option<String> {
    payload.extra_body.as_ref().and_then(|extra| {
        read_optional_hash_string(
            extra,
            &["browserCdpUrl", "browser_cdp_url", "browserDebuggerAddress"],
        )
    })
}

pub fn browser_cookie_header(payload: &ProviderAccountPayload) -> Option<String> {
    payload
        .extra_body
        .as_ref()
        .and_then(|extra| read_optional_hash_string(extra, &["cookieHeader", "cookie_header"]))
        .or_else(|| {
            payload.headers.iter().find_map(|(key, value)| {
                if key.eq_ignore_ascii_case("cookie") {
                    let trimmed = value.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                } else {
                    None
                }
            })
        })
}

pub fn image_json_fallback_enabled(payload: &ProviderAccountPayload) -> bool {
    payload
        .extra_body
        .as_ref()
        .and_then(|extra| {
            read_optional_hash_string(
                extra,
                &[
                    "imageJsonFallbackEnabled",
                    "image_json_fallback_enabled",
                    "imageJsonFallback",
                    "image_json_fallback",
                ],
            )
        })
        .map(|mode| {
            matches!(
                mode.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on" | "enabled"
            )
        })
        .unwrap_or_else(|| {
            std::env::var("GEMINI_CANVAS_IMAGE_JSON_FALLBACK_ENABLED")
                .ok()
                .map(|value| {
                    matches!(
                        value.trim().to_ascii_lowercase().as_str(),
                        "1" | "true" | "yes" | "on"
                    )
                })
                .unwrap_or(false)
        })
}

pub fn direct_http_auth_user(payload: &ProviderAccountPayload) -> String {
    payload
        .extra_body
        .as_ref()
        .and_then(|extra| {
            read_optional_hash_string(extra, &["googleAuthUser", "authUser", "auth_user"])
        })
        .unwrap_or_else(|| "0".to_string())
}
