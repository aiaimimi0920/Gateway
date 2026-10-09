// ---------------------------------------------------------------------------
// http/routes/models.rs — GET /v1/models
//
// Returns the list of models available through the gateway, compatible with
// the OpenAI /v1/models response format.
// ---------------------------------------------------------------------------

use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, response::IntoResponse, Json};

use crate::http::extractors::OptionalBearerToken;
use crate::http::request_headers::build_public_auth_request;
use crate::provider_runtime;
use crate::state::AppState;

/// GET /v1/models — list available models.
///
/// Returns an OpenAI-compatible model list derived from the route
/// configuration (providers, exact routes, and aliases).
pub async fn handle_models(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> impl IntoResponse {
    let session = crate::auth::authenticate_request(
        state.as_ref(),
        &build_public_auth_request(
            &state.config,
            token.as_deref(),
            &headers,
            "/v1/models",
            "GET",
        ),
        headers
            .get("x-credential-ref")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
    )
    .await?;

    let local_id = crate::local_runtime::access_keys::local_key_id(&state, Some(&session));
    let mut models = if let (Some(local), Some(id)) = (&state.local_runtime, local_id) {
        local.local_access_models(id, &state.route_config).await?
    } else if let Some(pg_pool) = &state.pg_pool {
        provider_runtime::sweep_cooling_provider_accounts_best_effort(&state, 10).await;
        if let Some(access_key_id) = session.access_key_id.as_deref() {
            crate::db::list_models_for_access_key(pg_pool, &state.redis_pool, access_key_id).await?
        } else {
            crate::db::list_models_for_project(pg_pool, &session.project_id).await?
        }
    } else {
        state.route_config.list_models()
    };
    if local_id.is_none() {
        let snapshot = state.route_config.snapshot();
        if let Some(groups) =
            crate::access_key_groups::session_constraint(&state, Some(&session), &snapshot).await?
        {
            // Group restrictions narrow server entitlements; they never grant a missing bundle right.
            models.retain(|model| {
                !snapshot
                    .resolve_candidates_with_constraint(Some(&model.id), Some(&groups))
                    .candidates
                    .is_empty()
            });
        }
    }
    Ok::<_, crate::error::GatewayError>(Json(serde_json::json!({
        "object": "list",
        "data": models,
    })))
}
