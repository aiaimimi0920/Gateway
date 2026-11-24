//! Read-only operator inventory; unresolved sends are never guessed or replayed.
use super::*;
use axum::{extract::Query, routing::get};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    after: Option<String>,
}

pub(super) fn mount(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router.route(
        "/v1/internal/gateway/console/sap-aicore-responses/pending",
        get(read_pending).layer(map_response(no_store)),
    )
}

async fn read_pending(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(bearer): OptionalBearerToken,
    headers: HeaderMap,
    Query(cursor): Query<Cursor>,
) -> Result<Response, GatewayError> {
    authorize_actor(
        &state,
        &headers,
        bearer.as_deref(),
        info.as_ref(),
        false,
        true,
    )?;
    let rows = sap_aicore::responses_lifecycle::store::reconciliation::list(
        &state,
        cursor.after.as_deref(),
    )
    .await?;
    // A request that lost its management session during the read cannot disclose the inventory.
    authorize_actor(
        &state,
        &headers,
        bearer.as_deref(),
        info.as_ref(),
        false,
        true,
    )?;
    Ok(Json(serde_json::json!({"items":rows})).into_response())
}
