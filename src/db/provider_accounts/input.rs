use super::UpsertProviderAccountInput;
use crate::error::GatewayError;
use crate::protocol::registry::{
    canonicalize_protocol_family_key, infer_protocol_family, infer_protocol_profile,
};
use crate::routing::candidate::ProviderExecutionMode;
use serde_json::Value;
use std::collections::HashMap;

pub(super) fn validate_provider_account_input(
    input: &UpsertProviderAccountInput,
) -> Result<(), GatewayError> {
    normalize_required_text(&input.label, "Provider account 标题", 120)?;
    normalize_service_provider_identity(
        &input.label,
        &input.service_provider_key,
        &input.service_provider_label,
    )?;
    normalize_required_text(&input.adapter, "adapter", 120)?;
    normalize_required_text(
        canonicalize_protocol_family_key(&input.protocol_family).as_str(),
        "protocolFamily",
        120,
    )?;

    let payload_adapter = input
        .payload
        .get("adapter")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .unwrap_or_default();
    if payload_adapter.is_empty() {
        return Err(GatewayError::bad_request("payload.adapter 不能为空"));
    }
    if payload_adapter != input.adapter.trim() {
        return Err(GatewayError::conflict(
            "payload.adapter 必须与 provider account adapter 一致",
        ));
    }
    let execution_mode =
        normalize_gateway_execution_mode(&input.adapter, input.execution_mode.as_deref())?;
    let endpoint_execution_modes =
        normalize_endpoint_execution_modes(&input.adapter, input.endpoint_execution_modes.clone())?;
    if execution_mode == ProviderExecutionMode::BrowserBacked
        && !adapter_supports_browser_backed_execution(&input.adapter)
    {
        return Err(GatewayError::conflict(format!(
            "{} 当前不支持 executionMode=browser_backed",
            input.adapter
        )));
    }
    if endpoint_execution_modes.as_ref().is_some_and(|values| {
        values
            .values()
            .any(|mode| *mode == ProviderExecutionMode::BrowserBacked)
    }) && !adapter_supports_browser_backed_execution(&input.adapter)
    {
        return Err(GatewayError::conflict(format!(
            "{} 当前不支持 endpointExecutionModes.*=browser_backed",
            input.adapter
        )));
    }
    Ok(())
}

pub(super) fn resolve_protocol_profile(
    protocol_profile: Option<&str>,
    adapter: &str,
    provider_hint: Option<&str>,
    base_url: Option<&str>,
) -> String {
    if base_url
        .is_some_and(crate::protocol::chatgpt::official_api::is_chatgpt_codex_backend_base_url)
    {
        return crate::protocol::chatgpt::official_api::CHATGPT_CODEX_BACKEND_PROFILE.to_string();
    }
    protocol_profile
        .map(str::trim)
        .filter(|value| !value.trim().is_empty())
        .map(|value| infer_protocol_profile(adapter, Some(value), base_url))
        .unwrap_or_else(|| infer_protocol_profile(adapter, provider_hint, base_url))
}

pub(super) fn resolve_protocol_family(
    protocol_family: &str,
    protocol_profile: Option<&str>,
    adapter: &str,
    provider_hint: Option<&str>,
    base_url: Option<&str>,
) -> String {
    infer_protocol_family(
        Some(protocol_family),
        adapter,
        protocol_profile.or(provider_hint),
        base_url,
    )
}

pub(super) fn payload_base_url(payload: &Value) -> Option<&str> {
    payload
        .get("baseUrl")
        .or_else(|| payload.get("base_url"))
        .and_then(|value| value.as_str())
}

fn adapter_supports_browser_backed_execution(adapter: &str) -> bool {
    matches!(
        adapter.trim(),
        "lumalabs_compatible"
            | "gemini_canvas_compatible"
            | "gemini_canvas_web_reverse_compatible"
            | "gemini_canvas_program_web_reverse_compatible"
            | "aistudio_web_reverse_compatible"
            | "producer_compatible"
            | "suno_compatible"
            | "udio_compatible"
    )
}

fn default_execution_mode_for_adapter(adapter: &str) -> ProviderExecutionMode {
    match adapter.trim() {
        "lumalabs_compatible"
        | "gemini_canvas_compatible"
        | "gemini_canvas_web_reverse_compatible"
        | "gemini_canvas_program_web_reverse_compatible"
        | "aistudio_web_reverse_compatible"
        | "suno_compatible"
        | "udio_compatible" => ProviderExecutionMode::BrowserBacked,
        _ => ProviderExecutionMode::DirectHttp,
    }
}

pub(super) fn normalize_gateway_execution_mode(
    adapter: &str,
    execution_mode: Option<&str>,
) -> Result<ProviderExecutionMode, GatewayError> {
    match execution_mode.map(|value| value.trim().to_lowercase()) {
        Some(value) if value == "direct_http" => Ok(ProviderExecutionMode::DirectHttp),
        Some(value) if value == "browser_backed" => {
            if !adapter_supports_browser_backed_execution(adapter) {
                Err(GatewayError::conflict(format!(
                    "{adapter} 当前不支持 executionMode=browser_backed"
                )))
            } else {
                Ok(ProviderExecutionMode::BrowserBacked)
            }
        }
        Some(_) => Err(GatewayError::bad_request("executionMode 不合法")),
        None => Ok(default_execution_mode_for_adapter(adapter)),
    }
}

pub(super) fn normalize_endpoint_execution_modes(
    adapter: &str,
    endpoint_execution_modes: Option<HashMap<String, String>>,
) -> Result<Option<HashMap<String, ProviderExecutionMode>>, GatewayError> {
    let mut normalized = HashMap::new();
    for (endpoint_kind, mode) in endpoint_execution_modes.unwrap_or_default() {
        let endpoint_kind = endpoint_kind.trim().to_lowercase();
        if endpoint_kind.is_empty() {
            continue;
        }
        normalized.insert(
            endpoint_kind,
            normalize_gateway_execution_mode(adapter, Some(mode.as_str()))?,
        );
    }
    if adapter.trim() == "producer_compatible" {
        normalized
            .entry("videos_generations".to_string())
            .or_insert(ProviderExecutionMode::BrowserBacked);
    }
    Ok((!normalized.is_empty()).then_some(normalized))
}

pub(super) fn parse_execution_mode(value: &str) -> Result<ProviderExecutionMode, GatewayError> {
    match value.trim().to_lowercase().as_str() {
        "direct_http" => Ok(ProviderExecutionMode::DirectHttp),
        "browser_backed" => Ok(ProviderExecutionMode::BrowserBacked),
        _ => Err(GatewayError::server_error("provider execution mode 非法")),
    }
}

pub(super) fn execution_mode_string(mode: ProviderExecutionMode) -> &'static str {
    match mode {
        ProviderExecutionMode::DirectHttp => "direct_http",
        ProviderExecutionMode::BrowserBacked => "browser_backed",
    }
}

pub(super) fn normalize_required_text(
    value: &str,
    label: &str,
    max_len: usize,
) -> Result<String, GatewayError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(GatewayError::bad_request(format!("{label} 不能为空")));
    }
    if trimmed.len() > max_len {
        return Err(GatewayError::bad_request(format!(
            "{label} 不能超过 {max_len} 个字符"
        )));
    }
    Ok(trimmed.to_string())
}

pub(super) fn normalize_optional_text(value: Option<&str>, max_len: usize) -> Option<String> {
    let trimmed = value?.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(max_len).collect())
}

pub(super) fn normalize_service_provider_identity(
    provider_label: &str,
    service_provider_key: &Option<String>,
    service_provider_label: &Option<String>,
) -> Result<(String, String), GatewayError> {
    let normalized_label = normalize_optional_text(service_provider_label.as_deref(), 120)
        .unwrap_or_else(|| provider_label.trim().to_string());
    if normalized_label.is_empty() {
        return Err(GatewayError::bad_request("serviceProviderLabel 不能为空"));
    }
    let normalized_key = match service_provider_key.as_deref() {
        Some(value) => normalize_service_provider_key(value)?,
        None => derive_service_provider_key_from_label(&normalized_label),
    };
    Ok((normalized_key, normalized_label))
}

fn normalize_service_provider_key(value: &str) -> Result<String, GatewayError> {
    let normalized = sanitize_service_provider_key(value);
    if normalized.is_empty() {
        return Err(GatewayError::bad_request(
            "serviceProviderKey 只允许字母、数字与分隔符，且归一化后不能为空",
        ));
    }
    if normalized.len() > 120 {
        return Err(GatewayError::bad_request(
            "serviceProviderKey 不能超过 120 个字符",
        ));
    }
    Ok(normalized)
}

fn derive_service_provider_key_from_label(label: &str) -> String {
    let normalized = sanitize_service_provider_key(label);
    if !normalized.is_empty() {
        return normalized;
    }
    format!("sp_{}", stable_service_provider_hash(label))
}

fn sanitize_service_provider_key(value: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_separator = false;
    for ch in value.trim().chars() {
        let lowered = ch.to_ascii_lowercase();
        if lowered.is_ascii_alphanumeric() {
            normalized.push(lowered);
            previous_was_separator = false;
            continue;
        }
        if matches!(ch, ' ' | '-' | '_' | '/' | '.' | ':') && !previous_was_separator {
            normalized.push('_');
            previous_was_separator = true;
        }
    }
    normalized.trim_matches('_').chars().take(120).collect()
}

fn stable_service_provider_hash(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
