use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;

use base64::Engine;
use deadpool_redis::Pool;
use rquest::header::{HeaderName, HeaderValue};
use rquest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use time::OffsetDateTime;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::{debug, warn};

use crate::credential_runtime::SessionAuthConfig;
use crate::db;
use crate::db::UpsertGatewaySessionInput;
use crate::error::{classify_network_error, classify_upstream_error, GatewayError};
use crate::http::request_headers::is_internal_gateway_header;
use crate::implementation_lines;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::gemini::shared::{
    GEMINI_API_MODULAR_ADAPTER, GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
    GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER, GEMINI_WEB_REVERSE_MODULAR_ADAPTER,
};
use crate::protocol::registry::{GEMINI_GENERATE_CONTENT_FAMILY, GEMINI_WEB_CHAT_FAMILY};
use crate::redis::credential_cache;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::request_time_browser_policy::{
    remote_browser_executor_required_unavailable_error, request_time_browser_forbidden_error,
    RequestTimeBrowserPolicy,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayKeepaliveEnsureRequest {
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "project_id")]
    project_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "session_key"
    )]
    session_key: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "previous_response_id"
    )]
    previous_response_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "credential_id"
    )]
    credential_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "account_name"
    )]
    account_name: Option<String>,
    provider_account_id: String,
    adapter: String,
    base_url: String,
    model: String,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "api_key")]
    api_key: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    headers: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "extra_body")]
    extra_body: Option<HashMap<String, Value>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "session_auth"
    )]
    session_auth: Option<SessionAuthConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "expires_at")]
    expires_at: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "runtime_state_object_key"
    )]
    runtime_state_object_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayKeepaliveEnsureResponse {
    ready: bool,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    headers: Option<HashMap<String, String>>,
    #[serde(default)]
    extra_body: Option<HashMap<String, Value>>,
    #[serde(default)]
    session_auth: Option<SessionAuthConfig>,
    #[serde(default)]
    keepalive: Option<crate::credential_runtime::KeepaliveConfig>,
    #[serde(default)]
    expires_at: Option<String>,
    #[serde(default)]
    runtime_state_object_key: Option<String>,
    #[serde(default)]
    upstream_session_id: Option<String>,
}

const QWEN_WEB_REFRESH_BEFORE_SECS: u64 = 300;
const QWEN_WEB_REFRESH_TIMEOUT_SECS: u64 = 120;
const QWEN_WEB_CREDENTIAL_FAMILY_DIR: &str = "qwen-web-chat";
const CHATGPT_WEB_REFRESH_BEFORE_SECS: u64 = 300;
const CHATGPT_WEB_OAUTH_REFRESH_BEFORE_SECS: u64 = 24 * 60 * 60;
const CHATGPT_WEB_REFRESH_TIMEOUT_SECS: u64 = 150;
const CHATGPT_WEB_OAUTH_TOKEN_ENDPOINT: &str = "https://auth.openai.com/oauth/token";
const CHATGPT_WEB_OAUTH_CLIENT_ID: &str = "app_2SKx67EdpoN0G6j64rFvigXD";
const CHATGPT_WEB_CREDENTIAL_FAMILY_DIR: &str = "chatgpt-platform/chatgpt-web-reverse/session-auth";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QwenWebSessionWorkerInput {
    base_url: String,
    preferred_models: Vec<String>,
    write_credential_file: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential_file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential_family_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential_root_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    browser_executable_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_data_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile_directory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_seed: Option<QwenWebSessionWorkerAuthSeed>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QwenWebSessionWorkerAuthSeed {
    email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    password_sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QwenWebSessionWorkerOutput {
    ok: bool,
    #[serde(default)]
    auth_token: Option<String>,
    #[serde(default)]
    expires_at: Option<String>,
    #[serde(default)]
    cookie_header: Option<String>,
    #[serde(default)]
    _selected_model: Option<String>,
    #[serde(default)]
    selected_display_model: Option<String>,
    #[serde(default)]
    auth_probe: Option<QwenWebSessionAuthProbe>,
    #[serde(default)]
    credential_file: Option<String>,
    #[serde(default)]
    error: Option<QwenWebSessionWorkerError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QwenWebSessionAuthProbe {
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QwenWebSessionWorkerError {
    #[serde(default)]
    status: Option<u16>,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Clone)]
struct QwenWebRefreshedRuntime {
    api_key: String,
    expires_at: Option<String>,
    cookie_header: Option<String>,
    account_name: Option<String>,
    selected_display_model: Option<String>,
    credential_material_key: Option<String>,
}

#[derive(Debug, Clone)]
struct QwenWebSigninSeed {
    email: String,
    password: Option<String>,
    password_sha256: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatGptWebSessionWorkerInput {
    base_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    models_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cookie_header: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chatgpt_pow_sources: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chatgpt_pow_data_build: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_build_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mailbox_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mailbox_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    proxy_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    proxy_bypass: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    registration_ip_country: Option<String>,
    write_credential_file: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential_file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential_family_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential_root_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    browser_executable_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_data_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile_directory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_seed: Option<ChatGptWebSessionWorkerAuthSeed>,
    #[serde(skip_serializing_if = "Option::is_none")]
    relay_request: Option<ChatGptWebSessionWorkerRelayRequest>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatGptWebSessionWorkerAuthSeed {
    email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    password_sha256: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatGptWebSessionWorkerRelayRequest {
    stream: bool,
    body: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatGptWebSessionWorkerOutput {
    ok: bool,
    #[serde(default)]
    auth_token: Option<String>,
    #[serde(default)]
    expires_at: Option<String>,
    #[serde(default)]
    cookie_header: Option<String>,
    #[serde(default)]
    device_id: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    client_version: Option<String>,
    #[serde(default)]
    client_build_number: Option<String>,
    #[serde(default)]
    user_agent: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    language_code: Option<String>,
    #[serde(default)]
    timezone: Option<String>,
    #[serde(default)]
    chatgpt_pow_sources: Option<Vec<String>>,
    #[serde(default)]
    chatgpt_pow_data_build: Option<String>,
    #[serde(default)]
    account_name: Option<String>,
    #[serde(default)]
    credential_material_key: Option<String>,
    #[serde(default)]
    auth_probe: Option<ChatGptWebSessionAuthProbe>,
    #[serde(default)]
    credential_file: Option<String>,
    #[serde(default)]
    relay_response: Option<ChatGptWebSessionWorkerRelayResponse>,
    #[serde(default)]
    error: Option<ChatGptWebSessionWorkerError>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatGptWebSessionWorkerRelayResponse {
    #[serde(default)]
    requirements_status: Option<u16>,
    #[serde(default)]
    requirements_content_type: Option<String>,
    #[serde(default)]
    requirements_preview: Option<String>,
    #[serde(default)]
    status: Option<u16>,
    #[serde(default)]
    content_type: Option<String>,
    #[serde(default)]
    body_text: Option<String>,
    #[serde(default)]
    body_preview: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatGptWebSessionAuthProbe {
    #[serde(default)]
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatGptWebSessionWorkerError {
    #[serde(default)]
    status: Option<u16>,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatGptWebOAuthTokenResponse {
    #[serde(alias = "access_token", alias = "apiKey", alias = "api_key")]
    access_token: String,
    #[serde(default, alias = "refresh_token")]
    refresh_token: Option<String>,
    #[serde(default, alias = "id_token")]
    id_token: Option<String>,
    #[serde(default, alias = "expires_in")]
    expires_in: Option<u64>,
}

#[derive(Debug, Clone)]
struct ChatGptWebRefreshedRuntime {
    api_key: String,
    expires_at: Option<String>,
    refresh_token: Option<String>,
    id_token: Option<String>,
    cookie_header: Option<String>,
    device_id: Option<String>,
    session_id: Option<String>,
    client_version: Option<String>,
    client_build_number: Option<String>,
    user_agent: Option<String>,
    language: Option<String>,
    language_code: Option<String>,
    timezone: Option<String>,
    chatgpt_pow_sources: Option<Vec<String>>,
    chatgpt_pow_data_build: Option<String>,
    account_name: Option<String>,
    credential_material_key: Option<String>,
}

#[derive(Debug, Clone)]
struct ChatGptWebSessionWorkerExecution {
    refreshed: ChatGptWebRefreshedRuntime,
    relay_response: Option<ChatGptWebSessionWorkerRelayResponse>,
}

#[derive(Debug, Clone)]
pub struct ChatGptWebBrowserRelayResult {
    pub payload: ProviderAccountPayload,
    pub status: u16,
    pub content_type: Option<String>,
    pub body_text: String,
}

#[derive(Debug, Clone)]
struct ChatGptWebSigninSeed {
    email: String,
    password: Option<String>,
    password_sha256: Option<String>,
}

fn normalize_keepalive_session_auth(value: Option<SessionAuthConfig>) -> Option<SessionAuthConfig> {
    let mut session_auth = value?;
    let transport = match session_auth.transport.trim().to_lowercase().as_str() {
        "bearer" => "bearer",
        "header" => "header",
        _ => "cookie",
    }
    .to_string();
    session_auth.transport = transport.clone();

    if transport == "cookie" {
        if session_auth.primary_cookie_name.is_none() {
            session_auth.primary_cookie_name = Some("sso".to_string());
        }
        if session_auth.secondary_cookie_name.is_none() {
            session_auth.secondary_cookie_name = Some("sso-rw".to_string());
        }
        session_auth.header_name = None;
    } else if transport == "bearer" {
        if session_auth.header_name.is_none() {
            session_auth.header_name = Some("authorization".to_string());
        }
    } else if session_auth.header_name.is_none() {
        session_auth.header_name = Some("x-session-token".to_string());
    }

    Some(session_auth)
}

fn truncate_error_summary(value: &str, max_length: usize) -> String {
    let message = value.trim();
    if message.len() <= max_length {
        return message.to_string();
    }

    let mut truncated = message
        .chars()
        .take(max_length.saturating_sub(1))
        .collect::<String>();
    truncated.push('…');
    truncated
}

fn decode_jwt_expiry_iso(token: Option<&str>) -> Option<String> {
    let token = token?.trim();
    if token.is_empty() {
        return None;
    }

    let payload = token.split('.').nth(1)?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()
        .or_else(|| {
            base64::engine::general_purpose::STANDARD
                .decode(payload)
                .ok()
        })?;
    let value = serde_json::from_slice::<Value>(&decoded).ok()?;
    let exp = value.get("exp")?.as_i64()?;
    let expires_at = OffsetDateTime::from_unix_timestamp(exp).ok()?;
    Some(db::format_timestamp(expires_at))
}

fn parse_rfc3339(value: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(value.trim(), &time::format_description::well_known::Rfc3339).ok()
}

fn build_keepalive_probe_headers(
    headers: Option<&HashMap<String, String>>,
    session_auth: Option<&SessionAuthConfig>,
    api_key: &str,
) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for (key, value) in headers.into_iter().flat_map(|items| items.iter()) {
        let normalized = key.trim().to_ascii_lowercase();
        if key.trim().is_empty()
            || value.trim().is_empty()
            || normalized == "content-type"
            || normalized == "authorization"
            || normalized == "cookie"
            || is_internal_gateway_header(&normalized)
        {
            continue;
        }
        result.insert(key.trim().to_string(), value.trim().to_string());
    }

    match session_auth
        .map(|config| config.transport.as_str())
        .unwrap_or("cookie")
    {
        "bearer" => {
            let header_name = session_auth
                .and_then(SessionAuthConfig::header_name)
                .unwrap_or("authorization")
                .to_string();
            if !is_internal_gateway_header(&header_name) {
                result.insert(header_name, format!("Bearer {api_key}"));
            }
        }
        "header" => {
            let header_name = session_auth
                .and_then(SessionAuthConfig::header_name)
                .unwrap_or("x-session-token")
                .to_string();
            if !is_internal_gateway_header(&header_name) {
                result.insert(header_name, api_key.to_string());
            }
        }
        _ => {
            let primary = session_auth
                .and_then(|config| config.primary_cookie_name.clone())
                .unwrap_or_else(|| "token".to_string());
            let secondary = session_auth.and_then(|config| config.secondary_cookie_name.clone());
            let mut cookies = vec![format!("{primary}={api_key}")];
            if let Some(secondary) = secondary.filter(|value| !value.trim().is_empty()) {
                cookies.push(format!("{}={}", secondary.trim(), api_key));
            }
            result.insert("cookie".to_string(), cookies.join("; "));
        }
    }

    result
}

fn hash_runtime_session_key(provider_account_id: &str, session_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(session_key.as_bytes());
    let digest = hasher.finalize();
    format!(
        "ai-gateway/runtime-session/{}/{}.tar.gz",
        provider_account_id.trim(),
        hex::encode(&digest)[..24].to_string()
    )
}

fn adapter_uses_runtime_archive(adapter: &str) -> bool {
    matches!(
        adapter.trim(),
        "codex_cli" | "claude_code" | "udio_compatible"
    )
}

fn is_gemini_canvas_keepalive_adapter(adapter: &str) -> bool {
    matches!(
        adapter.trim(),
        "gemini_canvas_compatible"
            | GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER
            | GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER
    )
}

fn infer_keepalive_protocol_family(adapter: &str) -> String {
    match adapter.trim() {
        "anthropic_compatible" => "anthropic",
        "kiro_compatible" => "kiro",
        "search_api_compatible" | "linkup_compatible" => "search",
        "producer_compatible" => "producer",
        "suno_compatible" => "suno",
        "udio_compatible" => "udio",
        GEMINI_API_MODULAR_ADAPTER => GEMINI_GENERATE_CONTENT_FAMILY,
        GEMINI_WEB_REVERSE_MODULAR_ADAPTER => GEMINI_WEB_CHAT_FAMILY,
        _ if is_gemini_canvas_keepalive_adapter(adapter) => "gemini_canvas",
        "codex_cli" => "codex",
        "claude_code" => "claude",
        "provider_passthrough" => "provider_passthrough",
        _ => "openai",
    }
    .to_string()
}

fn read_extra_body_string(
    extra_body: Option<&HashMap<String, Value>>,
    aliases: &[&str],
) -> Option<String> {
    let extra_body = extra_body?;
    let normalized_aliases = aliases
        .iter()
        .map(|alias| alias.replace(['_', '-'], "").to_ascii_lowercase())
        .collect::<Vec<_>>();
    for (key, value) in extra_body {
        let normalized_key = key.replace(['_', '-'], "").to_ascii_lowercase();
        if !normalized_aliases.contains(&normalized_key) {
            continue;
        }
        match value {
            Value::String(text) if !text.trim().is_empty() => return Some(text.trim().to_string()),
            Value::Number(number) => return Some(number.to_string()),
            Value::Bool(flag) => return Some(flag.to_string()),
            _ => {}
        }
    }
    None
}

fn read_extra_body_bool(
    extra_body: Option<&HashMap<String, Value>>,
    aliases: &[&str],
) -> Option<bool> {
    let text = read_extra_body_string(extra_body, aliases)?;
    match text.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" | "enabled" => Some(true),
        "0" | "false" | "no" | "off" | "disabled" | "never" => Some(false),
        _ => None,
    }
}

fn read_nested_json_string(value: Option<&Value>, aliases: &[&str]) -> Option<String> {
    let object = value?.as_object()?;
    let normalized_aliases = aliases
        .iter()
        .map(|alias| alias.replace(['_', '-'], "").to_ascii_lowercase())
        .collect::<Vec<_>>();
    for (key, value) in object {
        let normalized_key = key.replace(['_', '-'], "").to_ascii_lowercase();
        if !normalized_aliases.contains(&normalized_key) {
            continue;
        }
        if let Some(text) = value
            .as_str()
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_string)
        {
            return Some(text);
        }
    }
    None
}

fn read_json_field_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
}

fn qwen_web_signin_seed(extra_body: Option<&HashMap<String, Value>>) -> Option<QwenWebSigninSeed> {
    let extra_body = extra_body?;
    let auth_seed = extra_body
        .get("authSeed")
        .or_else(|| extra_body.get("auth_seed"))
        .or_else(|| extra_body.get("signinSeed"))
        .or_else(|| extra_body.get("signin_seed"));

    let email = read_nested_json_string(
        auth_seed,
        &[
            "email",
            "loginEmail",
            "login_email",
            "account",
            "accountEmail",
        ],
    )
    .or_else(|| {
        read_extra_body_string(
            Some(extra_body),
            &[
                "email",
                "loginEmail",
                "login_email",
                "account",
                "accountEmail",
            ],
        )
    })?;

    let password =
        read_nested_json_string(auth_seed, &["password", "loginPassword", "login_password"])
            .or_else(|| {
                read_extra_body_string(
                    Some(extra_body),
                    &["password", "loginPassword", "login_password"],
                )
            });
    let password_sha256 = read_nested_json_string(
        auth_seed,
        &[
            "passwordSha256",
            "password_sha256",
            "passwordHash",
            "password_hash",
        ],
    )
    .or_else(|| {
        read_extra_body_string(
            Some(extra_body),
            &[
                "passwordSha256",
                "password_sha256",
                "passwordHash",
                "password_hash",
            ],
        )
    });

    if password.is_none() && password_sha256.is_none() {
        return None;
    }

    Some(QwenWebSigninSeed {
        email,
        password,
        password_sha256,
    })
}

fn sha256_hex(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}

fn qwen_web_signin_password_attempts(seed: &QwenWebSigninSeed) -> Vec<String> {
    let mut attempts = Vec::new();
    if let Some(password) = seed
        .password
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        attempts.push(password.to_string());
        let hashed = sha256_hex(password);
        if !attempts.contains(&hashed) {
            attempts.push(hashed);
        }
    }
    if let Some(password_sha256) = seed
        .password_sha256
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if !attempts.iter().any(|entry| entry == password_sha256) {
            attempts.push(password_sha256.to_string());
        }
    }
    attempts
}

fn qwen_web_signin_headers(payload: &ProviderAccountPayload) -> HashMap<String, String> {
    let base_url = payload.base_url.trim_end_matches('/').to_string();
    let mut headers = external_gateway_headers(&payload.headers);
    let defaults = [
        (
            "User-Agent".to_string(),
            crate::protocol::qwen_web::QWEN_WEB_DEFAULT_USER_AGENT.to_string(),
        ),
        ("Connection".to_string(), "keep-alive".to_string()),
        (
            "Accept".to_string(),
            "application/json, text/plain, */*".to_string(),
        ),
        (
            "Accept-Encoding".to_string(),
            "gzip, deflate, br, zstd".to_string(),
        ),
        (
            "Timezone".to_string(),
            crate::protocol::qwen_web::QWEN_WEB_DEFAULT_TIMEZONE.to_string(),
        ),
        (
            "sec-ch-ua".to_string(),
            crate::protocol::qwen_web::QWEN_WEB_DEFAULT_SEC_CH_UA.to_string(),
        ),
        ("source".to_string(), "web".to_string()),
        (
            "Version".to_string(),
            crate::protocol::qwen_web::QWEN_WEB_DEFAULT_VERSION.to_string(),
        ),
        (
            "bx-v".to_string(),
            crate::protocol::qwen_web::QWEN_WEB_DEFAULT_BX_VERSION.to_string(),
        ),
        ("Sec-Fetch-Site".to_string(), "same-origin".to_string()),
        ("Sec-Fetch-Mode".to_string(), "cors".to_string()),
        ("Sec-Fetch-Dest".to_string(), "empty".to_string()),
        (
            "Accept-Language".to_string(),
            crate::protocol::qwen_web::QWEN_WEB_DEFAULT_ACCEPT_LANGUAGE.to_string(),
        ),
        ("Origin".to_string(), base_url.clone()),
        ("Referer".to_string(), format!("{base_url}/auth")),
        ("Content-Type".to_string(), "application/json".to_string()),
    ];
    for (key, value) in defaults {
        if !headers.contains_key(&key) {
            headers.insert(key, value);
        }
    }
    headers.remove("Authorization");
    headers.remove("authorization");
    headers
}

fn chatgpt_web_signin_seed(
    extra_body: Option<&HashMap<String, Value>>,
) -> Option<ChatGptWebSigninSeed> {
    let extra_body = extra_body?;
    let auth_seed = extra_body
        .get("authSeed")
        .or_else(|| extra_body.get("auth_seed"))
        .or_else(|| extra_body.get("signinSeed"))
        .or_else(|| extra_body.get("signin_seed"));

    let email = read_nested_json_string(
        auth_seed,
        &[
            "email",
            "loginEmail",
            "login_email",
            "account",
            "accountEmail",
        ],
    )
    .or_else(|| {
        read_extra_body_string(
            Some(extra_body),
            &[
                "email",
                "loginEmail",
                "login_email",
                "account",
                "accountEmail",
            ],
        )
    })?;

    let password =
        read_nested_json_string(auth_seed, &["password", "loginPassword", "login_password"])
            .or_else(|| {
                read_extra_body_string(
                    Some(extra_body),
                    &["password", "loginPassword", "login_password"],
                )
            });
    let password_sha256 = read_nested_json_string(
        auth_seed,
        &[
            "passwordSha256",
            "password_sha256",
            "passwordHash",
            "password_hash",
        ],
    )
    .or_else(|| {
        read_extra_body_string(
            Some(extra_body),
            &[
                "passwordSha256",
                "password_sha256",
                "passwordHash",
                "password_hash",
            ],
        )
    });

    if password.is_none() && password_sha256.is_none() {
        return None;
    }

    Some(ChatGptWebSigninSeed {
        email,
        password,
        password_sha256,
    })
}

fn chatgpt_web_session_worker_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("chatgpt-web-session-worker.mjs")
}

fn chatgpt_web_effective_session_auth(
    payload: &ProviderAccountPayload,
) -> Option<SessionAuthConfig> {
    let mut session_auth = payload.session_auth.clone()?;
    if session_auth.expires_at.is_none() {
        session_auth.expires_at = payload.expires_at.clone();
    }
    Some(session_auth)
}

fn chatgpt_web_should_refresh(payload: &ProviderAccountPayload, force_refresh: bool) -> bool {
    if force_refresh {
        return true;
    }
    if payload.api_key.trim().is_empty() {
        let has_cookie_header = read_header_case_insensitive(&payload.headers, "Cookie")
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
        if !has_cookie_header {
            return true;
        }
    }
    match chatgpt_web_effective_session_auth(payload) {
        Some(session_auth) => session_auth.expires_within_secs(CHATGPT_WEB_REFRESH_BEFORE_SECS),
        None => false,
    }
}

fn chatgpt_web_oauth_refresh_token(payload: &ProviderAccountPayload) -> Option<String> {
    read_extra_body_string(
        payload.extra_body.as_ref(),
        &[
            "refreshToken",
            "refresh_token",
            "oauthRefreshToken",
            "openaiRefreshToken",
        ],
    )
}

fn chatgpt_web_oauth_token_endpoint(payload: &ProviderAccountPayload) -> String {
    read_extra_body_string(
        payload.extra_body.as_ref(),
        &[
            "oauthTokenEndpoint",
            "oauthTokenUrl",
            "tokenEndpoint",
            "refreshEndpoint",
        ],
    )
    .unwrap_or_else(|| CHATGPT_WEB_OAUTH_TOKEN_ENDPOINT.to_string())
}

fn chatgpt_web_oauth_client_id(payload: &ProviderAccountPayload) -> String {
    read_extra_body_string(
        payload.extra_body.as_ref(),
        &["oauthClientId", "clientId", "openaiOAuthClientId"],
    )
    .unwrap_or_else(|| CHATGPT_WEB_OAUTH_CLIENT_ID.to_string())
}

pub(crate) fn chatgpt_web_oauth_refresh_due(
    payload: &ProviderAccountPayload,
    refresh_before_secs: u64,
    force_refresh: bool,
) -> bool {
    if chatgpt_web_oauth_refresh_token(payload).is_none() {
        return false;
    }
    if force_refresh || payload.api_key.trim().is_empty() {
        return true;
    }
    match chatgpt_web_effective_session_auth(payload) {
        Some(session_auth) => session_auth.expires_within_secs(refresh_before_secs),
        None => payload
            .expires_at
            .as_deref()
            .and_then(parse_rfc3339)
            .map(|expires_at| {
                expires_at
                    <= OffsetDateTime::now_utc()
                        + time::Duration::seconds(refresh_before_secs.min(i64::MAX as u64) as i64)
            })
            .unwrap_or(false),
    }
}

fn chatgpt_web_should_oauth_refresh(payload: &ProviderAccountPayload, force_refresh: bool) -> bool {
    chatgpt_web_oauth_refresh_due(
        payload,
        CHATGPT_WEB_OAUTH_REFRESH_BEFORE_SECS,
        force_refresh,
    )
}

fn request_time_local_browser_worker_blocking_error(
    policy: RequestTimeBrowserPolicy,
    provider: &str,
) -> Option<GatewayError> {
    match policy {
        RequestTimeBrowserPolicy::Disabled => Some(
            request_time_browser_forbidden_error(
                "Request-time local browser execution is disabled by GATEWAY_REQUEST_TIME_BROWSER_POLICY=disabled.",
            )
            .with_provider(provider),
        ),
        RequestTimeBrowserPolicy::RemoteOnly => Some(
            remote_browser_executor_required_unavailable_error(
                "Remote browser executor is required by GATEWAY_REQUEST_TIME_BROWSER_POLICY=remote_only, but keepalive can only launch a local browser worker.",
            )
            .with_provider(provider),
        ),
        RequestTimeBrowserPolicy::LocalAllowed => None,
    }
}

fn chatgpt_web_provider_request_time_browser_fallback_allowed(
    payload: &ProviderAccountPayload,
) -> bool {
    match std::env::var("CHATGPT_WEB_REVERSE_REQUEST_TIME_BROWSER")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("0" | "false" | "disabled" | "never" | "off" | "pure_http_only" | "browserless") => {
            return false;
        }
        _ => {}
    }
    if let Some(allowed) = read_extra_body_bool(
        payload.extra_body.as_ref(),
        &[
            "requestTimeBrowserAllowed",
            "chatgptWebRequestTimeBrowserAllowed",
        ],
    ) {
        return allowed;
    }
    match read_extra_body_string(
        payload.extra_body.as_ref(),
        &[
            "requestTimeBrowserMode",
            "chatgptWebRequestTimeBrowserMode",
            "browserFallbackMode",
        ],
    )
    .map(|value| value.trim().to_ascii_lowercase())
    .as_deref()
    {
        Some("disabled" | "never" | "off" | "pure_http_only" | "browserless" | "fail_fast") => {
            false
        }
        _ => true,
    }
}

fn chatgpt_web_request_time_browser_fallback_allowed_for_policy(
    payload: &ProviderAccountPayload,
    policy: RequestTimeBrowserPolicy,
) -> bool {
    request_time_local_browser_worker_blocking_error(policy, "chatgpt_web_reverse_compatible")
        .is_none()
        && chatgpt_web_provider_request_time_browser_fallback_allowed(payload)
}

fn chatgpt_web_request_time_browser_fallback_allowed(payload: &ProviderAccountPayload) -> bool {
    chatgpt_web_request_time_browser_fallback_allowed_for_policy(
        payload,
        RequestTimeBrowserPolicy::from_env(),
    )
}

fn future_iso_after_secs(seconds: u64) -> Option<String> {
    let seconds = seconds.min(i64::MAX as u64) as i64;
    Some(db::format_timestamp(
        OffsetDateTime::now_utc() + time::Duration::seconds(seconds),
    ))
}

async fn execute_chatgpt_web_oauth_refresh(
    http: &Client,
    payload: &ProviderAccountPayload,
) -> Result<ChatGptWebRefreshedRuntime, GatewayError> {
    let refresh_token = chatgpt_web_oauth_refresh_token(payload).ok_or_else(|| {
        GatewayError::bad_request(
            "ChatGPT Web OAuth refresh requested, but credential payload has no refreshToken.",
        )
        .with_provider("chatgpt_web_reverse_compatible")
        .with_code("chatgpt_web_oauth_refresh_token_missing")
    })?;
    let endpoint = chatgpt_web_oauth_token_endpoint(payload);
    let client_id = chatgpt_web_oauth_client_id(payload);
    let user_agent = read_extra_body_string(payload.extra_body.as_ref(), &["userAgent", "ua"])
        .or_else(|| read_header_case_insensitive(&payload.headers, "User-Agent"))
        .unwrap_or_else(|| {
            crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_DEFAULT_USER_AGENT.to_string()
        });
    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token.as_str()),
        ("client_id", client_id.as_str()),
    ];
    let response = http
        .post(&endpoint)
        .header("Accept", "application/json")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("User-Agent", user_agent)
        .form(&params)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some("chatgpt_web_reverse_compatible")))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(GatewayError::service_unavailable(format!(
            "ChatGPT Web OAuth refresh failed with HTTP {status}: {}",
            truncate_error_summary(&body, 300)
        ))
        .with_provider("chatgpt_web_reverse_compatible")
        .with_code("chatgpt_web_oauth_refresh_failed"));
    }

    let token_response: ChatGptWebOAuthTokenResponse = response.json().await.map_err(|error| {
        GatewayError::server_error(format!(
            "ChatGPT Web OAuth refresh response could not be decoded: {error}"
        ))
        .with_provider("chatgpt_web_reverse_compatible")
        .with_code("chatgpt_web_oauth_refresh_decode_failed")
    })?;
    let access_token = token_response.access_token.trim().to_string();
    if access_token.is_empty() {
        return Err(GatewayError::server_error(
            "ChatGPT Web OAuth refresh response did not include access_token.",
        )
        .with_provider("chatgpt_web_reverse_compatible")
        .with_code("chatgpt_web_oauth_refresh_missing_access_token"));
    }
    let rotated_refresh_token = token_response
        .refresh_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| Some(refresh_token));
    let id_token = token_response
        .id_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let expires_at = decode_jwt_expiry_iso(Some(&access_token))
        .or_else(|| token_response.expires_in.and_then(future_iso_after_secs));

    Ok(ChatGptWebRefreshedRuntime {
        api_key: access_token,
        expires_at,
        refresh_token: rotated_refresh_token,
        id_token,
        cookie_header: None,
        device_id: None,
        session_id: None,
        client_version: None,
        client_build_number: None,
        user_agent: None,
        language: None,
        language_code: None,
        timezone: None,
        chatgpt_pow_sources: None,
        chatgpt_pow_data_build: None,
        account_name: None,
        credential_material_key: None,
    })
}

fn sanitize_chatgpt_web_credential_file_name(value: &str) -> Option<String> {
    let normalized = value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| match ch {
            'a'..='z' | '0'..='9' | '.' | '_' | '-' => ch,
            _ => '-',
        })
        .collect::<String>();
    let collapsed = normalized
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if collapsed.is_empty() {
        None
    } else if collapsed.ends_with(".json") {
        Some(collapsed)
    } else {
        Some(format!("{collapsed}.json"))
    }
}

fn chatgpt_web_refresh_input(payload: &ProviderAccountPayload) -> ChatGptWebSessionWorkerInput {
    let credential_file_name = payload
        .credential_id
        .as_deref()
        .and_then(sanitize_chatgpt_web_credential_file_name)
        .or_else(|| {
            payload
                .account_name
                .as_deref()
                .and_then(sanitize_chatgpt_web_credential_file_name)
        });
    let cookie_header = read_header_case_insensitive(&payload.headers, "Cookie");
    ChatGptWebSessionWorkerInput {
        base_url: crate::protocol::chatgpt::web_reverse::normalize_site_base_url(&payload.base_url),
        models_path: payload
            .extra_body
            .as_ref()
            .and_then(|body| body.get("modelsPath"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| {
                Some(
                    crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_DEFAULT_MODELS_PATH
                        .to_string(),
                )
            }),
        auth_url: payload
            .extra_body
            .as_ref()
            .and_then(|body| body.get("chatgptAuthUrl"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        auth_token: {
            let api_key = payload.api_key.trim();
            (!api_key.is_empty()).then_some(api_key.to_string())
        },
        cookie_header,
        user_agent: read_extra_body_string(payload.extra_body.as_ref(), &["userAgent", "ua"])
            .or_else(|| read_header_case_insensitive(&payload.headers, "User-Agent")),
        language: read_extra_body_string(
            payload.extra_body.as_ref(),
            &["language", "acceptLanguage"],
        )
        .or_else(|| read_header_case_insensitive(&payload.headers, "Accept-Language")),
        language_code: read_extra_body_string(
            payload.extra_body.as_ref(),
            &["languageCode", "oaiLanguage"],
        ),
        timezone: read_extra_body_string(payload.extra_body.as_ref(), &["timezone"]),
        chatgpt_pow_sources: payload
            .extra_body
            .as_ref()
            .and_then(|extra_body| extra_body.get("chatgptPowSources"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .filter(|items| !items.is_empty()),
        chatgpt_pow_data_build: read_extra_body_string(
            payload.extra_body.as_ref(),
            &["chatgptPowDataBuild", "powDataBuild"],
        ),
        client_version: read_extra_body_string(payload.extra_body.as_ref(), &["clientVersion"]),
        client_build_number: read_extra_body_string(
            payload.extra_body.as_ref(),
            &["clientBuildNumber"],
        ),
        device_id: read_extra_body_string(
            payload.extra_body.as_ref(),
            &["deviceId", "oaiDeviceId"],
        ),
        session_id: read_extra_body_string(
            payload.extra_body.as_ref(),
            &["sessionId", "oaiSessionId"],
        ),
        mailbox_ref: read_extra_body_string(payload.extra_body.as_ref(), &["mailboxRef"]),
        mailbox_session_id: read_extra_body_string(
            payload.extra_body.as_ref(),
            &["mailboxSessionId"],
        ),
        proxy_url: read_extra_body_string(
            payload.extra_body.as_ref(),
            &[
                "proxyUrl",
                "proxy_url",
                "credentialProxyUrl",
                "credential_proxy_url",
                "outboundProxy",
                "outbound_proxy",
            ],
        ),
        proxy_bypass: read_extra_body_string(
            payload.extra_body.as_ref(),
            &["proxyBypass", "proxy_bypass", "noProxy", "no_proxy"],
        ),
        registration_ip_country: read_extra_body_string(
            payload.extra_body.as_ref(),
            &[
                "registrationIpCountry",
                "registration_ip_country",
                "ipCountry",
                "ip_country",
            ],
        ),
        write_credential_file: true,
        credential_file_name,
        credential_family_dir: Some(CHATGPT_WEB_CREDENTIAL_FAMILY_DIR.to_string()),
        credential_root_dir: std::env::var("NEURO_PROVIDER_CREDENTIAL_ROOT_DIR")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        browser_executable_path: std::env::var("CHATGPT_WEB_BROWSER_EXECUTABLE_PATH")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        user_data_dir: std::env::var("CHATGPT_WEB_BROWSER_USER_DATA_DIR")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        profile_directory: std::env::var("CHATGPT_WEB_BROWSER_PROFILE_DIR")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        auth_seed: chatgpt_web_signin_seed(payload.extra_body.as_ref()).map(|seed| {
            ChatGptWebSessionWorkerAuthSeed {
                email: seed.email,
                password: seed.password,
                password_sha256: seed.password_sha256,
            }
        }),
        relay_request: None,
    }
}

fn collect_set_cookie_header(headers: &rquest::header::HeaderMap) -> Option<String> {
    let mut cookies = Vec::new();
    for value in headers.get_all(rquest::header::SET_COOKIE) {
        let Ok(value) = value.to_str() else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        if let Some(cookie) = value
            .split(';')
            .next()
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
        {
            if !cookies.iter().any(|existing: &String| existing == cookie) {
                cookies.push(cookie.to_string());
            }
        }
    }
    if cookies.is_empty() {
        None
    } else {
        Some(cookies.join("; "))
    }
}

fn read_header_case_insensitive(headers: &HashMap<String, String>, name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(key, value)| key.trim().eq_ignore_ascii_case(name) && !value.trim().is_empty())
        .map(|(_, value)| value.trim().to_string())
}

fn upsert_header_case_insensitive(
    headers: &mut HashMap<String, String>,
    name: &str,
    value: impl Into<String>,
) {
    let existing = headers
        .keys()
        .filter(|key| key.trim().eq_ignore_ascii_case(name))
        .cloned()
        .collect::<Vec<_>>();
    for key in existing {
        headers.remove(&key);
    }
    headers.insert(name.to_string(), value.into());
}

fn extract_cookie_value(cookie_header: &str, cookie_name: &str) -> Option<String> {
    cookie_header
        .split(';')
        .filter_map(|entry| {
            let (name, value) = entry.trim().split_once('=')?;
            Some((name.trim(), value.trim()))
        })
        .find(|(name, value)| *name == cookie_name && !value.is_empty())
        .map(|(_, value)| value.to_string())
}

fn read_suno_cookie_header(input: &GatewayKeepaliveEnsureRequest) -> Option<String> {
    read_header_case_insensitive(&input.headers, "cookie").or_else(|| {
        input
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|value| value.contains('=') && !value.is_empty())
            .map(str::to_string)
    })
}

fn request_builder_with_headers(
    client: &Client,
    method: rquest::Method,
    url: &str,
    headers: &HashMap<String, String>,
) -> rquest::RequestBuilder {
    let mut builder = client.request(method, url);
    for (key, value) in headers {
        if is_internal_gateway_header(key) {
            continue;
        }
        if let (Ok(name), Ok(value)) = (
            HeaderName::try_from(key.as_str()),
            HeaderValue::from_str(value.as_str()),
        ) {
            builder = builder.header(name, value);
        }
    }
    builder
}

fn external_gateway_headers(headers: &HashMap<String, String>) -> HashMap<String, String> {
    headers
        .iter()
        .filter(|(name, _)| !is_internal_gateway_header(name))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

fn qwen_web_session_worker_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("qwen-web-session-worker.mjs")
}

fn qwen_web_effective_session_auth(payload: &ProviderAccountPayload) -> Option<SessionAuthConfig> {
    let mut session_auth = payload.session_auth.clone()?;
    if session_auth.expires_at.is_none() {
        session_auth.expires_at = payload.expires_at.clone();
    }
    Some(session_auth)
}

fn qwen_web_should_refresh(payload: &ProviderAccountPayload, force_refresh: bool) -> bool {
    if force_refresh {
        return true;
    }
    if payload.api_key.trim().is_empty() {
        return true;
    }

    match qwen_web_effective_session_auth(payload) {
        Some(session_auth) => session_auth.expires_within_secs(QWEN_WEB_REFRESH_BEFORE_SECS),
        None => false,
    }
}

fn sanitize_qwen_web_credential_file_name(value: &str) -> Option<String> {
    let normalized = value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| match ch {
            'a'..='z' | '0'..='9' | '.' | '_' | '-' => ch,
            _ => '-',
        })
        .collect::<String>();
    let collapsed = normalized
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if collapsed.is_empty() {
        None
    } else if collapsed.ends_with(".json") {
        Some(collapsed)
    } else {
        Some(format!("{collapsed}.json"))
    }
}

fn qwen_web_refresh_input(
    payload: &ProviderAccountPayload,
    model: &str,
) -> QwenWebSessionWorkerInput {
    let preferred_models = if model.trim().is_empty() {
        Vec::new()
    } else {
        vec![model.trim().to_string()]
    };
    let credential_file_name = payload
        .credential_id
        .as_deref()
        .and_then(sanitize_qwen_web_credential_file_name)
        .or_else(|| {
            payload
                .account_name
                .as_deref()
                .and_then(sanitize_qwen_web_credential_file_name)
        });

    QwenWebSessionWorkerInput {
        base_url: payload.base_url.trim().trim_end_matches('/').to_string(),
        preferred_models,
        write_credential_file: true,
        credential_file_name,
        credential_family_dir: Some(QWEN_WEB_CREDENTIAL_FAMILY_DIR.to_string()),
        credential_root_dir: std::env::var("NEURO_PROVIDER_CREDENTIAL_ROOT_DIR")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        browser_executable_path: std::env::var("QWEN_WEB_BROWSER_EXECUTABLE_PATH")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        user_data_dir: std::env::var("QWEN_WEB_BROWSER_USER_DATA_DIR")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        profile_directory: std::env::var("QWEN_WEB_BROWSER_PROFILE_DIR")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        auth_seed: qwen_web_signin_seed(payload.extra_body.as_ref()).map(|seed| {
            QwenWebSessionWorkerAuthSeed {
                email: seed.email,
                password: seed.password,
                password_sha256: seed.password_sha256,
            }
        }),
    }
}

async fn execute_chatgpt_web_session_worker_internal(
    payload: &ProviderAccountPayload,
    relay_request: Option<ChatGptWebSessionWorkerRelayRequest>,
) -> Result<ChatGptWebSessionWorkerExecution, GatewayError> {
    let provider = "chatgpt_web_reverse_compatible";
    if let Some(error) = request_time_local_browser_worker_blocking_error(
        RequestTimeBrowserPolicy::from_env(),
        provider,
    ) {
        return Err(error);
    }

    let script_path = chatgpt_web_session_worker_script_path();
    if !script_path.exists() {
        return Err(GatewayError::server_error(format!(
            "ChatGPT Web session worker is missing at {}.",
            script_path.display()
        ))
        .with_provider(provider)
        .with_code("chatgpt_web_session_worker_missing"));
    }

    let mut input = chatgpt_web_refresh_input(payload);
    input.relay_request = relay_request;
    let stdin_json = serde_json::to_vec(&input).map_err(|error| {
        GatewayError::server_error(format!(
            "Failed to serialize ChatGPT Web session worker input: {error}"
        ))
        .with_provider(provider)
        .with_code("chatgpt_web_session_worker_input_serialize_failed")
    })?;

    let node_bin = std::env::var("CHATGPT_WEB_BROWSER_NODE_BIN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var("PRODUCER_BROWSER_NODE_BIN")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| "node".to_string());

    let mut command = Command::new(node_bin);
    command
        .arg(&script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(not(target_os = "windows"))]
    {
        // Linux/WSL gateway workers commonly run without a usable X server even
        // when DISPLAY is inherited. Force headless by default so the browser
        // relay path does not die before it can reuse the existing session.
        if std::env::var_os("CHATGPT_WEB_SESSION_WORKER_HEADLESS").is_none() {
            command.env("CHATGPT_WEB_SESSION_WORKER_HEADLESS", "true");
        }
    }
    let mut child = command.spawn().map_err(|error| {
        GatewayError::server_error(format!(
            "Failed to launch ChatGPT Web session worker at {}: {error}",
            script_path.display()
        ))
        .with_provider(provider)
        .with_code("chatgpt_web_session_worker_spawn_failed")
    })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&stdin_json).await.map_err(|error| {
            GatewayError::server_error(format!(
                "Failed to write ChatGPT Web session worker input: {error}"
            ))
            .with_provider(provider)
            .with_code("chatgpt_web_session_worker_stdin_failed")
        })?;
    }

    let timeout = std::time::Duration::from_secs(CHATGPT_WEB_REFRESH_TIMEOUT_SECS);
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| {
            GatewayError::server_error(
                "ChatGPT Web session worker timed out while refreshing browser session material.",
            )
            .with_provider(provider)
            .with_code("chatgpt_web_session_worker_timeout")
        })?
        .map_err(|error| {
            GatewayError::server_error(format!(
                "ChatGPT Web session worker failed before producing output: {error}"
            ))
            .with_provider(provider)
            .with_code("chatgpt_web_session_worker_wait_failed")
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let last_json_line = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .trim()
        .to_string();
    if last_json_line.is_empty() {
        return Err(GatewayError::server_error(format!(
            "ChatGPT Web session worker produced no JSON output. stderr: {}",
            if stderr.is_empty() {
                "<empty>"
            } else {
                stderr.as_str()
            }
        ))
        .with_provider(provider)
        .with_code("chatgpt_web_session_worker_empty_output"));
    }

    let worker_output: ChatGptWebSessionWorkerOutput =
        serde_json::from_str(&last_json_line).map_err(|error| {
            GatewayError::server_error(format!(
                "Failed to parse ChatGPT Web session worker output: {error}. stdout: {last_json_line}"
            ))
            .with_provider(provider)
            .with_code("chatgpt_web_session_worker_output_parse_failed")
        })?;

    if !worker_output.ok {
        let message = worker_output
            .error
            .as_ref()
            .and_then(|error| error.message.as_deref())
            .map(str::to_string)
            .or_else(|| (!stderr.is_empty()).then_some(stderr.clone()))
            .unwrap_or_else(|| {
                "ChatGPT Web session worker returned ok=false while refreshing an existing session."
                    .to_string()
            });
        let mut error = GatewayError::server_error(message)
            .with_provider(provider)
            .with_code(
                worker_output
                    .error
                    .as_ref()
                    .and_then(|error| error.code.clone())
                    .unwrap_or_else(|| "chatgpt_web_session_worker_failed".to_string()),
            );
        if let Some(status) = worker_output.error.as_ref().and_then(|error| error.status) {
            error.http_status = Some(status);
        }
        return Err(error);
    }

    let cookie_header = worker_output
        .cookie_header
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let api_key = worker_output
        .auth_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| cookie_header.as_ref().map(|_| String::new()))
        .or_else(|| {
            let api_key = payload.api_key.trim();
            (!api_key.is_empty()).then_some(api_key.to_string())
        })
        .ok_or_else(|| {
            GatewayError::server_error(
                "ChatGPT Web session worker succeeded without returning any usable auth token, and the existing provider payload also had no apiKey to reuse.",
            )
            .with_provider(provider)
            .with_code("chatgpt_web_session_worker_missing_auth_token")
        })?;

    if let Some(path) = worker_output.credential_file.as_deref() {
        debug!(
            credential_file = %path,
            "ChatGPT Web session worker wrote refreshed credential file"
        );
    }

    Ok(ChatGptWebSessionWorkerExecution {
        refreshed: ChatGptWebRefreshedRuntime {
            api_key,
            expires_at: worker_output
                .expires_at
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| decode_jwt_expiry_iso(Some(&payload.api_key))),
            refresh_token: None,
            id_token: None,
            cookie_header,
            device_id: worker_output
                .device_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            session_id: worker_output
                .session_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            client_version: worker_output
                .client_version
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            client_build_number: worker_output
                .client_build_number
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            user_agent: worker_output
                .user_agent
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            language: worker_output
                .language
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            language_code: worker_output
                .language_code
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            timezone: worker_output
                .timezone
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            chatgpt_pow_sources: worker_output
                .chatgpt_pow_sources
                .filter(|items| !items.is_empty()),
            chatgpt_pow_data_build: worker_output
                .chatgpt_pow_data_build
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            account_name: worker_output
                .account_name
                .as_deref()
                .or_else(|| {
                    worker_output
                        .auth_probe
                        .as_ref()
                        .and_then(|probe| probe.email.as_deref())
                })
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            credential_material_key: worker_output
                .credential_material_key
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        },
        relay_response: worker_output.relay_response,
    })
}

async fn execute_chatgpt_web_session_worker(
    payload: &ProviderAccountPayload,
) -> Result<ChatGptWebRefreshedRuntime, GatewayError> {
    Ok(execute_chatgpt_web_session_worker_internal(payload, None)
        .await?
        .refreshed)
}

fn merge_chatgpt_web_runtime_headers(
    current_headers: &HashMap<String, String>,
    cookie_header: Option<&str>,
    user_agent: Option<&str>,
) -> HashMap<String, String> {
    let mut headers = external_gateway_headers(current_headers);
    if let Some(cookie_header) = cookie_header {
        upsert_header_case_insensitive(&mut headers, "Cookie", cookie_header.to_string());
    }
    if let Some(user_agent) = user_agent {
        upsert_header_case_insensitive(&mut headers, "User-Agent", user_agent.to_string());
    }
    headers
}

fn apply_chatgpt_web_runtime_refresh(
    payload: &ProviderAccountPayload,
    refreshed: &ChatGptWebRefreshedRuntime,
) -> ProviderAccountPayload {
    let mut effective = payload.clone();
    effective.api_key = refreshed.api_key.clone();
    effective.expires_at = refreshed.expires_at.clone();
    effective.headers = merge_chatgpt_web_runtime_headers(
        &effective.headers,
        refreshed.cookie_header.as_deref(),
        refreshed.user_agent.as_deref(),
    );
    if let Some(session_auth) = effective.session_auth.as_mut() {
        session_auth.expires_at = refreshed.expires_at.clone();
    }
    let extra_body = effective.extra_body.get_or_insert_with(HashMap::new);
    if let Some(device_id) = refreshed.device_id.as_ref() {
        extra_body.insert("deviceId".to_string(), Value::String(device_id.clone()));
    }
    if let Some(session_id) = refreshed.session_id.as_ref() {
        extra_body.insert("sessionId".to_string(), Value::String(session_id.clone()));
    }
    if let Some(client_version) = refreshed.client_version.as_ref() {
        extra_body.insert(
            "clientVersion".to_string(),
            Value::String(client_version.clone()),
        );
    }
    if let Some(client_build_number) = refreshed.client_build_number.as_ref() {
        extra_body.insert(
            "clientBuildNumber".to_string(),
            Value::String(client_build_number.clone()),
        );
    }
    if let Some(user_agent) = refreshed.user_agent.as_ref() {
        extra_body.insert("userAgent".to_string(), Value::String(user_agent.clone()));
    }
    if let Some(language) = refreshed.language.as_ref() {
        extra_body.insert("language".to_string(), Value::String(language.clone()));
    }
    if let Some(language_code) = refreshed.language_code.as_ref() {
        extra_body.insert(
            "languageCode".to_string(),
            Value::String(language_code.clone()),
        );
    }
    if let Some(timezone) = refreshed.timezone.as_ref() {
        extra_body.insert("timezone".to_string(), Value::String(timezone.clone()));
    }
    if let Some(pow_sources) = refreshed.chatgpt_pow_sources.as_ref() {
        extra_body.insert(
            "chatgptPowSources".to_string(),
            Value::Array(
                pow_sources
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect::<Vec<_>>(),
            ),
        );
    }
    if let Some(pow_data_build) = refreshed.chatgpt_pow_data_build.as_ref() {
        extra_body.insert(
            "chatgptPowDataBuild".to_string(),
            Value::String(pow_data_build.clone()),
        );
    }
    if let Some(refresh_token) = refreshed.refresh_token.as_ref() {
        extra_body.insert(
            "refreshToken".to_string(),
            Value::String(refresh_token.clone()),
        );
        extra_body.insert(
            "refreshStrategy".to_string(),
            Value::String("oauth_token".to_string()),
        );
        extra_body.insert(
            "lastTokenRefreshAt".to_string(),
            Value::String(db::format_timestamp(OffsetDateTime::now_utc())),
        );
    }
    if let Some(id_token) = refreshed.id_token.as_ref() {
        extra_body.insert("idToken".to_string(), Value::String(id_token.clone()));
    }
    if let Some(expires_at) = refreshed.expires_at.as_ref() {
        extra_body.insert(
            "accessTokenExpiresAt".to_string(),
            Value::String(expires_at.clone()),
        );
    }
    if let Some(account_name) = refreshed.account_name.as_ref() {
        effective.account_name = Some(account_name.clone());
    }
    effective
}

async fn persist_chatgpt_web_runtime_refresh(
    redis_pool: &Pool,
    pg_pool: Option<&PgPool>,
    payload: &ProviderAccountPayload,
    refreshed: &ChatGptWebRefreshedRuntime,
) {
    let merged_headers = merge_chatgpt_web_runtime_headers(
        &payload.headers,
        refreshed.cookie_header.as_deref(),
        refreshed.user_agent.as_deref(),
    );
    let mut extra_body_patch = payload.extra_body.clone().unwrap_or_default();
    if let Some(device_id) = refreshed.device_id.as_ref() {
        extra_body_patch.insert("deviceId".to_string(), Value::String(device_id.clone()));
    }
    if let Some(session_id) = refreshed.session_id.as_ref() {
        extra_body_patch.insert("sessionId".to_string(), Value::String(session_id.clone()));
    }
    if let Some(client_version) = refreshed.client_version.as_ref() {
        extra_body_patch.insert(
            "clientVersion".to_string(),
            Value::String(client_version.clone()),
        );
    }
    if let Some(client_build_number) = refreshed.client_build_number.as_ref() {
        extra_body_patch.insert(
            "clientBuildNumber".to_string(),
            Value::String(client_build_number.clone()),
        );
    }
    if let Some(user_agent) = refreshed.user_agent.as_ref() {
        extra_body_patch.insert("userAgent".to_string(), Value::String(user_agent.clone()));
    }
    if let Some(language) = refreshed.language.as_ref() {
        extra_body_patch.insert("language".to_string(), Value::String(language.clone()));
    }
    if let Some(language_code) = refreshed.language_code.as_ref() {
        extra_body_patch.insert(
            "languageCode".to_string(),
            Value::String(language_code.clone()),
        );
    }
    if let Some(timezone) = refreshed.timezone.as_ref() {
        extra_body_patch.insert("timezone".to_string(), Value::String(timezone.clone()));
    }
    if let Some(pow_sources) = refreshed.chatgpt_pow_sources.as_ref() {
        extra_body_patch.insert(
            "chatgptPowSources".to_string(),
            Value::Array(
                pow_sources
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect::<Vec<_>>(),
            ),
        );
    }
    if let Some(pow_data_build) = refreshed.chatgpt_pow_data_build.as_ref() {
        extra_body_patch.insert(
            "chatgptPowDataBuild".to_string(),
            Value::String(pow_data_build.clone()),
        );
    }
    if let Some(refresh_token) = refreshed.refresh_token.as_ref() {
        extra_body_patch.insert(
            "refreshToken".to_string(),
            Value::String(refresh_token.clone()),
        );
        extra_body_patch.insert(
            "refreshStrategy".to_string(),
            Value::String("oauth_token".to_string()),
        );
        extra_body_patch.insert(
            "lastTokenRefreshAt".to_string(),
            Value::String(db::format_timestamp(OffsetDateTime::now_utc())),
        );
    }
    if let Some(id_token) = refreshed.id_token.as_ref() {
        extra_body_patch.insert("idToken".to_string(), Value::String(id_token.clone()));
    }
    if let Some(expires_at) = refreshed.expires_at.as_ref() {
        extra_body_patch.insert(
            "accessTokenExpiresAt".to_string(),
            Value::String(expires_at.clone()),
        );
    }

    if let Some(credential_id) = payload.credential_id.as_deref() {
        let _ = credential_cache::write_back_runtime_material(
            redis_pool,
            credential_id,
            Some(refreshed.api_key.as_str()),
            Some(&merged_headers),
            Some(&extra_body_patch),
            payload.session_auth.as_ref(),
            payload.keepalive.as_ref(),
            refreshed.expires_at.as_deref(),
            payload.runtime_state_object_key.as_deref(),
        )
        .await;
    }

    if let (Some(pg_pool), Some(credential_id)) = (pg_pool, payload.credential_id.as_deref()) {
        match db::get_provider_credential(pg_pool, credential_id).await {
            Ok(Some(existing)) => {
                let mut updated_payload = existing.payload.clone();
                if let Some(map) = updated_payload.as_object_mut() {
                    map.insert(
                        "apiKey".to_string(),
                        Value::String(refreshed.api_key.clone()),
                    );
                    if let Some(expires_at) = refreshed.expires_at.as_ref() {
                        map.insert("expiresAt".to_string(), Value::String(expires_at.clone()));
                    }
                    if let Some(refresh_token) = refreshed.refresh_token.as_ref() {
                        map.insert(
                            "refreshToken".to_string(),
                            Value::String(refresh_token.clone()),
                        );
                    }
                    if let Some(id_token) = refreshed.id_token.as_ref() {
                        map.insert("idToken".to_string(), Value::String(id_token.clone()));
                    }
                    let headers_value = map
                        .entry("headers".to_string())
                        .or_insert_with(|| Value::Object(serde_json::Map::new()));
                    if let Some(headers_map) = headers_value.as_object_mut() {
                        for (key, value) in &merged_headers {
                            headers_map.insert(key.clone(), Value::String(value.clone()));
                        }
                    }
                    let extra_body_value = map
                        .entry("extraBody".to_string())
                        .or_insert_with(|| Value::Object(serde_json::Map::new()));
                    if let Some(extra_body_map) = extra_body_value.as_object_mut() {
                        for (key, value) in &extra_body_patch {
                            extra_body_map.insert(key.clone(), value.clone());
                        }
                    }
                    if let Some(account_name) = refreshed.account_name.as_ref() {
                        map.insert(
                            "accountName".to_string(),
                            Value::String(account_name.clone()),
                        );
                    }
                    if let Some(credential_material_key) =
                        refreshed.credential_material_key.as_ref()
                    {
                        map.insert(
                            "credentialMaterialKey".to_string(),
                            Value::String(credential_material_key.clone()),
                        );
                    }
                }

                let update_input = db::UpsertProviderCredentialInput {
                    provider_account_id: existing.provider_account_id.clone(),
                    label: existing.label.clone(),
                    status: Some(existing.status.clone()),
                    payload: updated_payload,
                    source_kind: Some(existing.source_kind.clone()),
                    source_path: existing.source_path.clone(),
                    source_hash: existing.source_hash.clone(),
                    sync_mode: Some(existing.sync_mode.clone()),
                    sync_state: Some(existing.sync_state.clone()),
                    sync_error: existing.sync_error.clone(),
                };
                if let Err(error) =
                    db::update_provider_credential(pg_pool, credential_id, update_input).await
                {
                    warn!(
                        provider_credential_id = %credential_id,
                        error = %error,
                        "failed to persist refreshed ChatGPT Web credential payload to Postgres"
                    );
                }
            }
            Ok(None) => {}
            Err(error) => {
                warn!(
                    provider_credential_id = %credential_id,
                    error = %error,
                    "failed to load ChatGPT Web credential before persisting refreshed runtime material"
                );
            }
        }
    }
}

async fn ensure_chatgpt_web_payload_ready(
    redis_pool: &Pool,
    pg_pool: Option<&PgPool>,
    payload: &ProviderAccountPayload,
    force_refresh: bool,
) -> Result<ProviderAccountPayload, GatewayError> {
    if chatgpt_web_should_oauth_refresh(payload, force_refresh) {
        match execute_chatgpt_web_oauth_refresh(&Client::new(), payload).await {
            Ok(refreshed) => {
                persist_chatgpt_web_runtime_refresh(redis_pool, pg_pool, payload, &refreshed).await;
                return Ok(apply_chatgpt_web_runtime_refresh(payload, &refreshed));
            }
            Err(error) => {
                if force_refresh || chatgpt_web_should_refresh(payload, force_refresh) {
                    if let Some(policy_error) = request_time_local_browser_worker_blocking_error(
                        RequestTimeBrowserPolicy::from_env(),
                        "chatgpt_web_reverse_compatible",
                    ) {
                        return Err(policy_error);
                    }
                    if !chatgpt_web_request_time_browser_fallback_allowed(payload) {
                        return Err(error);
                    }
                    warn!(
                        error = %error,
                        "ChatGPT Web OAuth refresh failed; falling back to browser materialization"
                    );
                } else {
                    warn!(
                        error = %error,
                        "ChatGPT Web OAuth refresh failed before hard expiry; keeping current payload"
                    );
                    return Ok(payload.clone());
                }
            }
        }
    }
    if !chatgpt_web_should_refresh(payload, force_refresh) {
        return Ok(payload.clone());
    }

    let refreshed = execute_chatgpt_web_session_worker(payload).await?;
    persist_chatgpt_web_runtime_refresh(redis_pool, pg_pool, payload, &refreshed).await;
    Ok(apply_chatgpt_web_runtime_refresh(payload, &refreshed))
}

pub async fn refresh_chatgpt_web_oauth_payload_if_due(
    redis_pool: &Pool,
    pg_pool: Option<&PgPool>,
    http: &Client,
    payload: &ProviderAccountPayload,
    refresh_before_secs: u64,
    force_refresh: bool,
) -> Result<Option<ProviderAccountPayload>, GatewayError> {
    if !chatgpt_web_oauth_refresh_due(payload, refresh_before_secs, force_refresh) {
        return Ok(None);
    }

    let refreshed = execute_chatgpt_web_oauth_refresh(http, payload).await?;
    persist_chatgpt_web_runtime_refresh(redis_pool, pg_pool, payload, &refreshed).await;
    Ok(Some(apply_chatgpt_web_runtime_refresh(payload, &refreshed)))
}

async fn execute_qwen_web_session_worker(
    payload: &ProviderAccountPayload,
    model: &str,
) -> Result<QwenWebRefreshedRuntime, GatewayError> {
    let provider = "qwen_web_compatible";
    if let Some(error) = request_time_local_browser_worker_blocking_error(
        RequestTimeBrowserPolicy::from_env(),
        provider,
    ) {
        return Err(error);
    }

    let script_path = qwen_web_session_worker_script_path();
    if !script_path.exists() {
        return Err(GatewayError::server_error(format!(
            "Qwen Web session worker is missing at {}.",
            script_path.display()
        ))
        .with_provider(provider)
        .with_code("qwen_web_session_worker_missing"));
    }

    let input = qwen_web_refresh_input(payload, model);
    let stdin_json = serde_json::to_vec(&input).map_err(|error| {
        GatewayError::server_error(format!(
            "Failed to serialize Qwen Web session worker input: {error}"
        ))
        .with_provider(provider)
        .with_code("qwen_web_session_worker_input_serialize_failed")
    })?;

    let node_bin = std::env::var("QWEN_WEB_BROWSER_NODE_BIN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var("PRODUCER_BROWSER_NODE_BIN")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| "node".to_string());

    let mut child = Command::new(node_bin)
        .arg(&script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            GatewayError::server_error(format!(
                "Failed to launch Qwen Web session worker at {}: {error}",
                script_path.display()
            ))
            .with_provider(provider)
            .with_code("qwen_web_session_worker_spawn_failed")
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&stdin_json).await.map_err(|error| {
            GatewayError::server_error(format!(
                "Failed to write Qwen Web session worker input: {error}"
            ))
            .with_provider(provider)
            .with_code("qwen_web_session_worker_stdin_failed")
        })?;
    }

    let timeout = std::time::Duration::from_secs(QWEN_WEB_REFRESH_TIMEOUT_SECS);
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| {
            GatewayError::server_error(
                "Qwen Web session worker timed out while refreshing browser session material.",
            )
            .with_provider(provider)
            .with_code("qwen_web_session_worker_timeout")
        })?
        .map_err(|error| {
            GatewayError::server_error(format!(
                "Qwen Web session worker failed before producing output: {error}"
            ))
            .with_provider(provider)
            .with_code("qwen_web_session_worker_wait_failed")
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let last_json_line = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .trim()
        .to_string();
    if last_json_line.is_empty() {
        return Err(GatewayError::server_error(format!(
            "Qwen Web session worker produced no JSON output. stderr: {}",
            if stderr.is_empty() {
                "<empty>"
            } else {
                stderr.as_str()
            }
        ))
        .with_provider(provider)
        .with_code("qwen_web_session_worker_empty_output"));
    }

    let worker_output: QwenWebSessionWorkerOutput =
        serde_json::from_str(&last_json_line).map_err(|error| {
            GatewayError::server_error(format!(
                "Failed to parse Qwen Web session worker output: {error}. stdout: {last_json_line}"
            ))
            .with_provider(provider)
            .with_code("qwen_web_session_worker_output_parse_failed")
        })?;

    if !worker_output.ok {
        let message = worker_output
            .error
            .as_ref()
            .and_then(|error| error.message.as_deref())
            .map(str::to_string)
            .or_else(|| (!stderr.is_empty()).then_some(stderr.clone()))
            .unwrap_or_else(|| {
                "Qwen Web session worker returned ok=false while refreshing an existing session."
                    .to_string()
            });
        let mut error = GatewayError::server_error(message)
            .with_provider(provider)
            .with_code(
                worker_output
                    .error
                    .as_ref()
                    .and_then(|error| error.code.clone())
                    .unwrap_or_else(|| "qwen_web_session_worker_failed".to_string()),
            );
        if let Some(status) = worker_output.error.as_ref().and_then(|error| error.status) {
            error.http_status = Some(status);
        }
        return Err(error);
    }

    let api_key = worker_output
        .auth_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::server_error(
                "Qwen Web session worker succeeded without returning an auth token. Qwen Web refresh only works against an existing signed-in browser/session source; the gateway does not register or sign in accounts here.",
            )
            .with_provider(provider)
            .with_code("qwen_web_session_worker_missing_auth_token")
        })?
        .to_string();

    let account_name = worker_output
        .auth_probe
        .as_ref()
        .and_then(|probe| probe.email.as_deref().or(probe.user_id.as_deref()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let credential_material_key = worker_output
        .auth_probe
        .as_ref()
        .and_then(|probe| probe.user_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("qwen-web-user:{value}"));

    if let Some(path) = worker_output.credential_file.as_deref() {
        debug!(credential_file = %path, "Qwen Web session worker wrote refreshed credential file");
    }

    Ok(QwenWebRefreshedRuntime {
        api_key,
        expires_at: worker_output
            .expires_at
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        cookie_header: worker_output
            .cookie_header
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        account_name,
        selected_display_model: worker_output
            .selected_display_model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        credential_material_key,
    })
}

async fn execute_qwen_web_http_signin_refresh(
    payload: &ProviderAccountPayload,
) -> Result<Option<QwenWebRefreshedRuntime>, GatewayError> {
    let Some(seed) = qwen_web_signin_seed(payload.extra_body.as_ref()) else {
        return Ok(None);
    };

    let provider = "qwen_web_compatible";
    let base_url = payload.base_url.trim_end_matches('/').to_string();
    let signin_url = format!("{base_url}/api/v1/auths/signin");
    let auths_url = format!("{base_url}/api/v1/auths/");
    let client = Client::new();
    let signin_headers = qwen_web_signin_headers(payload);
    let password_attempts = qwen_web_signin_password_attempts(&seed);
    if password_attempts.is_empty() {
        return Ok(None);
    }

    for password_value in password_attempts {
        let signin_response = match request_builder_with_headers(
            &client,
            rquest::Method::POST,
            &signin_url,
            &signin_headers,
        )
        .json(&json!({
            "email": seed.email,
            "password": password_value,
        }))
        .send()
        .await
        {
            Ok(response) => response,
            Err(error) => {
                warn!(
                    provider = provider,
                    email = %seed.email,
                    error = %error,
                    "Qwen Web HTTP signin refresh request failed"
                );
                continue;
            }
        };

        let signin_status = signin_response.status().as_u16();
        let signin_content_type = signin_response
            .headers()
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let set_cookie_header = collect_set_cookie_header(signin_response.headers());
        let signin_body = match signin_response.text().await {
            Ok(body) => body,
            Err(error) => {
                warn!(
                    provider = provider,
                    email = %seed.email,
                    error = %error,
                    "Qwen Web HTTP signin refresh response body could not be read"
                );
                continue;
            }
        };

        if crate::protocol::qwen_web::response_indicates_browser_challenge(
            signin_status,
            signin_content_type.as_deref(),
            &signin_body,
        ) {
            warn!(
                provider = provider,
                email = %seed.email,
                status = signin_status,
                "Qwen Web HTTP signin hit an upstream challenge; browser-backed refresh is required"
            );
            return Ok(None);
        }

        if !(200..300).contains(&signin_status) {
            continue;
        }

        let signin_json: Value = match serde_json::from_str(&signin_body) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Some(auth_token) = signin_json
            .get("token")
            .or_else(|| signin_json.get("access_token"))
            .or_else(|| signin_json.get("data").and_then(|value| value.get("token")))
            .or_else(|| {
                signin_json
                    .get("data")
                    .and_then(|value| value.get("access_token"))
            })
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
        else {
            continue;
        };

        let mut auth_headers = qwen_web_signin_headers(payload);
        auth_headers.insert("authorization".to_string(), format!("Bearer {auth_token}"));
        let auth_response = match request_builder_with_headers(
            &client,
            rquest::Method::GET,
            &auths_url,
            &auth_headers,
        )
        .send()
        .await
        {
            Ok(response) => response,
            Err(error) => {
                warn!(
                    provider = provider,
                    email = %seed.email,
                    error = %error,
                    "Qwen Web auth probe after HTTP signin failed"
                );
                continue;
            }
        };
        let auth_status = auth_response.status().as_u16();
        let auth_content_type = auth_response
            .headers()
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let auth_body = match auth_response.text().await {
            Ok(body) => body,
            Err(error) => {
                warn!(
                    provider = provider,
                    email = %seed.email,
                    error = %error,
                    "Qwen Web auth probe body after HTTP signin could not be read"
                );
                continue;
            }
        };

        if crate::protocol::qwen_web::response_indicates_browser_challenge(
            auth_status,
            auth_content_type.as_deref(),
            &auth_body,
        ) || crate::protocol::qwen_web::response_indicates_session_invalid(
            auth_status,
            auth_content_type.as_deref(),
            &auth_body,
        ) || !(200..300).contains(&auth_status)
        {
            continue;
        }

        let auth_json: Value = serde_json::from_str(&auth_body).unwrap_or(Value::Null);
        let expires_at = read_json_field_string(&auth_json, "expires_at")
            .or_else(|| {
                auth_json
                    .get("data")
                    .and_then(|value| read_json_field_string(value, "expires_at"))
            })
            .or_else(|| decode_jwt_expiry_iso(Some(&auth_token)));
        let account_name = read_json_field_string(&auth_json, "email")
            .or_else(|| read_json_field_string(&auth_json, "id"))
            .or_else(|| {
                auth_json
                    .get("data")
                    .and_then(|value| read_json_field_string(value, "email"))
                    .or_else(|| {
                        auth_json
                            .get("data")
                            .and_then(|value| read_json_field_string(value, "id"))
                    })
            })
            .or_else(|| Some(seed.email.clone()));
        let credential_material_key = read_json_field_string(&auth_json, "id")
            .or_else(|| {
                auth_json
                    .get("data")
                    .and_then(|value| read_json_field_string(value, "id"))
            })
            .map(|value| format!("qwen-web-user:{value}"));

        return Ok(Some(QwenWebRefreshedRuntime {
            api_key: auth_token,
            expires_at,
            cookie_header: set_cookie_header,
            account_name,
            selected_display_model: None,
            credential_material_key,
        }));
    }

    Ok(None)
}

fn merge_qwen_web_runtime_headers(
    current_headers: &HashMap<String, String>,
    cookie_header: Option<&str>,
) -> HashMap<String, String> {
    let mut headers = external_gateway_headers(current_headers);
    if let Some(cookie_header) = cookie_header {
        headers.insert("Cookie".to_string(), cookie_header.to_string());
    }
    headers
}

fn apply_qwen_web_runtime_refresh(
    payload: &ProviderAccountPayload,
    refreshed: &QwenWebRefreshedRuntime,
) -> ProviderAccountPayload {
    let mut effective = payload.clone();
    effective.api_key = refreshed.api_key.clone();
    effective.expires_at = refreshed.expires_at.clone();
    effective.headers =
        merge_qwen_web_runtime_headers(&effective.headers, refreshed.cookie_header.as_deref());
    if let Some(session_auth) = effective.session_auth.as_mut() {
        session_auth.expires_at = refreshed.expires_at.clone();
    }
    if let Some(account_name) = refreshed.account_name.as_ref() {
        effective.account_name = Some(account_name.clone());
    }
    effective
}

async fn persist_qwen_web_runtime_refresh(
    redis_pool: &Pool,
    pg_pool: Option<&PgPool>,
    payload: &ProviderAccountPayload,
    refreshed: &QwenWebRefreshedRuntime,
) {
    let merged_headers =
        merge_qwen_web_runtime_headers(&payload.headers, refreshed.cookie_header.as_deref());

    if let Some(credential_id) = payload.credential_id.as_deref() {
        let _ = credential_cache::write_back_runtime_material(
            redis_pool,
            credential_id,
            Some(refreshed.api_key.as_str()),
            Some(&merged_headers),
            None,
            payload.session_auth.as_ref(),
            payload.keepalive.as_ref(),
            refreshed.expires_at.as_deref(),
            payload.runtime_state_object_key.as_deref(),
        )
        .await;
    }

    if let (Some(pg_pool), Some(credential_id)) = (pg_pool, payload.credential_id.as_deref()) {
        match db::get_provider_credential(pg_pool, credential_id).await {
            Ok(Some(existing)) => {
                let mut updated_payload = existing.payload.clone();
                if let Some(map) = updated_payload.as_object_mut() {
                    map.insert(
                        "apiKey".to_string(),
                        Value::String(refreshed.api_key.clone()),
                    );
                    if let Some(expires_at) = refreshed.expires_at.as_ref() {
                        map.insert("expiresAt".to_string(), Value::String(expires_at.clone()));
                    }
                    let headers_value = map
                        .entry("headers".to_string())
                        .or_insert_with(|| Value::Object(serde_json::Map::new()));
                    if let Some(headers_map) = headers_value.as_object_mut() {
                        for (key, value) in &merged_headers {
                            headers_map.insert(key.clone(), Value::String(value.clone()));
                        }
                    }
                    if let Some(account_name) = refreshed.account_name.as_ref() {
                        map.insert(
                            "accountName".to_string(),
                            Value::String(account_name.clone()),
                        );
                    }
                    if let Some(selected_display_model) = refreshed.selected_display_model.as_ref()
                    {
                        map.insert(
                            "selectedDisplayModel".to_string(),
                            Value::String(selected_display_model.clone()),
                        );
                    }
                    if let Some(credential_material_key) =
                        refreshed.credential_material_key.as_ref()
                    {
                        map.insert(
                            "credentialMaterialKey".to_string(),
                            Value::String(credential_material_key.clone()),
                        );
                    }
                }

                let update_input = db::UpsertProviderCredentialInput {
                    provider_account_id: existing.provider_account_id.clone(),
                    label: existing.label.clone(),
                    status: Some(existing.status.clone()),
                    payload: updated_payload,
                    source_kind: Some(existing.source_kind.clone()),
                    source_path: existing.source_path.clone(),
                    source_hash: existing.source_hash.clone(),
                    sync_mode: Some(existing.sync_mode.clone()),
                    sync_state: Some(existing.sync_state.clone()),
                    sync_error: existing.sync_error.clone(),
                };
                if let Err(error) =
                    db::update_provider_credential(pg_pool, credential_id, update_input).await
                {
                    warn!(
                        provider_credential_id = %credential_id,
                        error = %error,
                        "failed to persist refreshed Qwen Web credential payload to Postgres"
                    );
                }
            }
            Ok(None) => {}
            Err(error) => {
                warn!(
                    provider_credential_id = %credential_id,
                    error = %error,
                    "failed to load Qwen Web credential before persisting refreshed runtime material"
                );
            }
        }
    }
}

async fn ensure_qwen_web_payload_ready(
    redis_pool: &Pool,
    pg_pool: Option<&PgPool>,
    payload: &ProviderAccountPayload,
    model: &str,
    force_refresh: bool,
) -> Result<ProviderAccountPayload, GatewayError> {
    if !qwen_web_should_refresh(payload, force_refresh) {
        return Ok(payload.clone());
    }

    let refreshed = match execute_qwen_web_http_signin_refresh(payload).await {
        Ok(Some(refreshed)) => refreshed,
        Ok(None) => execute_qwen_web_session_worker(payload, model).await?,
        Err(error) => {
            warn!(
                provider = "qwen_web_compatible",
                provider_account_id = %payload.base_url,
                error = %error,
                "Qwen Web HTTP signin refresh failed; falling back to browser-backed refresh"
            );
            execute_qwen_web_session_worker(payload, model).await?
        }
    };
    persist_qwen_web_runtime_refresh(redis_pool, pg_pool, payload, &refreshed).await;
    Ok(apply_qwen_web_runtime_refresh(payload, &refreshed))
}

fn provider_payload_from_keepalive_request(
    input: &GatewayKeepaliveEnsureRequest,
    session_auth: Option<SessionAuthConfig>,
    effective_expires_at: Option<String>,
) -> ProviderAccountPayload {
    ProviderAccountPayload {
        adapter: input.adapter.clone(),
        base_url: input.base_url.clone(),
        api_key: input.api_key.clone().unwrap_or_default(),
        credential_id: input.credential_id.clone(),
        expires_at: effective_expires_at,
        runtime_state_object_key: input.runtime_state_object_key.clone(),
        account_name: input.account_name.clone(),
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: Some(input.model.clone()),
        headers: input.headers.clone(),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        auth_token: None,
        responses_path: None,
        chat_completions_path: None,
        completions_path: None,
        embeddings_path: None,
        audio_transcriptions_path: None,
        audio_speech_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        extra_body: input.extra_body.clone(),
        session_auth,
        keepalive: None,
    }
}

fn build_gemini_canvas_runtime_material_response(
    input: &GatewayKeepaliveEnsureRequest,
    effective_session_auth: Option<SessionAuthConfig>,
    effective_expires_at: Option<String>,
    requested_runtime_state_object_key: Option<String>,
) -> GatewayKeepaliveEnsureResponse {
    let Some(runtime_state_object_key) = requested_runtime_state_object_key else {
        return GatewayKeepaliveEnsureResponse {
            ready: false,
            message: Some(
                "Gemini Canvas credentials require runtimeStateObjectKey browser-state material."
                    .to_string(),
            ),
            api_key: input.api_key.clone(),
            headers: Some(input.headers.clone()),
            extra_body: input.extra_body.clone(),
            session_auth: effective_session_auth,
            keepalive: None,
            expires_at: effective_expires_at,
            runtime_state_object_key: None,
            upstream_session_id: None,
        };
    };

    if let Some(expires_at) = effective_expires_at.as_deref().and_then(parse_rfc3339) {
        if expires_at <= OffsetDateTime::now_utc() {
            return GatewayKeepaliveEnsureResponse {
                ready: false,
                message: Some(
                    "Gemini Canvas browser-state credential has expired and must be rotated by the external account worker."
                        .to_string(),
                ),
                api_key: input.api_key.clone(),
                headers: Some(input.headers.clone()),
                extra_body: input.extra_body.clone(),
                session_auth: effective_session_auth,
                keepalive: None,
                expires_at: effective_expires_at,
                runtime_state_object_key: Some(runtime_state_object_key),
                upstream_session_id: None,
            };
        }
    }

    GatewayKeepaliveEnsureResponse {
        ready: true,
        message: Some("Gemini Canvas runtime material is ready.".to_string()),
        api_key: input.api_key.clone(),
        headers: Some(input.headers.clone()),
        extra_body: input.extra_body.clone(),
        session_auth: effective_session_auth,
        keepalive: None,
        expires_at: effective_expires_at,
        runtime_state_object_key: Some(runtime_state_object_key),
        upstream_session_id: None,
    }
}

/// Ensure session-backed payloads remain fresh before the real upstream call.
///
/// The keepalive steward is a separate service boundary. It may refresh session
/// material and return updated runtime auth state, but it does not replace the
/// gateway's request-sending hot path.
pub async fn ensure_payload_ready(
    redis_pool: &Pool,
    pg_pool: Option<&PgPool>,
    http: &Client,
    payload: &ProviderAccountPayload,
    project_id: Option<&str>,
    session_key: Option<&str>,
    previous_response_id: Option<&str>,
    provider_account_id: &str,
    model: &str,
) -> Result<ProviderAccountPayload, GatewayError> {
    implementation_lines::assert_adapter_compiled(
        payload.adapter.as_str(),
        "keepalive payload ensure requested",
    )?;

    if payload.adapter.trim() == "chatgpt_web_reverse_compatible" {
        return ensure_chatgpt_web_payload_ready(redis_pool, pg_pool, payload, false).await;
    }
    if payload.adapter.trim() == "qwen_web_compatible" {
        return ensure_qwen_web_payload_ready(redis_pool, pg_pool, payload, model, false).await;
    }

    let Some(keepalive) = payload.keepalive.as_ref() else {
        return Ok(payload.clone());
    };

    if !keepalive.should_ensure(payload.session_auth.as_ref()) {
        return Ok(payload.clone());
    }

    let request = GatewayKeepaliveEnsureRequest {
        project_id: project_id.map(str::to_string),
        session_key: session_key.map(str::to_string),
        previous_response_id: previous_response_id.map(str::to_string),
        credential_id: payload.credential_id.clone(),
        account_name: payload.account_name.clone(),
        provider_account_id: provider_account_id.to_string(),
        adapter: payload.adapter.clone(),
        base_url: payload.base_url.clone(),
        model: model.to_string(),
        api_key: Some(payload.api_key.clone()),
        headers: external_gateway_headers(&payload.headers),
        extra_body: payload.extra_body.clone(),
        session_auth: payload.session_auth.clone(),
        expires_at: payload.expires_at.clone().or_else(|| {
            payload
                .session_auth
                .as_ref()
                .and_then(|cfg| cfg.expires_at.clone())
        }),
        runtime_state_object_key: payload.runtime_state_object_key.clone(),
    };

    let mut call = http
        .post(keepalive.ensure_url())
        .header("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(keepalive.timeout_secs()))
        .json(&request);

    if let Some(token) = keepalive.auth_token.as_deref() {
        call = call.bearer_auth(token);
    }

    let response = call
        .send()
        .await
        .map_err(|e| classify_network_error(&e, Some("credential_keepalive")))?;

    let status = response.status().as_u16();
    if !response.status().is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("<unreadable keepalive body>"));
        return Err(classify_upstream_error(
            status,
            &body,
            Some("credential_keepalive"),
        ));
    }

    let ensured: GatewayKeepaliveEnsureResponse = response.json().await.map_err(|e| {
        GatewayError::server_error(format!("Keepalive service returned invalid JSON: {e}"))
            .with_provider("credential_keepalive")
            .with_code("credential_keepalive_invalid_json")
    })?;

    if !ensured.ready {
        return Err(
            GatewayError::server_error(ensured.message.unwrap_or_else(|| {
                "Credential keepalive steward could not ensure session readiness".to_string()
            }))
            .with_provider("credential_keepalive")
            .with_code("credential_keepalive_not_ready"),
        );
    }

    let mut effective = payload.clone();
    effective.headers = external_gateway_headers(&effective.headers);

    if let Some(api_key) = ensured.api_key.as_ref() {
        effective.api_key = api_key.clone();
    }

    if let Some(session_auth) = ensured.session_auth.as_ref() {
        effective.session_auth = Some(session_auth.clone());
    }

    if let Some(expiry) = ensured.expires_at.as_ref() {
        effective.expires_at = Some(expiry.clone());
        if let Some(session_auth) = effective.session_auth.as_mut() {
            session_auth.expires_at = Some(expiry.clone());
        }
    }

    if let Some(ref object_key) = ensured.runtime_state_object_key {
        effective.runtime_state_object_key = Some(object_key.clone());
    }

    if let Some(ref keepalive_cfg) = ensured.keepalive {
        effective.keepalive = Some(keepalive_cfg.clone());
    }

    if let Some(ref header_patch) = ensured.headers {
        for (k, v) in header_patch {
            if is_internal_gateway_header(k) {
                continue;
            }
            effective.headers.insert(k.clone(), v.clone());
        }
    }

    if let Some(ref body_patch) = ensured.extra_body {
        let extra_body = effective.extra_body.get_or_insert_with(HashMap::new);
        for (k, v) in body_patch {
            extra_body.insert(k.clone(), v.clone());
        }
    }

    if let Some(credential_id) = payload.credential_id.as_deref() {
        let api_key_write_back = if payload.adapter == "suno_compatible" {
            None
        } else {
            ensured.api_key.as_deref()
        };
        let _ = credential_cache::write_back_runtime_material(
            redis_pool,
            credential_id,
            api_key_write_back,
            ensured.headers.as_ref(),
            ensured.extra_body.as_ref(),
            effective.session_auth.as_ref(),
            effective.keepalive.as_ref(),
            ensured.expires_at.as_deref(),
            ensured.runtime_state_object_key.as_deref(),
        )
        .await;
    }

    Ok(effective)
}

pub async fn refresh_qwen_web_payload_after_challenge(
    redis_pool: &Pool,
    pg_pool: Option<&PgPool>,
    payload: &ProviderAccountPayload,
    model: &str,
) -> Result<ProviderAccountPayload, GatewayError> {
    ensure_qwen_web_payload_ready(redis_pool, pg_pool, payload, model, true).await
}

pub async fn refresh_chatgpt_web_payload_after_challenge(
    redis_pool: &Pool,
    pg_pool: Option<&PgPool>,
    payload: &ProviderAccountPayload,
) -> Result<ProviderAccountPayload, GatewayError> {
    ensure_chatgpt_web_payload_ready(redis_pool, pg_pool, payload, true).await
}

pub async fn execute_chatgpt_web_browser_relay(
    redis_pool: &Pool,
    pg_pool: Option<&PgPool>,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<ChatGptWebBrowserRelayResult, GatewayError> {
    let timezone = read_extra_body_string(payload.extra_body.as_ref(), &["timezone"])
        .unwrap_or_else(|| {
            crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_DEFAULT_TIMEZONE.to_string()
        });
    let relay_body = crate::protocol::chatgpt::web_reverse::pack_request(req, model, &timezone)?;
    let execution = execute_chatgpt_web_session_worker_internal(
        payload,
        Some(ChatGptWebSessionWorkerRelayRequest {
            stream,
            body: relay_body,
        }),
    )
    .await?;
    persist_chatgpt_web_runtime_refresh(redis_pool, pg_pool, payload, &execution.refreshed).await;
    let refreshed_payload = apply_chatgpt_web_runtime_refresh(payload, &execution.refreshed);
    let relay = execution.relay_response.ok_or_else(|| {
        GatewayError::service_unavailable(
            "ChatGPT Web browser relay worker completed without returning any relay response.",
        )
        .with_provider("chatgpt_web_reverse_compatible")
        .with_code("chatgpt_web_browser_relay_missing_response")
    })?;
    let status = relay.status.or(relay.requirements_status).unwrap_or(0);
    let content_type = relay
        .content_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            relay
                .requirements_content_type
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });
    let body_text = relay
        .body_text
        .as_deref()
        .or(relay.body_preview.as_deref())
        .or(relay.requirements_preview.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_default();
    if !(200..300).contains(&status) {
        return Err(
            crate::protocol::chatgpt::web_reverse::classify_chatgpt_web_http_error(
                status,
                content_type.as_deref(),
                &body_text,
            ),
        );
    }
    if body_text.trim().is_empty() {
        return Err(GatewayError::service_unavailable(
            "ChatGPT Web browser relay succeeded without returning any response body.",
        )
        .with_provider("chatgpt_web_reverse_compatible")
        .with_code("chatgpt_web_browser_relay_empty_body"));
    }
    Ok(ChatGptWebBrowserRelayResult {
        payload: refreshed_payload,
        status,
        content_type,
        body_text,
    })
}

pub async fn ensure_credential_runtime(
    redis_pool: &Pool,
    pg_pool: Option<&PgPool>,
    http: &Client,
    mut input: GatewayKeepaliveEnsureRequest,
) -> Result<GatewayKeepaliveEnsureResponse, GatewayError> {
    input.headers = external_gateway_headers(&input.headers);
    implementation_lines::assert_adapter_compiled(
        input.adapter.as_str(),
        "keepalive credential runtime requested",
    )?;

    let session_auth = normalize_keepalive_session_auth(input.session_auth.clone());
    let api_key = input
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let inferred_expires_at = decode_jwt_expiry_iso(api_key);
    let mut effective_session_auth = session_auth.clone();
    if let Some(config) = effective_session_auth.as_mut() {
        if config.expires_at.is_none() {
            config.expires_at = input.expires_at.clone().or(inferred_expires_at.clone());
        }
    }
    let effective_expires_at = effective_session_auth
        .as_ref()
        .and_then(|config| config.expires_at.clone())
        .or_else(|| input.expires_at.clone())
        .or(inferred_expires_at);
    let requested_runtime_state_object_key = input
        .runtime_state_object_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let requires_runtime_material =
        effective_session_auth.is_some() || input.adapter.trim() == "grok_compatible";

    if input.adapter.trim() == "chatgpt_web_reverse_compatible" {
        let payload = provider_payload_from_keepalive_request(
            &input,
            effective_session_auth.clone(),
            effective_expires_at.clone(),
        );
        if !chatgpt_web_should_refresh(&payload, false) {
            return Ok(GatewayKeepaliveEnsureResponse {
                ready: true,
                message: Some("ChatGPT Web reverse session is still fresh.".to_string()),
                api_key: input.api_key.clone(),
                headers: Some(input.headers.clone()),
                extra_body: input.extra_body.clone(),
                session_auth: effective_session_auth,
                keepalive: None,
                expires_at: effective_expires_at,
                runtime_state_object_key: requested_runtime_state_object_key,
                upstream_session_id: None,
            });
        }

        let refreshed = execute_chatgpt_web_session_worker(&payload).await?;
        persist_chatgpt_web_runtime_refresh(redis_pool, pg_pool, &payload, &refreshed).await;
        let refreshed_payload = apply_chatgpt_web_runtime_refresh(&payload, &refreshed);

        return Ok(GatewayKeepaliveEnsureResponse {
            ready: true,
            message: Some(
                "ChatGPT Web reverse session was refreshed from the browser-backed steward."
                    .to_string(),
            ),
            api_key: Some(refreshed_payload.api_key),
            headers: Some(refreshed_payload.headers),
            extra_body: refreshed_payload.extra_body,
            session_auth: refreshed_payload.session_auth,
            keepalive: None,
            expires_at: refreshed.expires_at,
            runtime_state_object_key: refreshed_payload.runtime_state_object_key,
            upstream_session_id: None,
        });
    }

    if input.adapter.trim() == "qwen_web_compatible" {
        let payload = provider_payload_from_keepalive_request(
            &input,
            effective_session_auth.clone(),
            effective_expires_at.clone(),
        );
        if !qwen_web_should_refresh(&payload, false) {
            return Ok(GatewayKeepaliveEnsureResponse {
                ready: true,
                message: Some("Qwen Web direct replay session is still fresh.".to_string()),
                api_key: input.api_key.clone(),
                headers: Some(input.headers.clone()),
                extra_body: input.extra_body.clone(),
                session_auth: effective_session_auth,
                keepalive: None,
                expires_at: effective_expires_at,
                runtime_state_object_key: requested_runtime_state_object_key,
                upstream_session_id: None,
            });
        }

        let refreshed = execute_qwen_web_session_worker(&payload, &input.model).await?;
        persist_qwen_web_runtime_refresh(redis_pool, pg_pool, &payload, &refreshed).await;
        let refreshed_payload = apply_qwen_web_runtime_refresh(&payload, &refreshed);

        return Ok(GatewayKeepaliveEnsureResponse {
            ready: true,
            message: Some(
                "Qwen Web session was refreshed from the browser-backed steward.".to_string(),
            ),
            api_key: Some(refreshed_payload.api_key),
            headers: Some(refreshed_payload.headers),
            extra_body: refreshed_payload.extra_body,
            session_auth: refreshed_payload.session_auth,
            keepalive: None,
            expires_at: refreshed.expires_at,
            runtime_state_object_key: refreshed_payload.runtime_state_object_key,
            upstream_session_id: None,
        });
    }

    if is_gemini_canvas_keepalive_adapter(&input.adapter) {
        return Ok(build_gemini_canvas_runtime_material_response(
            &input,
            effective_session_auth,
            effective_expires_at,
            requested_runtime_state_object_key,
        ));
    }

    if requires_runtime_material && api_key.is_none() {
        return Ok(GatewayKeepaliveEnsureResponse {
            ready: false,
            message: Some(
                "Missing session-backed credential material for keepalive ensure.".to_string(),
            ),
            api_key: None,
            headers: None,
            extra_body: None,
            session_auth: effective_session_auth,
            keepalive: None,
            expires_at: effective_expires_at,
            runtime_state_object_key: requested_runtime_state_object_key,
            upstream_session_id: None,
        });
    }

    if input.adapter.trim() == "chataibot_compatible" {
        if let Some(expires_at) = effective_expires_at.as_deref().and_then(parse_rfc3339) {
            if expires_at <= OffsetDateTime::now_utc() {
                return Ok(GatewayKeepaliveEnsureResponse {
                    ready: false,
                    message: Some(
                        "Chataibot token has expired and must be rotated by the external account worker."
                            .to_string(),
                    ),
                    api_key: input.api_key.clone(),
                    headers: Some(input.headers.clone()),
                    extra_body: input.extra_body.clone(),
                    session_auth: effective_session_auth,
                    keepalive: None,
                    expires_at: effective_expires_at,
                    runtime_state_object_key: requested_runtime_state_object_key,
                    upstream_session_id: None,
                });
            }
        }

        if let Some(api_key) = api_key {
            let headers = build_keepalive_probe_headers(
                Some(&input.headers),
                effective_session_auth.as_ref(),
                api_key,
            );
            let response = request_builder_with_headers(
                http,
                rquest::Method::GET,
                &format!(
                    "{}{}",
                    input.base_url.trim_end_matches('/'),
                    crate::protocol::chataibot::CHATAIBOT_QUOTA_PROBE_PATH
                ),
                &headers,
            )
            .send()
            .await;

            match response {
                Ok(response) if response.status().is_success() => {
                    let probe = response.json::<Value>().await.map_err(|error| {
                        GatewayError::server_error(format!(
                            "Chataibot quota probe JSON 解析失败: {error}"
                        ))
                    })?;
                    let remaining = probe
                        .get("leftAnswersCount")
                        .and_then(Value::as_i64)
                        .ok_or_else(|| {
                            GatewayError::server_error(
                                "Chataibot quota probe response did not include leftAnswersCount."
                                    .to_string(),
                            )
                        })?;
                    let required =
                        crate::protocol::chataibot::estimate_required_quota(&input.model);
                    if remaining < required {
                        return Ok(GatewayKeepaliveEnsureResponse {
                            ready: false,
                            message: Some(format!(
                                "Chataibot account quota {remaining} is below the required minimum {required} for model {}.",
                                input.model
                            )),
                            api_key: input.api_key.clone(),
                            headers: Some(input.headers.clone()),
                            extra_body: input.extra_body.clone(),
                            session_auth: effective_session_auth,
                            keepalive: None,
                            expires_at: effective_expires_at,
                            runtime_state_object_key: requested_runtime_state_object_key,
                            upstream_session_id: None,
                        });
                    }
                }
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    return Ok(GatewayKeepaliveEnsureResponse {
                        ready: false,
                        message: Some(if body.trim().is_empty() {
                            format!(
                                "Chataibot quota probe failed with HTTP {}; account must be refreshed externally.",
                                status
                            )
                        } else {
                            truncate_error_summary(&body, 1_000)
                        }),
                        api_key: input.api_key.clone(),
                        headers: Some(input.headers.clone()),
                        extra_body: input.extra_body.clone(),
                        session_auth: effective_session_auth,
                        keepalive: None,
                        expires_at: effective_expires_at,
                        runtime_state_object_key: requested_runtime_state_object_key,
                        upstream_session_id: None,
                    });
                }
                Err(error) => {
                    return Ok(GatewayKeepaliveEnsureResponse {
                        ready: false,
                        message: Some(format!("Chataibot quota probe failed: {error}")),
                        api_key: input.api_key.clone(),
                        headers: Some(input.headers.clone()),
                        extra_body: input.extra_body.clone(),
                        session_auth: effective_session_auth,
                        keepalive: None,
                        expires_at: effective_expires_at,
                        runtime_state_object_key: requested_runtime_state_object_key,
                        upstream_session_id: None,
                    });
                }
            }
        }
    }

    if input.adapter.trim() == "lumalabs_compatible" {
        let realm_id = read_extra_body_string(
            input.extra_body.as_ref(),
            &["realmId", "realm_id", "boardId", "board_id"],
        );
        let Some(realm_id) = realm_id else {
            return Ok(GatewayKeepaliveEnsureResponse {
                ready: false,
                message: Some(
                    "LumaLabs credentials require extraBody.realmId runtime material.".to_string(),
                ),
                api_key: input.api_key.clone(),
                headers: Some(input.headers.clone()),
                extra_body: input.extra_body.clone(),
                session_auth: effective_session_auth,
                keepalive: None,
                expires_at: effective_expires_at,
                runtime_state_object_key: requested_runtime_state_object_key,
                upstream_session_id: None,
            });
        };

        if let Some(api_key) = api_key {
            let mut headers = build_keepalive_probe_headers(
                Some(&input.headers),
                effective_session_auth.as_ref(),
                api_key,
            );
            headers.insert(
                "accept".to_string(),
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".to_string(),
            );

            let response = request_builder_with_headers(
                http,
                rquest::Method::GET,
                &format!(
                    "{}/board/{}",
                    input.base_url.trim_end_matches('/'),
                    realm_id.trim()
                ),
                &headers,
            )
            .redirect(rquest::redirect::Policy::none())
            .send()
            .await;

            match response {
                Ok(response) if response.status().is_redirection() => {
                    return Ok(GatewayKeepaliveEnsureResponse {
                        ready: false,
                        message: Some(format!(
                            "LumaLabs board probe redirected with HTTP {}; session likely requires re-login.",
                            response.status()
                        )),
                        api_key: input.api_key.clone(),
                        headers: Some(input.headers.clone()),
                        extra_body: input.extra_body.clone(),
                        session_auth: effective_session_auth,
                        keepalive: None,
                        expires_at: effective_expires_at,
                        runtime_state_object_key: requested_runtime_state_object_key,
                        upstream_session_id: None,
                    });
                }
                Ok(response) if response.status().is_success() => {
                    let body = response
                        .text()
                        .await
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    if body.contains("just a moment")
                        || body.contains("auth.lumalabs.ai")
                        || body.contains("sign in")
                        || body.contains("log in")
                    {
                        return Ok(GatewayKeepaliveEnsureResponse {
                            ready: false,
                            message: Some(
                                "LumaLabs board probe returned an auth/challenge page; session must be refreshed externally."
                                    .to_string(),
                            ),
                            api_key: input.api_key.clone(),
                            headers: Some(input.headers.clone()),
                            extra_body: input.extra_body.clone(),
                            session_auth: effective_session_auth,
                            keepalive: None,
                            expires_at: effective_expires_at,
                            runtime_state_object_key: requested_runtime_state_object_key,
                            upstream_session_id: None,
                        });
                    }
                }
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    return Ok(GatewayKeepaliveEnsureResponse {
                        ready: false,
                        message: Some(if body.trim().is_empty() {
                            format!(
                                "LumaLabs board probe failed with HTTP {}; account must be refreshed externally.",
                                status
                            )
                        } else {
                            truncate_error_summary(&body, 1_000)
                        }),
                        api_key: input.api_key.clone(),
                        headers: Some(input.headers.clone()),
                        extra_body: input.extra_body.clone(),
                        session_auth: effective_session_auth,
                        keepalive: None,
                        expires_at: effective_expires_at,
                        runtime_state_object_key: requested_runtime_state_object_key,
                        upstream_session_id: None,
                    });
                }
                Err(error) => {
                    return Ok(GatewayKeepaliveEnsureResponse {
                        ready: false,
                        message: Some(format!("LumaLabs board probe failed: {error}")),
                        api_key: input.api_key.clone(),
                        headers: Some(input.headers.clone()),
                        extra_body: input.extra_body.clone(),
                        session_auth: effective_session_auth,
                        keepalive: None,
                        expires_at: effective_expires_at,
                        runtime_state_object_key: requested_runtime_state_object_key,
                        upstream_session_id: None,
                    });
                }
            }
        }
    }

    if input.adapter.trim() == "producer_compatible" {
        if let Some(expires_at) = effective_expires_at.as_deref().and_then(parse_rfc3339) {
            if expires_at <= OffsetDateTime::now_utc() {
                return Ok(GatewayKeepaliveEnsureResponse {
                    ready: false,
                    message: Some(
                        "Producer session bearer has expired and must be rotated by the external account worker."
                            .to_string(),
                    ),
                    api_key: input.api_key.clone(),
                    headers: Some(input.headers.clone()),
                    extra_body: input.extra_body.clone(),
                    session_auth: effective_session_auth,
                    keepalive: None,
                    expires_at: effective_expires_at,
                    runtime_state_object_key: requested_runtime_state_object_key,
                    upstream_session_id: None,
                });
            }
        }

        if let Some(api_key) = api_key {
            let headers = build_keepalive_probe_headers(
                Some(&input.headers),
                effective_session_auth.as_ref(),
                api_key,
            );
            let response = request_builder_with_headers(
                http,
                rquest::Method::GET,
                &format!(
                    "{}/__api/billing/credits",
                    input.base_url.trim_end_matches('/')
                ),
                &headers,
            )
            .send()
            .await;

            match response {
                Ok(response) if response.status().is_success() => {}
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    return Ok(GatewayKeepaliveEnsureResponse {
                        ready: false,
                        message: Some(if body.trim().is_empty() {
                            format!(
                                "Producer credits probe failed with HTTP {}; session must be refreshed externally.",
                                status
                            )
                        } else {
                            truncate_error_summary(&body, 1_000)
                        }),
                        api_key: input.api_key.clone(),
                        headers: Some(input.headers.clone()),
                        extra_body: input.extra_body.clone(),
                        session_auth: effective_session_auth,
                        keepalive: None,
                        expires_at: effective_expires_at,
                        runtime_state_object_key: requested_runtime_state_object_key,
                        upstream_session_id: None,
                    });
                }
                Err(error) => {
                    return Ok(GatewayKeepaliveEnsureResponse {
                        ready: false,
                        message: Some(format!("Producer credits probe failed: {error}")),
                        api_key: input.api_key.clone(),
                        headers: Some(input.headers.clone()),
                        extra_body: input.extra_body.clone(),
                        session_auth: effective_session_auth,
                        keepalive: None,
                        expires_at: effective_expires_at,
                        runtime_state_object_key: requested_runtime_state_object_key,
                        upstream_session_id: None,
                    });
                }
            }
        }
    }

    if input.adapter.trim() == "suno_compatible" {
        let Some(cookie_header) = read_suno_cookie_header(&input) else {
            return Ok(GatewayKeepaliveEnsureResponse {
                ready: false,
                message: Some(
                    "Suno credentials require the raw browser Cookie header as the long-lived credential material."
                        .to_string(),
                ),
                api_key: input.api_key.clone(),
                headers: Some(input.headers.clone()),
                extra_body: input.extra_body.clone(),
                session_auth: effective_session_auth,
                keepalive: None,
                expires_at: effective_expires_at,
                runtime_state_object_key: requested_runtime_state_object_key,
                upstream_session_id: None,
            });
        };
        let Some(session_token) = extract_cookie_value(&cookie_header, "__session") else {
            return Ok(GatewayKeepaliveEnsureResponse {
                ready: false,
                message: Some(
                    "Suno Cookie material is missing the __session access token and must be refreshed from a signed-in browser."
                        .to_string(),
                ),
                api_key: input.api_key.clone(),
                headers: Some(input.headers.clone()),
                extra_body: input.extra_body.clone(),
                session_auth: effective_session_auth,
                keepalive: None,
                expires_at: effective_expires_at,
                runtime_state_object_key: requested_runtime_state_object_key,
                upstream_session_id: None,
            });
        };

        let runtime_expires_at = decode_jwt_expiry_iso(Some(session_token.as_str()))
            .or_else(|| effective_expires_at.clone());

        if let Some(config) = effective_session_auth.as_mut() {
            if config.expires_at.is_none() {
                config.expires_at = runtime_expires_at.clone();
            }
        }

        let mut runtime_headers = input.headers.clone();
        upsert_header_case_insensitive(&mut runtime_headers, "Cookie", cookie_header.clone());
        if read_header_case_insensitive(&runtime_headers, "Device-Id").is_none() {
            if let Some(device_id) = extract_cookie_value(&cookie_header, "ajs_anonymous_id") {
                upsert_header_case_insensitive(&mut runtime_headers, "Device-Id", device_id);
            }
        }
        upsert_header_case_insensitive(&mut runtime_headers, "Referring-Pathname", "/");
        upsert_header_case_insensitive(
            &mut runtime_headers,
            "Referring-Origin",
            "https://suno.com",
        );

        let mut probe_headers = build_keepalive_probe_headers(
            Some(&runtime_headers),
            effective_session_auth.as_ref(),
            &session_token,
        );
        upsert_header_case_insensitive(&mut probe_headers, "Cookie", cookie_header.clone());
        if let Some(device_id) = read_header_case_insensitive(&runtime_headers, "Device-Id") {
            upsert_header_case_insensitive(&mut probe_headers, "Device-Id", device_id);
        }
        upsert_header_case_insensitive(&mut probe_headers, "Referring-Pathname", "/");
        upsert_header_case_insensitive(&mut probe_headers, "Referring-Origin", "https://suno.com");

        let user_config_response = request_builder_with_headers(
            http,
            rquest::Method::POST,
            &format!(
                "{}/api/user/user_config/",
                input.base_url.trim_end_matches('/')
            ),
            &probe_headers,
        )
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await;

        match user_config_response {
            Ok(response)
                if response.status().is_success()
                    || (response.status().is_client_error()
                        && response.status().as_u16() != 401
                        && response.status().as_u16() != 403) => {}
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Ok(GatewayKeepaliveEnsureResponse {
                    ready: false,
                    message: Some(if body.trim().is_empty() {
                        format!(
                            "Suno user_config probe failed with HTTP {}; session must be refreshed externally.",
                            status
                        )
                    } else {
                        truncate_error_summary(&body, 1_000)
                    }),
                    api_key: input.api_key.clone(),
                    headers: Some(input.headers.clone()),
                    extra_body: input.extra_body.clone(),
                    session_auth: effective_session_auth,
                    keepalive: None,
                    expires_at: runtime_expires_at,
                    runtime_state_object_key: requested_runtime_state_object_key,
                    upstream_session_id: None,
                });
            }
            Err(error) => {
                return Ok(GatewayKeepaliveEnsureResponse {
                    ready: false,
                    message: Some(format!("Suno user_config probe failed: {error}")),
                    api_key: input.api_key.clone(),
                    headers: Some(input.headers.clone()),
                    extra_body: input.extra_body.clone(),
                    session_auth: effective_session_auth,
                    keepalive: None,
                    expires_at: runtime_expires_at,
                    runtime_state_object_key: requested_runtime_state_object_key,
                    upstream_session_id: None,
                });
            }
        }

        let challenge_response = request_builder_with_headers(
            http,
            rquest::Method::POST,
            &format!("{}/api/c/check", input.base_url.trim_end_matches('/')),
            &probe_headers,
        )
        .header("Content-Type", "application/json")
        .json(&json!({ "ctype": "generation" }))
        .send()
        .await;

        match challenge_response {
            Ok(response) if response.status().is_success() => {
                let body = response.json::<Value>().await.map_err(|error| {
                    GatewayError::server_error(format!(
                        "Suno challenge probe JSON 解析失败: {error}"
                    ))
                })?;
                if body.get("required").and_then(Value::as_bool).is_none() {
                    return Ok(GatewayKeepaliveEnsureResponse {
                        ready: false,
                        message: Some(
                            "Suno challenge probe response did not include the required flag."
                                .to_string(),
                        ),
                        api_key: input.api_key.clone(),
                        headers: Some(input.headers.clone()),
                        extra_body: input.extra_body.clone(),
                        session_auth: effective_session_auth,
                        keepalive: None,
                        expires_at: runtime_expires_at,
                        runtime_state_object_key: requested_runtime_state_object_key,
                        upstream_session_id: None,
                    });
                }
            }
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Ok(GatewayKeepaliveEnsureResponse {
                    ready: false,
                    message: Some(if body.trim().is_empty() {
                        format!(
                            "Suno challenge probe failed with HTTP {}; session must be refreshed externally.",
                            status
                        )
                    } else {
                        truncate_error_summary(&body, 1_000)
                    }),
                    api_key: input.api_key.clone(),
                    headers: Some(input.headers.clone()),
                    extra_body: input.extra_body.clone(),
                    session_auth: effective_session_auth,
                    keepalive: None,
                    expires_at: runtime_expires_at,
                    runtime_state_object_key: requested_runtime_state_object_key,
                    upstream_session_id: None,
                });
            }
            Err(error) => {
                return Ok(GatewayKeepaliveEnsureResponse {
                    ready: false,
                    message: Some(format!("Suno challenge probe failed: {error}")),
                    api_key: input.api_key.clone(),
                    headers: Some(input.headers.clone()),
                    extra_body: input.extra_body.clone(),
                    session_auth: effective_session_auth,
                    keepalive: None,
                    expires_at: runtime_expires_at,
                    runtime_state_object_key: requested_runtime_state_object_key,
                    upstream_session_id: None,
                });
            }
        }

        return Ok(GatewayKeepaliveEnsureResponse {
            ready: true,
            message: Some("Suno runtime material is ready.".to_string()),
            api_key: Some(session_token),
            headers: Some(runtime_headers),
            extra_body: input.extra_body.clone(),
            session_auth: effective_session_auth,
            keepalive: None,
            expires_at: runtime_expires_at,
            runtime_state_object_key: requested_runtime_state_object_key,
            upstream_session_id: None,
        });
    }

    let mut runtime_state_object_key = requested_runtime_state_object_key;
    let mut upstream_session_id = None;

    if let (Some(pg_pool), Some(project_id), Some(session_key)) = (
        pg_pool,
        input
            .project_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
        input
            .session_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    ) {
        let existing_session = db::resolve_gateway_session(
            pg_pool,
            project_id,
            Some(session_key),
            input.previous_response_id.as_deref(),
        )
        .await?;
        if runtime_state_object_key.is_none() {
            runtime_state_object_key = existing_session
                .as_ref()
                .and_then(|session| session.runtime_state_object_key.clone())
                .or_else(|| {
                    if adapter_uses_runtime_archive(&input.adapter) {
                        Some(hash_runtime_session_key(
                            &input.provider_account_id,
                            session_key,
                        ))
                    } else {
                        None
                    }
                });
        }
        upstream_session_id = existing_session
            .as_ref()
            .and_then(|session| session.upstream_session_id.clone());

        let _ = db::upsert_gateway_session(
            pg_pool,
            UpsertGatewaySessionInput {
                project_id: project_id.to_string(),
                session_key: session_key.to_string(),
                protocol_family: infer_keepalive_protocol_family(&input.adapter),
                provider_account_id: input.provider_account_id.trim().to_string(),
                upstream_session_id: upstream_session_id.clone(),
                runtime_state_object_key: runtime_state_object_key.clone(),
                latest_response_id: input.previous_response_id.clone(),
                active_request_audit_id: None,
            },
        )
        .await?;
    }

    let ready_message = if requires_runtime_material {
        "Credential runtime material is ready."
    } else {
        "Keepalive ensure no-op."
    };

    if let Some(credential_id) = input
        .credential_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let api_key_write_back = if input.adapter.trim() == "suno_compatible" {
            None
        } else {
            input
                .api_key
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
        };
        let _ = credential_cache::write_back_runtime_material(
            redis_pool,
            credential_id,
            api_key_write_back,
            Some(&input.headers),
            input.extra_body.as_ref(),
            effective_session_auth.as_ref(),
            None,
            effective_expires_at.as_deref(),
            runtime_state_object_key.as_deref(),
        )
        .await;
    }

    Ok(GatewayKeepaliveEnsureResponse {
        ready: true,
        message: Some(ready_message.to_string()),
        api_key: input.api_key,
        headers: Some(input.headers),
        extra_body: input.extra_body,
        session_auth: effective_session_auth,
        keepalive: None,
        expires_at: effective_expires_at,
        runtime_state_object_key,
        upstream_session_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::candidate::ProviderAccountPayload;
    use axum::{extract::State, http::HeaderMap as AxumHeaderMap, routing::post, Json, Router};
    use std::sync::{Arc, Mutex};
    use tokio::net::TcpListener;

    fn qwen_payload(expires_at: Option<&str>) -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: "qwen_web_compatible".to_string(),
            base_url: "https://chat.qwen.ai".to_string(),
            api_key: "session-token".to_string(),
            credential_id: Some("cred-1".to_string()),
            expires_at: expires_at.map(str::to_string),
            runtime_state_object_key: None,
            account_name: Some("qwen-web".to_string()),
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: Some("qwen3-coder-plus".to_string()),
            headers: HashMap::new(),
            auth_mode: Some("bearer".to_string()),
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: Some("/api/v2/chat/completions".to_string()),
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: None,
            session_auth: Some(SessionAuthConfig {
                transport: "bearer".to_string(),
                primary_cookie_name: None,
                secondary_cookie_name: None,
                header_name: Some("authorization".to_string()),
                expires_at: expires_at.map(str::to_string),
            }),
            keepalive: None,
        }
    }

    fn chatgpt_web_payload_for_oauth_refresh(token_endpoint: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: "chatgpt_web_reverse_compatible".to_string(),
            base_url: "https://chatgpt.com".to_string(),
            api_key: "expired-access-token".to_string(),
            credential_id: Some("chatgpt-web-cred-1".to_string()),
            expires_at: Some("2000-01-01T00:00:00Z".to_string()),
            runtime_state_object_key: None,
            account_name: Some("chatgpt-web".to_string()),
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: Some("gpt-5.4".to_string()),
            headers: HashMap::new(),
            auth_mode: Some("bearer".to_string()),
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: Some(HashMap::from([
                ("refreshToken".to_string(), json!("old-refresh-token")),
                ("oauthTokenEndpoint".to_string(), json!(token_endpoint)),
                ("oauthClientId".to_string(), json!("app-test-client")),
            ])),
            session_auth: Some(SessionAuthConfig {
                transport: "bearer".to_string(),
                primary_cookie_name: None,
                secondary_cookie_name: None,
                header_name: Some("authorization".to_string()),
                expires_at: Some("2000-01-01T00:00:00Z".to_string()),
            }),
            keepalive: None,
        }
    }

    #[test]
    fn chatgpt_web_request_time_browser_global_policy_overrides_provider_allowed() {
        let payload = chatgpt_web_payload_for_oauth_refresh("http://127.0.0.1/oauth/token");

        assert!(
            !chatgpt_web_request_time_browser_fallback_allowed_for_policy(
                &payload,
                RequestTimeBrowserPolicy::Disabled
            )
        );
        assert!(
            !chatgpt_web_request_time_browser_fallback_allowed_for_policy(
                &payload,
                RequestTimeBrowserPolicy::RemoteOnly
            )
        );
        assert!(
            chatgpt_web_request_time_browser_fallback_allowed_for_policy(
                &payload,
                RequestTimeBrowserPolicy::LocalAllowed
            )
        );
    }

    #[test]
    fn chatgpt_web_request_time_browser_local_policy_keeps_provider_disable() {
        let mut payload = chatgpt_web_payload_for_oauth_refresh("http://127.0.0.1/oauth/token");
        payload
            .extra_body
            .get_or_insert_with(HashMap::new)
            .insert("requestTimeBrowserAllowed".to_string(), json!(false));

        assert!(
            !chatgpt_web_request_time_browser_fallback_allowed_for_policy(
                &payload,
                RequestTimeBrowserPolicy::LocalAllowed
            )
        );
    }

    #[test]
    fn request_time_browser_worker_blocking_error_maps_policy_codes() {
        assert_eq!(
            request_time_local_browser_worker_blocking_error(
                RequestTimeBrowserPolicy::Disabled,
                "chatgpt_web_reverse_compatible",
            )
            .and_then(|error| error.code),
            Some("request_time_browser_forbidden".to_string())
        );
        assert_eq!(
            request_time_local_browser_worker_blocking_error(
                RequestTimeBrowserPolicy::RemoteOnly,
                "qwen_web_compatible",
            )
            .and_then(|error| error.code),
            Some("browser_executor_required_unavailable".to_string())
        );
        assert!(request_time_local_browser_worker_blocking_error(
            RequestTimeBrowserPolicy::LocalAllowed,
            "qwen_web_compatible",
        )
        .is_none());
    }

    #[test]
    fn qwen_web_refresh_forces_when_expired() {
        let payload = qwen_payload(Some("2000-01-01T00:00:00.000Z"));
        assert!(qwen_web_should_refresh(&payload, false));
    }

    #[test]
    fn qwen_web_refresh_skips_when_session_is_fresh() {
        let payload = qwen_payload(Some("2099-01-01T00:00:00.000Z"));
        assert!(!qwen_web_should_refresh(&payload, false));
    }

    #[tokio::test]
    async fn chatgpt_web_oauth_refresh_posts_form_and_returns_rotated_tokens() {
        #[derive(Clone, Default)]
        struct OAuthState {
            seen: Arc<Mutex<Vec<(Option<String>, String)>>>,
        }

        async fn record_oauth_refresh(
            State(state): State<OAuthState>,
            headers: AxumHeaderMap,
            body: String,
        ) -> Json<Value> {
            state.seen.lock().unwrap().push((
                headers
                    .get("content-type")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string),
                body,
            ));
            Json(json!({
                "access_token": "refreshed-access-token",
                "refresh_token": "rotated-refresh-token",
                "id_token": "id-token-123",
                "expires_in": 3600
            }))
        }

        let state = OAuthState::default();
        let app = Router::new()
            .route("/oauth/token", post(record_oauth_refresh))
            .with_state(state.clone());
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("axum serve");
        });

        let payload = chatgpt_web_payload_for_oauth_refresh(&format!("http://{addr}/oauth/token"));
        let refreshed = execute_chatgpt_web_oauth_refresh(&Client::new(), &payload)
            .await
            .expect("oauth refresh");

        assert_eq!(refreshed.api_key, "refreshed-access-token");
        assert_eq!(
            refreshed.refresh_token.as_deref(),
            Some("rotated-refresh-token")
        );
        assert_eq!(refreshed.id_token.as_deref(), Some("id-token-123"));
        assert!(refreshed.expires_at.is_some());

        let seen = state.seen.lock().unwrap().clone();
        assert_eq!(seen.len(), 1);
        assert!(seen[0]
            .0
            .as_deref()
            .is_some_and(|value| value.starts_with("application/x-www-form-urlencoded")));
        assert!(seen[0].1.contains("grant_type=refresh_token"));
        assert!(seen[0].1.contains("refresh_token=old-refresh-token"));
        assert!(seen[0].1.contains("client_id=app-test-client"));

        server.abort();
    }

    #[test]
    fn qwen_web_refresh_merges_cookie_header() {
        let mut headers = HashMap::new();
        headers.insert("Accept".to_string(), "application/json".to_string());
        headers.insert("x-neuro-account-group".to_string(), "premium".to_string());
        let merged = merge_qwen_web_runtime_headers(&headers, Some("token=abc"));
        assert_eq!(
            merged.get("Accept").map(String::as_str),
            Some("application/json")
        );
        assert_eq!(merged.get("Cookie").map(String::as_str), Some("token=abc"));
        assert!(!merged
            .keys()
            .any(|name| name.eq_ignore_ascii_case("x-neuro-account-group")));
    }

    #[test]
    fn chatgpt_web_runtime_header_merge_drops_internal_account_group_selectors() {
        let headers = HashMap::from([
            (
                "X-Account-Group-Id".to_string(),
                "legacy-premium".to_string(),
            ),
            ("Accept".to_string(), "application/json".to_string()),
        ]);

        let merged = merge_chatgpt_web_runtime_headers(&headers, Some("a=b"), None);

        assert!(!merged
            .keys()
            .any(|name| name.eq_ignore_ascii_case("x-account-group-id")));
        assert_eq!(
            merged.get("Accept").map(String::as_str),
            Some("application/json")
        );
        assert_eq!(merged.get("Cookie").map(String::as_str), Some("a=b"));
    }

    #[test]
    fn keepalive_probe_headers_drop_internal_account_group_selectors() {
        let headers = HashMap::from([
            ("x-neuro-account-group".to_string(), "premium".to_string()),
            (
                "X-Account-Group-Id".to_string(),
                "legacy-premium".to_string(),
            ),
            ("x-provider-runtime".to_string(), "allowed".to_string()),
        ]);

        let result = build_keepalive_probe_headers(Some(&headers), None, "session-token");

        assert!(!result
            .keys()
            .any(|name| name.eq_ignore_ascii_case("x-neuro-account-group")));
        assert!(!result
            .keys()
            .any(|name| name.eq_ignore_ascii_case("x-account-group-id")));
        assert_eq!(
            result.get("x-provider-runtime").map(String::as_str),
            Some("allowed")
        );
    }

    #[test]
    fn keepalive_probe_auth_cannot_reintroduce_internal_account_group_selector() {
        let session_auth = SessionAuthConfig {
            transport: "header".to_string(),
            primary_cookie_name: None,
            secondary_cookie_name: None,
            header_name: Some("X-Account-Group-Id".to_string()),
            expires_at: None,
        };

        let result = build_keepalive_probe_headers(None, Some(&session_auth), "session-token");

        assert!(!result
            .keys()
            .any(|name| name.eq_ignore_ascii_case("x-account-group-id")));
    }

    #[test]
    fn qwen_web_signin_headers_drop_internal_account_group_selectors() {
        let mut payload = qwen_payload(None);
        payload
            .headers
            .insert("x-neuro-account-group".to_string(), "premium".to_string());
        payload.headers.insert(
            "X-Account-Group-Id".to_string(),
            "legacy-premium".to_string(),
        );
        payload
            .headers
            .insert("x-provider-runtime".to_string(), "allowed".to_string());

        let result = qwen_web_signin_headers(&payload);

        assert!(!result
            .keys()
            .any(|name| name.eq_ignore_ascii_case("x-neuro-account-group")));
        assert!(!result
            .keys()
            .any(|name| name.eq_ignore_ascii_case("x-account-group-id")));
        assert_eq!(
            result.get("x-provider-runtime").map(String::as_str),
            Some("allowed")
        );
    }

    #[test]
    fn raw_keepalive_request_builder_drops_internal_account_group_selectors() {
        let headers = HashMap::from([
            ("x-neuro-account-group".to_string(), "premium".to_string()),
            (
                "X-Account-Group-Id".to_string(),
                "legacy-premium".to_string(),
            ),
            ("x-provider-runtime".to_string(), "allowed".to_string()),
        ]);

        let request = request_builder_with_headers(
            &Client::new(),
            rquest::Method::GET,
            "https://example.com/probe",
            &headers,
        )
        .build()
        .expect("build keepalive request");

        assert!(request.headers().get("x-neuro-account-group").is_none());
        assert!(request.headers().get("x-account-group-id").is_none());
        assert_eq!(
            request
                .headers()
                .get("x-provider-runtime")
                .and_then(|value| value.to_str().ok()),
            Some("allowed")
        );
    }

    #[test]
    fn qwen_web_signin_seed_reads_nested_auth_seed() {
        let mut extra_body = HashMap::new();
        extra_body.insert(
            "authSeed".to_string(),
            serde_json::json!({
                "type": "qwen_web_signin",
                "email": "user@example.com",
                "password": "secret-123"
            }),
        );
        let seed = qwen_web_signin_seed(Some(&extra_body)).expect("seed");
        assert_eq!(seed.email, "user@example.com");
        assert_eq!(seed.password.as_deref(), Some("secret-123"));
        assert_eq!(seed.password_sha256.as_deref(), None);
    }

    #[test]
    fn qwen_web_signin_password_attempts_include_plain_and_hash() {
        let seed = QwenWebSigninSeed {
            email: "user@example.com".to_string(),
            password: Some("secret-123".to_string()),
            password_sha256: None,
        };
        let attempts = qwen_web_signin_password_attempts(&seed);
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0], "secret-123");
        assert_eq!(attempts[1], sha256_hex("secret-123"));
    }

    #[test]
    fn extract_cookie_value_reads_named_entries() {
        let cookie_header = "__client=refresh-123; __session=session-456; other=1";
        assert_eq!(
            extract_cookie_value(cookie_header, "__session").as_deref(),
            Some("session-456")
        );
        assert_eq!(
            extract_cookie_value(cookie_header, "__client").as_deref(),
            Some("refresh-123")
        );
        assert_eq!(extract_cookie_value(cookie_header, "missing"), None);
    }

    #[test]
    fn read_suno_cookie_header_prefers_explicit_cookie_header() {
        let mut headers = HashMap::new();
        headers.insert("Cookie".to_string(), "__session=session-789".to_string());
        let request = GatewayKeepaliveEnsureRequest {
            project_id: None,
            session_key: None,
            previous_response_id: None,
            credential_id: None,
            account_name: None,
            provider_account_id: "provider-1".to_string(),
            adapter: "suno_compatible".to_string(),
            base_url: "https://studio-api-prod.suno.com".to_string(),
            model: "chirp-v3-5".to_string(),
            api_key: Some("__client=refresh-123".to_string()),
            headers,
            extra_body: None,
            session_auth: None,
            expires_at: None,
            runtime_state_object_key: None,
        };
        assert_eq!(
            read_suno_cookie_header(&request).as_deref(),
            Some("__session=session-789")
        );
    }

    #[tokio::test]
    async fn ensure_suno_runtime_material_derives_cookie_and_bearer() {
        #[derive(Clone, Default)]
        struct ProbeState {
            seen: Arc<
                Mutex<
                    Vec<(
                        String,
                        Option<String>,
                        Option<String>,
                        Option<String>,
                        Option<String>,
                        Option<String>,
                    )>,
                >,
            >,
        }

        async fn record_probe(
            State(state): State<ProbeState>,
            headers: AxumHeaderMap,
            body: String,
        ) -> Json<Value> {
            let is_challenge_probe = body.contains("\"ctype\":\"generation\"");
            state.seen.lock().unwrap().push((
                body,
                headers
                    .get("authorization")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string),
                headers
                    .get("cookie")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string),
                headers
                    .get("device-id")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string),
                headers
                    .get("referring-pathname")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string),
                headers
                    .get("referring-origin")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string),
            ));
            if is_challenge_probe {
                Json(json!({ "required": false }))
            } else {
                Json(json!({ "ok": true }))
            }
        }

        let state = ProbeState::default();
        let app = Router::new()
            .route("/api/user/user_config/", post(record_probe))
            .route("/api/c/check", post(record_probe))
            .with_state(state.clone());
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("axum serve");
        });

        let redis_pool = deadpool_redis::Config::from_url("redis://localhost:6379")
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .expect("redis pool");
        let http = Client::new();
        let session_token = "eyJhbGciOiJIUzI1NiJ9.eyJleHAiOjQxMDE4NDYwMDB9.signature";
        let cookie_header = format!(
            "__client=refresh-token-123; __session={session_token}; ajs_anonymous_id=device-123; other=1"
        );
        let response = ensure_credential_runtime(
            &redis_pool,
            None,
            &http,
            GatewayKeepaliveEnsureRequest {
                project_id: None,
                session_key: None,
                previous_response_id: None,
                credential_id: None,
                account_name: Some("suno-main".to_string()),
                provider_account_id: "provider-1".to_string(),
                adapter: "suno_compatible".to_string(),
                base_url: format!("http://{addr}"),
                model: "chirp-v3-5".to_string(),
                api_key: Some(cookie_header.clone()),
                headers: HashMap::from([(
                    "Accept".to_string(),
                    "application/json, text/plain, */*".to_string(),
                )]),
                extra_body: None,
                session_auth: Some(SessionAuthConfig {
                    transport: "bearer".to_string(),
                    primary_cookie_name: None,
                    secondary_cookie_name: None,
                    header_name: Some("authorization".to_string()),
                    expires_at: None,
                }),
                expires_at: None,
                runtime_state_object_key: None,
            },
        )
        .await
        .expect("ensure suno runtime");

        assert!(response.ready);
        assert_eq!(response.api_key.as_deref(), Some(session_token));
        assert_eq!(
            response
                .headers
                .as_ref()
                .and_then(|headers| read_header_case_insensitive(headers, "cookie"))
                .as_deref(),
            Some(cookie_header.as_str())
        );
        assert_eq!(
            response
                .headers
                .as_ref()
                .and_then(|headers| read_header_case_insensitive(headers, "device-id"))
                .as_deref(),
            Some("device-123")
        );
        assert_eq!(
            response
                .headers
                .as_ref()
                .and_then(|headers| read_header_case_insensitive(headers, "referring-pathname"))
                .as_deref(),
            Some("/")
        );
        assert_eq!(
            response
                .headers
                .as_ref()
                .and_then(|headers| read_header_case_insensitive(headers, "referring-origin"))
                .as_deref(),
            Some("https://suno.com")
        );
        let derived_expiry = response.expires_at.clone();
        assert!(derived_expiry.is_some());
        assert_eq!(
            response
                .session_auth
                .as_ref()
                .and_then(|session_auth| session_auth.expires_at.clone()),
            derived_expiry
        );

        let seen = state.seen.lock().unwrap().clone();
        assert_eq!(seen.len(), 2);
        assert!(seen
            .iter()
            .all(|(_, auth, cookie, device_id, pathname, origin)| {
                auth.as_deref() == Some(&format!("Bearer {session_token}"))
                    && cookie.as_deref() == Some(cookie_header.as_str())
                    && device_id.as_deref() == Some("device-123")
                    && pathname.as_deref() == Some("/")
                    && origin.as_deref() == Some("https://suno.com")
            }));

        server.abort();
    }

    #[tokio::test]
    async fn ensure_suno_runtime_material_allows_expired_session_token_if_probes_pass() {
        #[derive(Clone, Default)]
        struct ProbeState {
            seen: Arc<Mutex<Vec<String>>>,
        }

        async fn record_probe(State(state): State<ProbeState>, body: String) -> Json<Value> {
            state.seen.lock().unwrap().push(body.clone());
            if body.contains("\"ctype\":\"generation\"") {
                Json(json!({ "required": false }))
            } else {
                Json(json!({ "ok": true }))
            }
        }

        let state = ProbeState::default();
        let app = Router::new()
            .route("/api/user/user_config/", post(record_probe))
            .route("/api/c/check", post(record_probe))
            .with_state(state.clone());
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("axum serve");
        });

        let redis_pool = deadpool_redis::Config::from_url("redis://localhost:6379")
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .expect("redis pool");
        let http = Client::new();
        let expired_session_token = "eyJhbGciOiJIUzI1NiJ9.eyJleHAiOjE3MDAwMDAwMDB9.signature";
        let cookie_header = format!(
            "__client=refresh-token-123; __session={expired_session_token}; ajs_anonymous_id=device-123; other=1"
        );
        let response = ensure_credential_runtime(
            &redis_pool,
            None,
            &http,
            GatewayKeepaliveEnsureRequest {
                project_id: None,
                session_key: None,
                previous_response_id: None,
                credential_id: None,
                account_name: Some("suno-main".to_string()),
                provider_account_id: "provider-1".to_string(),
                adapter: "suno_compatible".to_string(),
                base_url: format!("http://{addr}"),
                model: "chirp-v3-5".to_string(),
                api_key: Some(cookie_header),
                headers: HashMap::new(),
                extra_body: None,
                session_auth: Some(SessionAuthConfig {
                    transport: "bearer".to_string(),
                    primary_cookie_name: None,
                    secondary_cookie_name: None,
                    header_name: Some("authorization".to_string()),
                    expires_at: None,
                }),
                expires_at: None,
                runtime_state_object_key: None,
            },
        )
        .await
        .expect("ensure suno runtime with expired token");

        assert!(response.ready);
        assert_eq!(state.seen.lock().unwrap().len(), 2);
        server.abort();
    }

    fn make_gemini_canvas_keepalive_request(adapter: &str) -> GatewayKeepaliveEnsureRequest {
        GatewayKeepaliveEnsureRequest {
            project_id: None,
            session_key: None,
            previous_response_id: None,
            credential_id: Some("cred-gemini".to_string()),
            account_name: Some("gemini-main".to_string()),
            provider_account_id: "provider-gemini".to_string(),
            adapter: adapter.to_string(),
            base_url: "https://gemini.google.com".to_string(),
            model: "gemini-2.5-pro".to_string(),
            api_key: Some(String::new()),
            headers: HashMap::new(),
            extra_body: None,
            session_auth: None,
            expires_at: None,
            runtime_state_object_key: None,
        }
    }

    #[test]
    fn infer_keepalive_protocol_family_maps_gemini_modular_surfaces() {
        assert_eq!(
            infer_keepalive_protocol_family(GEMINI_API_MODULAR_ADAPTER),
            GEMINI_GENERATE_CONTENT_FAMILY
        );
        assert_eq!(
            infer_keepalive_protocol_family(GEMINI_WEB_REVERSE_MODULAR_ADAPTER),
            GEMINI_WEB_CHAT_FAMILY
        );
        assert_eq!(
            infer_keepalive_protocol_family(GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER),
            "gemini_canvas"
        );
        assert_eq!(
            infer_keepalive_protocol_family(GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER),
            "gemini_canvas"
        );
    }

    #[test]
    fn build_gemini_canvas_runtime_material_response_accepts_modular_canvas_lines() {
        let future_expiry = Some("2100-01-01T00:00:00Z".to_string());

        let browser_response = build_gemini_canvas_runtime_material_response(
            &make_gemini_canvas_keepalive_request(GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER),
            None,
            future_expiry.clone(),
            Some("credential-runtime/gemini-canvas/browser/storage-state.json".to_string()),
        );
        assert!(browser_response.ready);
        assert_eq!(
            browser_response.runtime_state_object_key.as_deref(),
            Some("credential-runtime/gemini-canvas/browser/storage-state.json")
        );

        let program_response = build_gemini_canvas_runtime_material_response(
            &make_gemini_canvas_keepalive_request(
                GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
            ),
            None,
            future_expiry,
            Some("credential-runtime/gemini-canvas/program/storage-state.json".to_string()),
        );
        assert!(program_response.ready);
        assert_eq!(
            program_response.runtime_state_object_key.as_deref(),
            Some("credential-runtime/gemini-canvas/program/storage-state.json")
        );
    }

    #[test]
    fn build_gemini_canvas_runtime_material_response_requires_runtime_state_for_modular_program_line(
    ) {
        let response = build_gemini_canvas_runtime_material_response(
            &make_gemini_canvas_keepalive_request(
                GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
            ),
            None,
            Some("2100-01-01T00:00:00Z".to_string()),
            None,
        );
        assert!(!response.ready);
        assert_eq!(response.runtime_state_object_key, None);
        assert!(response
            .message
            .as_deref()
            .is_some_and(|message| message.contains("runtimeStateObjectKey")));
    }
}
