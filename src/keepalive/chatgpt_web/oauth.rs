//! OAuth refresh transport and token rotation; browser fallback belongs to the caller.
use super::super::{
    decode_jwt_expiry_iso, parse_rfc3339, read_extra_body_string, read_header_case_insensitive,
    truncate_error_summary,
};
use super::policy::chatgpt_web_effective_session_auth;
use super::types::ChatGptWebRefreshedRuntime;
use crate::db;
use crate::error::{classify_network_error, GatewayError};
use crate::routing::candidate::ProviderAccountPayload;
use rquest::Client;
use serde::Deserialize;
use time::OffsetDateTime;

const CHATGPT_WEB_OAUTH_REFRESH_BEFORE_SECS: u64 = 24 * 60 * 60;
const CHATGPT_WEB_OAUTH_TOKEN_ENDPOINT: &str = "https://auth.openai.com/oauth/token";
const CHATGPT_WEB_OAUTH_CLIENT_ID: &str = "app_2SKx67EdpoN0G6j64rFvigXD";
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

pub(super) fn chatgpt_web_should_oauth_refresh(
    payload: &ProviderAccountPayload,
    force_refresh: bool,
) -> bool {
    chatgpt_web_oauth_refresh_due(
        payload,
        CHATGPT_WEB_OAUTH_REFRESH_BEFORE_SECS,
        force_refresh,
    )
}

fn future_iso_after_secs(seconds: u64) -> Option<String> {
    let seconds = seconds.min(i64::MAX as u64) as i64;
    Some(db::format_timestamp(
        OffsetDateTime::now_utc() + time::Duration::seconds(seconds),
    ))
}

pub(in crate::keepalive) async fn execute_chatgpt_web_oauth_refresh(
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
