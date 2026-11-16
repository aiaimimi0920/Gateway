//! Compiler-owned, non-secret catalog/binding projection. Never merge it into an upstream body.
use super::{invalid, AuthorizationContext, ModelCatalog};
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};
use serde::{Deserialize, Serialize};
const KEY: &str = "gateway_lobsterai_dispatch";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Metadata {
    pub context: AuthorizationContext,
    pub generation: u64,
    pub catalog: Option<ModelCatalog>,
    #[serde(default)]
    pub selection: Option<super::virtual_model::Selection>,
}
pub(crate) fn configure(
    payload: &mut ProviderAccountPayload,
    context: AuthorizationContext,
    catalog: Option<ModelCatalog>,
) -> Result<(), GatewayError> {
    let value = serde_json::to_value(Metadata {
        context,
        catalog,
        generation: 0,
        selection: None,
    })
    .map_err(|_| invalid())?;
    payload.extra_body = Some([(KEY.into(), value)].into());
    Ok(())
}
pub(crate) fn read(payload: &ProviderAccountPayload) -> Result<Metadata, GatewayError> {
    let map = payload
        .extra_body
        .as_ref()
        .filter(|m| m.len() == 1)
        .ok_or_else(invalid)?;
    serde_json::from_value(map.get(KEY).ok_or_else(invalid)?.clone()).map_err(|_| invalid())
}
pub(super) fn selected(
    payload: &mut ProviderAccountPayload,
    selection: super::virtual_model::Selection,
) {
    let value = payload
        .extra_body
        .as_mut()
        .and_then(|m| m.get_mut(KEY))
        .expect("compiler-owned Lobster metadata");
    value["selection"] = serde_json::to_value(selection).expect("resolved model is serializable");
}
pub(crate) fn update(
    payload: &mut ProviderAccountPayload,
    context: &AuthorizationContext,
    generation: u64,
) {
    if let Some(value) = payload.extra_body.as_mut().and_then(|m| m.get_mut(KEY)) {
        value["context"] = serde_json::to_value(context).expect("Lobster context is serializable");
        value["generation"] = generation.into();
    }
}
