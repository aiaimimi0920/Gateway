use base64::Engine;
use percent_encoding::percent_decode_str;
use rquest::header::{HeaderMap, HeaderValue};
use rquest::Client;
use serde_json::json;
use serde_json::Value;
use std::time::Duration;

use crate::upstream::header_map_helpers::{extract_bearer_token, header_map_string};

const PRODUCER_SESSION_REFRESH_MARGIN_SECS: u64 = 300;

pub(crate) const DEFAULT_PRODUCER_SUPABASE_ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6ImVkbmpjY3FjbWJ4ZWF4YmlkaW5yIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NzE1NjEwNjQsImV4cCI6MjA4NzEzNzA2NH0.XCXSuL7Th1xHecfRrP0vAOFmKwJxwBqVFLu06SxtVzg";

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ProducerSupabaseRefreshResponse {
    pub(crate) access_token: String,
}

#[derive(Debug)]
pub(crate) struct ProducerSupabaseSession {
    pub(crate) access_token: Option<String>,
    pub(crate) refresh_token: Option<String>,
}

pub(crate) fn extract_producer_supabase_session(
    cookie_header: &str,
) -> Option<ProducerSupabaseSession> {
    let mut chunks = cookie_header
        .split(';')
        .filter_map(|segment| {
            let (name, value) = segment.trim().split_once('=')?;
            let name = name.trim();
            let suffix = name.rsplit_once('.')?.1;
            if !name.contains("auth-token") {
                return None;
            }
            let chunk_index = suffix.parse::<u32>().ok()?;
            let decoded = percent_decode_str(value.trim())
                .decode_utf8_lossy()
                .to_string();
            Some((chunk_index, decoded))
        })
        .collect::<Vec<_>>();
    if chunks.is_empty() {
        return None;
    }
    chunks.sort_by_key(|(index, _)| *index);
    let combined = chunks
        .into_iter()
        .map(|(_, value)| value)
        .collect::<String>();
    let stripped = strip_outer_quotes(combined.trim());
    let normalized = stripped.replace("\\\"", "\"");
    for candidate in [combined.as_str(), stripped, normalized.as_str()] {
        let Some(parsed) = parse_producer_cookie_payload(candidate) else {
            continue;
        };
        let access_token = read_nested_string(
            &parsed,
            &[
                &["currentSession", "access_token"],
                &["access_token"],
                &["session", "access_token"],
            ],
        );
        let refresh_token = read_nested_string(
            &parsed,
            &[
                &["currentSession", "refresh_token"],
                &["refresh_token"],
                &["session", "refresh_token"],
            ],
        );
        if access_token.is_some() || refresh_token.is_some() {
            return Some(ProducerSupabaseSession {
                access_token,
                refresh_token,
            });
        }
    }
    None
}

pub(crate) fn producer_bearer_needs_refresh(token: Option<&str>) -> bool {
    let Some(token) = token else {
        return true;
    };
    let Some(expiry) = decode_jwt_expiry(token) else {
        return false;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    now + PRODUCER_SESSION_REFRESH_MARGIN_SECS >= expiry
}

pub(crate) async fn maybe_refresh_producer_headers(
    http: &Client,
    headers: &HeaderMap,
) -> HeaderMap {
    let mut effective = headers.clone();
    let cookie_header = match header_map_string(headers, "cookie") {
        Some(value) => value,
        None => return effective,
    };
    let Some(session) = extract_producer_supabase_session(&cookie_header) else {
        return effective;
    };
    let current_bearer = extract_bearer_token(headers).or(session.access_token.clone());
    let refresh_token = session.refresh_token.as_deref();
    let resolved_bearer = if producer_bearer_needs_refresh(current_bearer.as_deref()) {
        match refresh_token {
            Some(token) => refresh_producer_supabase_access_token(http, token)
                .await
                .or(current_bearer),
            None => current_bearer,
        }
    } else {
        current_bearer
    };

    if let Some(bearer) = resolved_bearer {
        if let Ok(value) = HeaderValue::from_str(&format!("Bearer {bearer}")) {
            effective.insert(rquest::header::AUTHORIZATION, value);
        }
    }

    effective
}

async fn refresh_producer_supabase_access_token(
    http: &Client,
    refresh_token: &str,
) -> Option<String> {
    let response = http
        .post("https://sb.producer.ai/auth/v1/token?grant_type=refresh_token")
        .header("apikey", DEFAULT_PRODUCER_SUPABASE_ANON_KEY)
        .header(
            "authorization",
            format!("Bearer {}", DEFAULT_PRODUCER_SUPABASE_ANON_KEY),
        )
        .header("content-type", "application/json;charset=UTF-8")
        .header("x-client-info", "gateway-producer-live/1.0")
        .json(&json!({
            "refresh_token": refresh_token,
        }))
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let payload: ProducerSupabaseRefreshResponse = response.json().await.ok()?;
    let access_token = payload.access_token.trim();
    if access_token.is_empty() {
        return None;
    }
    Some(access_token.to_string())
}

fn parse_producer_cookie_payload(candidate: &str) -> Option<Value> {
    let normalized = strip_outer_quotes(candidate.trim());
    let decoded = if let Some(encoded) = normalized.strip_prefix("base64-") {
        let mut padded = encoded.trim().to_string();
        let remainder = padded.len() % 4;
        if remainder != 0 {
            padded.extend(std::iter::repeat('=').take(4 - remainder));
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(padded.as_bytes())
            .ok()?;
        String::from_utf8(bytes).ok()?
    } else {
        normalized.to_string()
    };

    let mut current = serde_json::from_str::<Value>(&decoded).ok()?;
    for _ in 0..2 {
        let Value::String(text) = current else {
            break;
        };
        current = serde_json::from_str::<Value>(&text).ok()?;
    }
    Some(current)
}

fn read_nested_string(value: &Value, path_options: &[&[&str]]) -> Option<String> {
    path_options.iter().find_map(|path| {
        let mut current = value;
        for segment in *path {
            current = current.get(*segment)?;
        }
        current
            .as_str()
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_string)
    })
}

fn strip_outer_quotes(value: &str) -> &str {
    value.trim_matches('"')
}

fn decode_jwt_expiry(token: &str) -> Option<u64> {
    let payload = token.split('.').nth(1)?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.as_bytes())
        .ok()?;
    let body = serde_json::from_slice::<Value>(&decoded).ok()?;
    body.get("exp")?.as_u64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_producer_supabase_session_decodes_chunked_base64_cookie_payload() {
        let payload =
            r#"{"currentSession":{"access_token":"access-123","refresh_token":"refresh-456"}}"#;
        let encoded = format!(
            "base64-{}",
            base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                payload.as_bytes()
            )
        );
        let split = encoded.len() / 2;
        let cookie_header = format!(
            "sb-producer-auth-token.0={}; sb-producer-auth-token.1={}",
            &encoded[..split],
            &encoded[split..]
        );

        let session = extract_producer_supabase_session(&cookie_header)
            .expect("chunked producer auth cookie should decode");
        assert_eq!(session.access_token.as_deref(), Some("access-123"));
        assert_eq!(session.refresh_token.as_deref(), Some("refresh-456"));
    }

    #[test]
    fn producer_bearer_needs_refresh_respects_jwt_expiry_margin() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let near_expiry = format!(
            "header.{}.sig",
            base64::Engine::encode(
                &base64::engine::general_purpose::URL_SAFE_NO_PAD,
                format!(r#"{{"exp":{}}}"#, now + 60).as_bytes()
            )
        );
        let far_expiry = format!(
            "header.{}.sig",
            base64::Engine::encode(
                &base64::engine::general_purpose::URL_SAFE_NO_PAD,
                format!(r#"{{"exp":{}}}"#, now + 3600).as_bytes()
            )
        );

        assert!(producer_bearer_needs_refresh(None));
        assert!(producer_bearer_needs_refresh(Some(near_expiry.as_str())));
        assert!(!producer_bearer_needs_refresh(Some(far_expiry.as_str())));
    }

    #[test]
    fn producer_supabase_refresh_response_deserializes_access_token() {
        let payload: ProducerSupabaseRefreshResponse =
            serde_json::from_str(r#"{"access_token":"access-123"}"#)
                .expect("refresh response should deserialize");

        assert_eq!(payload.access_token, "access-123");
        assert!(DEFAULT_PRODUCER_SUPABASE_ANON_KEY.starts_with("eyJ"));
    }

    #[tokio::test]
    async fn maybe_refresh_producer_headers_uses_cookie_access_token_without_refresh() {
        let mut headers = rquest::header::HeaderMap::new();
        let payload = r#"{"currentSession":{"access_token":"cookie-access","refresh_token":null}}"#;
        let encoded = format!(
            "base64-{}",
            base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                payload.as_bytes()
            )
        );
        headers.insert(
            "cookie",
            rquest::header::HeaderValue::from_str(&format!("sb-producer-auth-token.0={encoded}"))
                .expect("cookie header"),
        );

        let refreshed = maybe_refresh_producer_headers(&rquest::Client::new(), &headers).await;

        assert_eq!(
            refreshed
                .get(rquest::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer cookie-access")
        );
    }
}
