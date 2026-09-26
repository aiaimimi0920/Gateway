//! Session transport normalization and token expiry decoding.

use crate::{credential_runtime::SessionAuthConfig, db};
use base64::Engine;
use serde_json::Value;
use time::OffsetDateTime;

pub(super) fn normalize_keepalive_session_auth(
    value: Option<SessionAuthConfig>,
) -> Option<SessionAuthConfig> {
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

pub(super) fn decode_jwt_expiry_iso(token: Option<&str>) -> Option<String> {
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

pub(super) fn parse_rfc3339(value: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(value.trim(), &time::format_description::well_known::Rfc3339).ok()
}
