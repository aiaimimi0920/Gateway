//! DB/Redis must supply the credential's whole grant and catalog; parent inheritance is forbidden.
use super::{business, AuthorizationContext, ModelCatalog};
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};
use serde_json::Value;

pub(crate) fn project(
    payload: &ProviderAccountPayload,
    raw: Option<&Value>,
    model: Option<&str>,
) -> Result<Option<(ProviderAccountPayload, &'static str)>, GatewayError> {
    if !business::owns_payload(payload) {
        return Ok(None);
    }
    let raw = raw.ok_or_else(business::invalid)?;
    let context: AuthorizationContext = serde_json::from_value(
        raw.get("dim_oauth")
            .cloned()
            .ok_or_else(business::invalid)?,
    )
    .map_err(|_| business::invalid())?;
    business::validate_context(payload, &context)?;
    let catalog: ModelCatalog = serde_json::from_value(
        raw.get("dim_catalog")
            .cloned()
            .ok_or_else(business::invalid)?,
    )
    .map_err(|_| business::invalid())?;
    catalog.validate(&context)?;
    catalog
        .project(payload, model.ok_or_else(business::invalid)?)
        .map(Some)
}
