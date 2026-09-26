//! Provider admission probes; success retains material for session persistence.

use super::auth_material::parse_rfc3339;
use super::headers::{build_keepalive_probe_headers, request_builder_with_headers};
use super::metadata::{read_extra_body_string, truncate_error_summary};
use super::{GatewayKeepaliveEnsureRequest, GatewayKeepaliveEnsureResponse};
use crate::{credential_runtime::SessionAuthConfig, error::GatewayError};
use rquest::Client;
use serde_json::Value;
use std::ops::ControlFlow;
use time::OffsetDateTime;

pub(super) struct ProbeMaterial {
    pub(super) effective_session_auth: Option<SessionAuthConfig>,
    pub(super) effective_expires_at: Option<String>,
    pub(super) requested_runtime_state_object_key: Option<String>,
}

pub(super) async fn ensure_provider_probes(
    http: &Client,
    input: &GatewayKeepaliveEnsureRequest,
    api_key: Option<&str>,
    material: ProbeMaterial,
) -> Result<ControlFlow<GatewayKeepaliveEnsureResponse, ProbeMaterial>, GatewayError> {
    let ProbeMaterial {
        effective_session_auth,
        effective_expires_at,
        requested_runtime_state_object_key,
    } = material;

    if input.adapter.trim() == "chataibot_compatible" {
        if let Some(expires_at) = effective_expires_at.as_deref().and_then(parse_rfc3339) {
            if expires_at <= OffsetDateTime::now_utc() {
                return Ok(ControlFlow::Break(GatewayKeepaliveEnsureResponse {
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
                }));
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
                        return Ok(ControlFlow::Break(GatewayKeepaliveEnsureResponse {
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
                        }));
                    }
                }
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    return Ok(ControlFlow::Break(GatewayKeepaliveEnsureResponse {
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
                    }));
                }
                Err(error) => {
                    return Ok(ControlFlow::Break(GatewayKeepaliveEnsureResponse {
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
                    }));
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
            return Ok(ControlFlow::Break(GatewayKeepaliveEnsureResponse {
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
            }));
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
                    return Ok(ControlFlow::Break(GatewayKeepaliveEnsureResponse {
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
                    }));
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
                        return Ok(ControlFlow::Break(GatewayKeepaliveEnsureResponse {
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
                        }));
                    }
                }
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    return Ok(ControlFlow::Break(GatewayKeepaliveEnsureResponse {
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
                    }));
                }
                Err(error) => {
                    return Ok(ControlFlow::Break(GatewayKeepaliveEnsureResponse {
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
                    }));
                }
            }
        }
    }

    if input.adapter.trim() == "producer_compatible" {
        if let Some(expires_at) = effective_expires_at.as_deref().and_then(parse_rfc3339) {
            if expires_at <= OffsetDateTime::now_utc() {
                return Ok(ControlFlow::Break(GatewayKeepaliveEnsureResponse {
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
                }));
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
                    return Ok(ControlFlow::Break(GatewayKeepaliveEnsureResponse {
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
                    }));
                }
                Err(error) => {
                    return Ok(ControlFlow::Break(GatewayKeepaliveEnsureResponse {
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
                    }));
                }
            }
        }
    }

    Ok(ControlFlow::Continue(ProbeMaterial {
        effective_session_auth,
        effective_expires_at,
        requested_runtime_state_object_key,
    }))
}
