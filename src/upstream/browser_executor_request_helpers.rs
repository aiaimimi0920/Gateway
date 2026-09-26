use rquest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::canonical::EndpointKind;

fn read_json_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
}

pub(crate) fn read_json_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key)?.as_u64()
}

pub(crate) fn read_json_bool(value: &Value, key: &str) -> Option<bool> {
    value.get(key)?.as_bool()
}

pub(crate) fn missing_browser_executor_field_error(
    provider: &str,
    field: &str,
    code: &str,
) -> GatewayError {
    GatewayError::bad_request(format!("{provider} browser executor requires {field}."))
        .with_code(code)
}

pub(crate) fn unsupported_browser_executor_provider_error(provider: &str) -> GatewayError {
    GatewayError::bad_request(format!(
        "Unsupported browser executor provider '{provider}'."
    ))
    .with_code("browser_executor_unsupported_provider")
}

pub(crate) fn browser_execution_status_from_message(message: &str) -> String {
    let normalized = message.trim().to_ascii_lowercase();
    if normalized.contains("challenge") || normalized.contains("cloudflare") {
        return "challenge".to_string();
    }
    if normalized.contains("timeout") {
        return "timed_out".to_string();
    }
    if normalized.contains("crash")
        || normalized.contains("browser closed")
        || normalized.contains("target closed")
        || normalized.contains("execution context destroyed")
    {
        return "crashed".to_string();
    }
    "released".to_string()
}

pub(crate) fn build_browser_executor_header_map(input: &Value) -> HeaderMap {
    let mut headers = HeaderMap::new();

    if let Some(cookie_header) = read_json_string(input, "cookieHeader") {
        if let Ok(value) = HeaderValue::from_str(&cookie_header) {
            headers.insert(rquest::header::COOKIE, value);
        }
    }
    if let Some(auth_token) = read_json_string(input, "authToken") {
        let bearer = format!("Bearer {auth_token}");
        if let Ok(value) = HeaderValue::from_str(&bearer) {
            headers.insert(rquest::header::AUTHORIZATION, value);
        }
    }

    for (json_key, header_name) in [
        ("userAgent", "user-agent"),
        ("acceptLanguage", "accept-language"),
        ("origin", "origin"),
        ("referer", "referer"),
        ("deviceId", "device-id"),
        ("browserToken", "browser-token"),
        ("referringPathname", "referring-pathname"),
        ("referringOrigin", "referring-origin"),
    ] {
        if let Some(value) = read_json_string(input, json_key) {
            if let (Ok(name), Ok(header_value)) = (
                HeaderName::from_bytes(header_name.as_bytes()),
                HeaderValue::from_str(&value),
            ) {
                headers.insert(name, header_value);
            }
        }
    }

    headers
}

pub(crate) fn browser_executor_endpoint_kind_key(endpoint_kind: EndpointKind) -> &'static str {
    match endpoint_kind {
        EndpointKind::ChatCompletions => "chat_completions",
        EndpointKind::Completions => "completions",
        EndpointKind::Embeddings => "embeddings",
        EndpointKind::Messages => "messages",
        EndpointKind::Responses => "responses",
        EndpointKind::Search => "search",
        EndpointKind::Fetch => "fetch",
        EndpointKind::ImagesGenerations => "images_generations",
        EndpointKind::ImagesEdits => "images_edits",
        EndpointKind::MusicGenerations => "music_generations",
        EndpointKind::VideosGenerations => "videos_generations",
        EndpointKind::AudioTranscriptions => "audio_transcriptions",
        EndpointKind::AudioSpeech => "audio_speech",
        EndpointKind::ResearchCreate => "research_create",
        EndpointKind::ResearchList => "research_list",
        EndpointKind::ResearchGet => "research_get",
        EndpointKind::CreditsBalance => "credits_balance",
    }
}
