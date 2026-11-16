//! Fixed consumption-site identity and normalized credential shape; not proof of login or quota.
use crate::{
    error::GatewayError,
    protocol::{
        call_flow::{CallAdapter, FingerprintAdapter, SpecialAdapter},
        request_profile::RequestClientProfile,
    },
    routing::candidate::{ProviderAccountPayload, ProviderExecutionMode},
};
pub const PRESET_ID: &str = "perplexity-web";
pub const ADAPTER: &str = "perplexity_web_compatible";
pub const AUTH_MODE: &str = "perplexity_web_session";
pub const BASE_URL: &str = "https://www.perplexity.ai";
pub const SESSION_COOKIE: &str = "__Secure-next-auth.session-token";
pub const ASK_PATH: &str = "/rest/sse/perplexity_ask";

pub fn owns_base(base: &str) -> bool {
    url::Url::parse(base)
        .ok()
        .is_some_and(|url| url.host_str() == Some("www.perplexity.ai"))
}
pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.adapter == ADAPTER
        || payload.auth_mode.as_deref() == Some(AUTH_MODE)
        || owns_base(&payload.base_url)
        || payload
            .session_auth
            .as_ref()
            .is_some_and(|s| s.primary_cookie_name.as_deref() == Some(SESSION_COOKIE))
}
pub(crate) fn invalid() -> GatewayError {
    super::input_failure(
        "perplexity_web_material_invalid",
        "Perplexity Web 材料或产品目标无效；未发送请求。",
    )
}

pub fn validate(payload: &ProviderAccountPayload, adapter: &str) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    if !matches!(
        payload.base_url.as_str(),
        BASE_URL | "https://www.perplexity.ai/"
    ) || payload.adapter != ADAPTER
        || adapter != ADAPTER
        || payload.auth_mode.as_deref() != Some(AUTH_MODE)
        || !matches!(payload.default_model.as_deref(), None | Some("turbo"))
        || !matches!(
            payload.request_client_profile,
            None | Some(RequestClientProfile::Auto)
        )
        || payload.auth_header_name.is_some()
        || payload.auth_token.is_some()
        || payload.keepalive.is_some()
        || payload.runtime_state_object_key.is_some()
        || !payload.discovered_protocols.is_empty()
        || payload.extra_body.as_ref().is_some_and(|b| !b.is_empty())
        || payload
            .execution_mode
            .is_some_and(|m| m != ProviderExecutionMode::DirectHttp)
        || payload
            .endpoint_execution_modes
            .as_ref()
            .is_some_and(|m| !m.is_empty())
        || payload
            .chat_completions_path
            .as_deref()
            .is_some_and(|p| p != ASK_PATH)
        || [
            &payload.responses_path,
            &payload.messages_path,
            &payload.completions_path,
            &payload.embeddings_path,
            &payload.audio_transcriptions_path,
            &payload.audio_speech_path,
            &payload.search_path,
            &payload.fetch_path,
            &payload.research_path,
            &payload.balance_path,
        ]
        .iter()
        .any(|p| p.is_some())
        || payload
            .headers
            .keys()
            .any(|h| !h.eq_ignore_ascii_case("cookie") && !h.eq_ignore_ascii_case("user-agent"))
        || payload.call_flow.is_some_and(|f| {
            f.call_adapter != CallAdapter::Auto
                || !matches!(
                    f.special_adapter,
                    SpecialAdapter::Auto | SpecialAdapter::PerplexityWeb
                )
                || !matches!(
                    f.fingerprint_adapter,
                    FingerprintAdapter::Auto | FingerprintAdapter::Session
                )
        })
    {
        return Err(invalid());
    }
    if let Some(session) = &payload.session_auth {
        if session.transport != "cookie"
            || session.primary_cookie_name.as_deref() != Some(SESSION_COOKIE)
            || session.secondary_cookie_name.is_some()
            || session.header_name.is_some()
            || session.expires_at != payload.expires_at
        {
            return Err(invalid());
        }
    }
    expiry(payload)?;
    if !payload.api_key.is_empty()
        || payload
            .headers
            .keys()
            .any(|h| h.eq_ignore_ascii_case("cookie"))
    {
        parts(payload)?;
    }
    Ok(())
}

pub(crate) struct Parts<'a> {
    pub token: &'a str,
    pub clearance: Option<&'a str>,
    pub user_agent: &'a str,
}
pub(crate) fn cookie_value(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 16 * 1024
        && value
            .bytes()
            .all(|v| matches!(v, 0x21 | 0x23..=0x2b | 0x2d..=0x3a | 0x3c..=0x5b | 0x5d..=0x7e))
}
fn header<'a>(
    payload: &'a ProviderAccountPayload,
    name: &str,
) -> Result<Option<&'a str>, GatewayError> {
    let mut entries = payload
        .headers
        .iter()
        .filter(|(k, _)| k.eq_ignore_ascii_case(name));
    let value = entries.next().map(|(_, v)| v.as_str());
    if entries.next().is_some() {
        return Err(invalid());
    }
    Ok(value)
}
pub(crate) fn parts(payload: &ProviderAccountPayload) -> Result<Parts<'_>, GatewayError> {
    let token = payload.api_key.as_str();
    let user_agent = header(payload, "user-agent")?.ok_or_else(invalid)?;
    if payload.session_auth.is_none()
        || !cookie_value(token)
        || user_agent.trim().is_empty()
        || user_agent.len() > 1024
        || !user_agent.bytes().all(|v| matches!(v, 0x20..=0x7e))
    {
        return Err(invalid());
    }
    let mut clearance = None;
    if let Some(cookie) = header(payload, "cookie")? {
        if cookie.len() > 34 * 1024 {
            return Err(invalid());
        }
        let mut primary = None;
        for item in cookie.split(';') {
            let (name, value) = item.trim().split_once('=').ok_or_else(invalid)?;
            if !cookie_value(value) {
                return Err(invalid());
            }
            match name {
                SESSION_COOKIE if primary.is_none() => primary = Some(value),
                "cf_clearance" if clearance.is_none() => clearance = Some(value),
                _ => return Err(invalid()),
            }
        }
        if primary != Some(token) {
            return Err(invalid());
        }
    }
    Ok(Parts {
        token,
        clearance,
        user_agent,
    })
}
pub(crate) fn expiry(
    payload: &ProviderAccountPayload,
) -> Result<Option<time::OffsetDateTime>, GatewayError> {
    payload
        .expires_at
        .as_deref()
        .map(|s| {
            time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339)
                .map_err(|_| invalid())
        })
        .transpose()
}
