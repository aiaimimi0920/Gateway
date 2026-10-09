//! Management-only financial records share the same owner as quota settlement.
use super::{super::internal_gateway::assert_management_access, AccessKeyPath};
use crate::{
    access_balance::AccessBalanceStore, error::GatewayError, http::extractors::OptionalBearerToken,
    state::AppState,
};
use axum::{
    extract::{Path, State},
    http::{header::CACHE_CONTROL, HeaderMap},
    response::IntoResponse,
    Json,
};
use std::sync::Arc;

pub async fn get_cash_ledger(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<AccessKeyPath>,
) -> Result<impl IntoResponse, GatewayError> {
    assert_management_access(&state, token.as_deref(), &headers)?;
    let receipts = AccessBalanceStore::from_state(&state)?
        .cash_ledger(&path.access_key_id)
        .await?;
    Ok(([(CACHE_CONTROL, "no-store")], Json(receipts)))
}
