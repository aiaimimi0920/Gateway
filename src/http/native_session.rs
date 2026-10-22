//! Explicit, standalone-owner Codex session compatibility; disabled unless opted in.
//! A path label partitions state but grants no authority. Responses keep the pipeline.
use super::{extractors::OptionalBearerToken, native_json::NativeJson};
use crate::{
    error::GatewayError,
    http_client::{native_cookie_sessions, NativeCodexConnection},
    state::AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Router,
};
use std::{collections::HashMap, sync::Arc};

const PREFIX: &str = "/codex/session/:session/:model/backend-api";

#[path = "native_session_alias.rs"]
mod alias;
pub(crate) use alias::normalize_headers;
#[path = "native_session_sdk.rs"]
mod sdk;
pub(crate) use sdk::attach_sdk_session;

pub(crate) fn mount(router: Router<Arc<AppState>>, state: &AppState) -> Router<Arc<AppState>> {
    if std::env::var("GATEWAY_EXPERIMENTAL_CODEX_SESSION").as_deref() != Ok("1") {
        return router;
    }
    router.merge(
        Router::new()
            .route(&format!("{PREFIX}/wham/settings/user"), get(settings))
            .route(&format!("{PREFIX}/codex/responses"), post(responses))
            .layer(super::middleware::body_limit_layer(
                state.config.max_body_responses_bytes,
            )),
    )
}

fn valid_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
}

fn owner_bearer<'a>(
    token: Option<&'a str>,
    expected: Option<&str>,
) -> Result<&'a str, GatewayError> {
    token
        .filter(|token| !token.is_empty() && Some(*token) == expected)
        .ok_or_else(|| GatewayError::unauthorized("Native session owner bearer required"))
}

async fn admit(
    state: &Arc<AppState>,
    token: Option<&str>,
    headers: &HeaderMap,
    session: &str,
    model: &str,
    path: &str,
    method: &str,
) -> Result<
    (
        native_cookie_sessions::Scope,
        crate::routing::candidate::ProviderAccountPayload,
    ),
    GatewayError,
> {
    if !valid_label(session) || !valid_label(model) {
        return Err(GatewayError::bad_request(
            "Invalid native session or model label",
        ));
    }
    // Initial compatibility mode is deliberately not a new multi-tenant auth API.
    // Reject managed/access-key deployments rather than bypassing their projections.
    if state.pg_pool.is_some()
        || !state.auth_adapters.is_empty()
        || state.config.gateway_api_key.is_none()
    {
        return Err(GatewayError::bad_request(
            "Native sessions currently require standalone shared-key authentication",
        ));
    }
    // Scope must use the credential actually authenticated, not a bearer shadowed
    // by an API-key alias in the general authentication request.
    let token = owner_bearer(token, state.config.gateway_api_key.as_deref())?;
    let authenticated = crate::auth::authenticate_request(
        state,
        &super::request_headers::build_public_auth_request(
            &state.config,
            Some(token),
            headers,
            path,
            method,
        ),
        None,
    )
    .await?;
    if authenticated.api_key_id.as_deref() != Some(crate::auth::session::SHARED_SECRET_API_KEY_ID) {
        return Err(GatewayError::unauthorized(
            "Native sessions require the standalone owner key",
        ));
    }
    let snapshot = state.route_config.snapshot();
    let mut candidates = snapshot
        .resolve_candidates_with_constraint(Some(model), None)
        .candidates;
    if candidates.len() != 1 {
        return Err(GatewayError::bad_request(
            "Native sessions require one unambiguous configured candidate",
        ));
    }
    let payload = candidates.remove(0).payload;
    if !crate::upstream::client::native_session::eligible(&payload) {
        return Err(GatewayError::bad_request(
            "Native sessions require the official Codex OAuth provider",
        ));
    }
    Ok((native_cookie_sessions::scope(token, session), payload))
}

async fn settings(
    State(state): State<Arc<AppState>>,
    Path((session, model)): Path<(String, String)>,
    OptionalBearerToken(token): OptionalBearerToken,
    Extension(connection): Extension<NativeCodexConnection>,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let path = format!("/codex/session/{session}/{model}/backend-api/wham/settings/user");
    let (scope, payload) = admit(
        &state,
        token.as_deref(),
        &headers,
        &session,
        &model,
        &path,
        "GET",
    )
    .await?;
    let (status, bytes) = state
        .upstream_client
        .native_user_settings(&payload, scope, &connection, &headers)
        .await?;
    // No upstream Set-Cookie, credentials or arbitrary response headers escape.
    // Redirects are not followed or exposed as redirect instructions to the CLI.
    if (300..400).contains(&status) {
        return Err(GatewayError::server_error(
            "Native settings redirect rejected",
        ));
    }
    Ok((
        StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY),
        [
            ("content-type", "application/json"),
            ("cache-control", "no-store"),
        ],
        bytes,
    )
        .into_response())
}

async fn responses(
    State(state): State<Arc<AppState>>,
    Path((session, model)): Path<(String, String)>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    mut body: NativeJson,
) -> Result<Response, GatewayError> {
    let path = format!("/codex/session/{session}/{model}/backend-api/codex/responses");
    let (scope, _) = admit(
        &state,
        token.as_deref(),
        &headers,
        &session,
        &model,
        &path,
        "POST",
    )
    .await?;
    if body.value.get("model").and_then(|v| v.as_str()) != Some(model.as_str())
        || body.value.get("stream").and_then(|v| v.as_bool()) != Some(true)
    {
        return Err(GatewayError::bad_request(
            "Native session requires the bound model and streaming Responses",
        ));
    }
    body.wire.native_session = Some(scope);
    super::routes::responses::handle_responses(
        State(state),
        OptionalBearerToken(token),
        Query(query),
        headers,
        body,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_session_labels_cannot_be_paths_or_unbounded_keys() {
        assert!(valid_label("f74b3a-gpt-6-sol"));
        for label in ["", "../../secret", "a?b", "a\\b", "a\r\n"] {
            assert!(!valid_label(label));
        }
        assert!(!valid_label(&"x".repeat(129)));
    }
    #[test]
    fn native_session_scope_requires_the_real_owner_bearer() {
        assert!(owner_bearer(None, Some("owner")).is_err());
        assert!(owner_bearer(Some("arbitrary"), Some("owner")).is_err());
        assert!(owner_bearer(Some(""), Some("")).is_err());
        assert_eq!(owner_bearer(Some("owner"), Some("owner")).unwrap(), "owner");
    }
}
