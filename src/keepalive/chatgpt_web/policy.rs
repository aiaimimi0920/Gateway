//! Refresh timing and browser-fallback admission for ChatGPT credentials.
use super::super::{
    read_extra_body_bool, read_extra_body_string, read_header_case_insensitive,
    request_time_local_browser_worker_blocking_error,
};
use crate::credential_runtime::SessionAuthConfig;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::request_time_browser_policy::RequestTimeBrowserPolicy;

const CHATGPT_WEB_REFRESH_BEFORE_SECS: u64 = 300;
pub(super) fn chatgpt_web_effective_session_auth(
    payload: &ProviderAccountPayload,
) -> Option<SessionAuthConfig> {
    let mut session_auth = payload.session_auth.clone()?;
    if session_auth.expires_at.is_none() {
        session_auth.expires_at = payload.expires_at.clone();
    }
    Some(session_auth)
}

pub(in crate::keepalive) fn chatgpt_web_should_refresh(
    payload: &ProviderAccountPayload,
    force_refresh: bool,
) -> bool {
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

pub(in crate::keepalive) fn chatgpt_web_request_time_browser_fallback_allowed_for_policy(
    payload: &ProviderAccountPayload,
    policy: RequestTimeBrowserPolicy,
) -> bool {
    request_time_local_browser_worker_blocking_error(policy, "chatgpt_web_reverse_compatible")
        .is_none()
        && chatgpt_web_provider_request_time_browser_fallback_allowed(payload)
}

pub(super) fn chatgpt_web_request_time_browser_fallback_allowed(
    payload: &ProviderAccountPayload,
) -> bool {
    chatgpt_web_request_time_browser_fallback_allowed_for_policy(
        payload,
        RequestTimeBrowserPolicy::from_env(),
    )
}
