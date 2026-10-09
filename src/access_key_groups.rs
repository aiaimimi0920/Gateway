//! Key-bound entitlement groups restrict live route membership, never client headers.
use crate::{
    access_store::AccessStore,
    auth::session::AuthenticatedSession,
    db::UpsertAccessKeyInput,
    error::GatewayError,
    routing::config::{RouteAccountGroupConstraint, RouteConfigSnapshot},
    state::AppState,
};
use serde_json::Value;

pub fn group_ids(metadata: Option<&Value>) -> Result<Option<Vec<String>>, GatewayError> {
    let Some(value) = metadata.and_then(|value| value.get("accountGroupIds")) else {
        return Ok(None);
    };
    let ids = value
        .as_array()
        .filter(|ids| ids.len() <= 128)
        .ok_or_else(|| {
            GatewayError::bad_request("accountGroupIds must be an array of at most 128 group IDs")
        })?;
    let mut result = Vec::new();
    for value in ids {
        let id = value
            .as_str()
            .filter(|id| {
                !id.is_empty()
                    && id.trim() == *id
                    && id.len() <= 256
                    && !id.chars().any(char::is_control)
            })
            .ok_or_else(|| GatewayError::bad_request("Invalid accountGroupIds entry"))?;
        if !result.iter().any(|existing| existing == id) {
            result.push(id.to_owned());
        }
    }
    // Some(empty) intentionally denies all; it must never become legacy unrestricted access.
    Ok(Some(result))
}

pub fn validate_input(state: &AppState, input: &UpsertAccessKeyInput) -> Result<(), GatewayError> {
    let Some(ids) = group_ids(input.metadata.as_ref())? else {
        return Ok(());
    };
    if !matches!(input.key_kind.trim(), "normal" | "user") {
        return Err(GatewayError::bad_request(
            "Entitlement groups require a normal access key",
        ));
    }
    let snapshot = state.route_config.snapshot();
    for id in ids {
        snapshot.account_group_constraint(Some(&id)).map_err(|_| {
            GatewayError::bad_request(format!("Entitlement group '{id}' is missing or disabled"))
                .with_code("access_key_group_unavailable")
        })?;
    }
    Ok(())
}

pub async fn session_constraint(
    state: &AppState,
    session: Option<&AuthenticatedSession>,
    snapshot: &RouteConfigSnapshot,
) -> Result<Option<RouteAccountGroupConstraint>, GatewayError> {
    let Some(id) = session.and_then(|session| session.access_key_id.as_deref()) else {
        return Ok(None);
    };
    let metadata = AccessStore(state).metadata(id).await?;
    Ok(group_ids(metadata.as_ref())?.map(|ids| snapshot.access_key_group_constraint(&ids)))
}

pub fn denied() -> GatewayError {
    let mut error =
        GatewayError::unauthorized("Access key has no permitted account in its entitlement groups");
    error.http_status = Some(403);
    error.with_code("access_key_group_forbidden")
}
