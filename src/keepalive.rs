mod auth_material;
mod browser_policy;
mod headers;
mod metadata;
mod probes;
mod suno;

use auth_material::{decode_jwt_expiry_iso, normalize_keepalive_session_auth, parse_rfc3339};
use browser_policy::request_time_local_browser_worker_blocking_error;
#[cfg(test)]
use headers::build_keepalive_probe_headers;
use headers::{
    collect_set_cookie_header, external_gateway_headers, read_header_case_insensitive,
    request_builder_with_headers, upsert_header_case_insensitive,
};
use metadata::{
    read_extra_body_bool, read_extra_body_string, read_json_field_string, read_nested_json_string,
    truncate_error_summary,
};
#[cfg(test)]
use suno::{extract_cookie_value, read_suno_cookie_header, suno_cookie_header_from_storage_state};

mod ensure;
mod types;

pub use ensure::ensure_payload_ready;
pub use types::{GatewayKeepaliveEnsureRequest, GatewayKeepaliveEnsureResponse};

#[cfg(test)]
mod tests;

mod qwen_web;

pub use qwen_web::refresh_qwen_web_payload_after_challenge;
use qwen_web::{
    apply_qwen_web_runtime_refresh, ensure_qwen_web_payload_ready, execute_qwen_web_session_worker,
    persist_qwen_web_runtime_refresh, qwen_web_should_refresh,
};
#[cfg(test)]
use qwen_web::{
    merge_qwen_web_runtime_headers, qwen_web_signin_headers, qwen_web_signin_password_attempts,
    qwen_web_signin_seed, sha256_hex, QwenWebSigninSeed,
};

mod chatgpt_web;

pub(crate) use chatgpt_web::chatgpt_web_oauth_refresh_due;
use chatgpt_web::{
    apply_chatgpt_web_runtime_refresh, chatgpt_web_should_refresh,
    ensure_chatgpt_web_payload_ready, execute_chatgpt_web_session_worker,
    persist_chatgpt_web_runtime_refresh,
};
#[cfg(test)]
use chatgpt_web::{
    chatgpt_web_request_time_browser_fallback_allowed_for_policy,
    execute_chatgpt_web_oauth_refresh, merge_chatgpt_web_runtime_headers,
};
pub use chatgpt_web::{
    execute_chatgpt_web_browser_relay, refresh_chatgpt_web_oauth_payload_if_due,
    refresh_chatgpt_web_payload_after_challenge, ChatGptWebBrowserRelayResult,
};

#[cfg(test)]
use std::collections::HashMap;

use deadpool_redis::Pool;
use rquest::Client;
#[cfg(test)]
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::credential_runtime::SessionAuthConfig;
use crate::db;
use crate::db::UpsertGatewaySessionInput;
use crate::error::GatewayError;
use crate::implementation_lines;
use crate::protocol::gemini::shared::{
    GEMINI_API_MODULAR_ADAPTER, GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
    GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER, GEMINI_WEB_REVERSE_MODULAR_ADAPTER,
};
use crate::protocol::registry::{GEMINI_GENERATE_CONTENT_FAMILY, GEMINI_WEB_CHAT_FAMILY};
use crate::redis::credential_cache;
use crate::routing::candidate::ProviderAccountPayload;
#[cfg(test)]
use crate::upstream::request_time_browser_policy::RequestTimeBrowserPolicy;

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

    // An admission failure returns immediately; success keeps the same owned material.
    let probes::ProbeMaterial {
        effective_session_auth,
        effective_expires_at,
        requested_runtime_state_object_key,
    } = match probes::ensure_provider_probes(
        http,
        &input,
        api_key,
        probes::ProbeMaterial {
            effective_session_auth,
            effective_expires_at,
            requested_runtime_state_object_key,
        },
    )
    .await?
    {
        std::ops::ControlFlow::Break(response) => return Ok(response),
        std::ops::ControlFlow::Continue(material) => material,
    };

    if input.adapter.trim() == "suno_compatible" {
        return suno::ensure_runtime(
            http,
            &input,
            effective_session_auth,
            effective_expires_at,
            requested_runtime_state_object_key,
        )
        .await;
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
