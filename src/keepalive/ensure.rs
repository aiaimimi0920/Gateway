//! Request-time dispatch to provider refresh owners or an external steward.

use deadpool_redis::Pool;
use rquest::Client;
use sqlx::PgPool;
use std::collections::HashMap;

use super::{
    ensure_chatgpt_web_payload_ready, ensure_qwen_web_payload_ready, external_gateway_headers,
    GatewayKeepaliveEnsureRequest, GatewayKeepaliveEnsureResponse,
};
use crate::error::{classify_network_error, classify_upstream_error, GatewayError};
use crate::http::request_headers::is_internal_gateway_header;
use crate::implementation_lines;
use crate::redis::credential_cache;
use crate::routing::candidate::ProviderAccountPayload;

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
