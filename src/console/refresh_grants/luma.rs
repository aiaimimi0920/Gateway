//! Luma's durable send identity derives from explicit offline context, never generic refresh overrides.
use super::{invalid, GrantMaterial, GrantRequest, GrantResult};
use crate::{
    console::PersistenceError,
    protocol::luma_oauth::{self, AuthorizationContext},
};
use time::format_description::well_known::Rfc3339;

pub(super) fn expiry(context: &AuthorizationContext) -> Option<String> {
    context
        .expires_at()
        .map(|at| at.format(&Rfc3339).expect("validated Luma timestamp"))
}

pub(super) fn validate(material: &GrantMaterial) -> Result<(), PersistenceError> {
    // Requiring typed context even at the fixed endpoint prevents stripping its consent/binding checks.
    let context = material.luma_oauth.as_ref().ok_or_else(invalid)?;
    if material.endpoint != luma_oauth::TOKEN_ENDPOINT
        || material.client != context.client_id()
        || !context.offline_access()
        || material.context.is_some()
        || material.accio_sgcookie.is_some()
        || material.kiro_oauth.is_some()
        || material.vertex_oauth.is_some()
        || material.dim_oauth.is_some()
        || material.nous_portal.is_some()
        || material.lobsterai.is_some()
        || !luma_oauth::valid_access(&material.access)
        || !luma_oauth::valid_refresh(&material.refresh)
        || material
            .expires_at
            .as_ref()
            .is_some_and(|value| value.len() > 64)
        || !material.matches_expiry(expiry(context).as_deref())
    {
        return Err(invalid());
    }
    // An expired access token is precisely a reason to refresh; do not restore usable bearer material here.
    Ok(())
}

pub(super) fn result(
    original: &GrantMaterial,
    received: &luma_oauth::ReceivedRefresh,
) -> Result<GrantResult, PersistenceError> {
    original.validate()?;
    let (tokens, refresh) = received
        .interpret_stored(
            original.luma_oauth.as_ref().ok_or_else(invalid)?,
            &original.refresh,
        )
        .map_err(|_| invalid())?
        .into_parts();
    let context = AuthorizationContext::from_stored_material(&tokens).map_err(|_| invalid())?;
    let mut material = original.clone();
    material.access = tokens.access_token().into();
    material.refresh = refresh;
    material.expires_at = expiry(&context);
    material.luma_oauth = Some(context);
    let result = GrantResult {
        expires_at: material.expires_at.clone(),
        material,
        expires_in: None,
    };
    result.validate(original)?;
    Ok(result)
}

impl GrantRequest {
    pub(crate) fn result_with_luma(
        &self,
        received: &luma_oauth::ReceivedRefresh,
    ) -> Result<GrantResult, PersistenceError> {
        result(&self.material, received)
    }
}
