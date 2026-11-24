//! Management-only discovery of unresolved durable work. No tokens, prompts,
//! upstream IDs, replay authorization or inferred usage are returned.
use super::{public_id, storage, Store};
use crate::{error::GatewayError, state::AppState};
use serde::Serialize;

#[derive(sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Pending {
    id: String,
    public_model: String,
    provider_ref: String,
    credential_ref: String,
    phase: String,
    revision: i64,
    upstream_known: bool,
    balance_open: bool,
    mutation_pending: bool,
}

pub(crate) async fn list(
    state: &AppState,
    after: Option<&str>,
) -> Result<Vec<Pending>, GatewayError> {
    if after.is_some_and(|id| !public_id(id)) {
        return Err(GatewayError::bad_request("Invalid stored response cursor."));
    }
    let store = Store::from_state(state).ok_or_else(super::dispatch::not_found)?;
    let sql = "SELECT r.id,p.public_model,r.provider_ref,r.credential_ref,r.phase,r.revision,
        r.upstream_id IS NOT NULL AS upstream_known,
        EXISTS(SELECT 1 FROM gateway_sap_response_balance_holds h
            WHERE h.response_id=r.id AND h.open=1) AS balance_open,
        EXISTS(SELECT 1 FROM gateway_sap_response_mutations m
            WHERE m.response_id=r.id AND m.confirmed=0) AS mutation_pending
        FROM gateway_sap_response_receipts r
        JOIN gateway_sap_response_route_pins p ON p.response_id=r.id
        WHERE r.id > $1 AND (EXISTS(SELECT 1 FROM gateway_sap_response_balance_holds h
            WHERE h.response_id=r.id AND h.open=1)
            OR EXISTS(SELECT 1 FROM gateway_sap_response_mutations m
                WHERE m.response_id=r.id AND m.confirmed=0))
        ORDER BY r.id LIMIT 100";
    run!(&store, pool => sqlx::query_as::<_, Pending>(sql)
        .bind(after.unwrap_or("")).fetch_all(*pool).await)
}
