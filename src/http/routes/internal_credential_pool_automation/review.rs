//! Management-only reviewed cleanup. No secret material is returned in either DTO.
use super::*;
use crate::credential_pool_automation::maintenance::{self, PruneSelection};
use axum::response::{IntoResponse, Response};

pub async fn preview_credential_prune(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(provider_id): Path<String>,
) -> Result<Response, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let preview = maintenance::preview(&state, &provider_id).await?;
    Ok(no_store(serde_json::json!({ "preview": preview })))
}

pub async fn apply_credential_prune(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(provider_id): Path<String>,
    body: axum::body::Bytes,
) -> Result<Response, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let selection: PruneSelection = serde_json::from_slice(&body).map_err(|_| {
        GatewayError::bad_request("Invalid reviewed credential prune selection")
            .with_code("credential_prune_selection_invalid")
    })?;
    let receipt = maintenance::apply(&state, &provider_id, selection).await?;
    Ok(no_store(serde_json::json!({ "receipt": receipt })))
}

fn no_store(value: Value) -> Response {
    (
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        Json(value),
    )
        .into_response()
}
