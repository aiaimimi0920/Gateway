//! Reuse the fixed Accio JSON transport; client identity is validated but never put in its wire.
use super::failed;
use crate::{
    console::refresh_grants::GrantResult,
    db::provider_credentials::refresh::Snapshot,
    error::GatewayError,
    protocol::accio_oauth::{OAuthClient, RefreshGrant, TokenMaterial},
};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub(super) fn prepare(snapshot: &Snapshot) -> Result<RefreshGrant, GatewayError> {
    let material = &snapshot.request.material;
    OAuthClient::new(&material.client).map_err(|_| failed("accio_db_steward_invalid_material"))?;
    RefreshGrant::new(
        &material.access,
        &material.refresh,
        material.accio_sgcookie.as_deref(),
    )
    .map_err(|_| failed("accio_db_steward_invalid_material"))
}

pub(super) fn result(
    snapshot: &Snapshot,
    tokens: TokenMaterial,
) -> Result<GrantResult, GatewayError> {
    let expiry = tokens
        .expires_at()
        .format(&Rfc3339)
        .map_err(|_| failed("accio_db_steward_invalid_result"))?;
    // Compatibility lifetime cannot alter the absolute deadline or authorize replay.
    let lifetime = (tokens.expires_at() - OffsetDateTime::now_utc())
        .whole_seconds()
        .max(1) as u64;
    snapshot
        .request
        .result_with_accio(
            tokens.access_token(),
            Some(tokens.refresh_token()),
            lifetime,
            &expiry,
            None,
            tokens.sgcookie().map(str::to_owned),
        )
        .map_err(|_| failed("accio_db_steward_invalid_result"))
}
