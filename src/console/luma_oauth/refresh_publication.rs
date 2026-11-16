//! Explicit Luma refresh uses the existing bounded projection, transaction journal and recovery proof.
use crate::{
    console::{
        refresh_grants::{GrantLocator, GrantMaterial, GrantRecord, GrantRequest},
        runtime::refresh_authority::RefreshPublicationTicket,
        secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
        PersistenceError, RouteConfigRedisRevision,
    },
    routing::config::{ProviderCredentialYaml, RouteConfigSnapshot, RouteConfigYaml},
};

pub(super) struct Publication(pub GrantRecord);
impl RefreshPublicationTicket for Publication {
    fn identity(&self) -> (&str, &str) {
        (self.0.provider_id(), self.0.credential_id())
    }
    fn grant_locator(&self) -> Result<GrantLocator, PersistenceError> {
        GrantLocator::from_record(&self.0)
    }
    fn project(&self, snapshot: &RouteConfigSnapshot) -> Result<Option<RouteConfigYaml>, ()> {
        self.0.validate().map_err(|_| ())?;
        let revision =
            RouteConfigRedisRevision::new(snapshot.revision().clone(), snapshot.document().clone())
                .map_err(|_| ())?;
        if self.0.published(&revision).is_ok() {
            return Ok(None);
        }
        let (provider, id) = self.identity();
        let request = GrantRequest::from_revision(&revision, provider, id).map_err(|_| ())?;
        let result = self.0.replay_for(&request).ok_or(())?;
        let p = snapshot
            .document()
            .providers
            .iter()
            .find(|p| p.id == provider)
            .ok_or(())?;
        let mut material = p
            .credentials
            .iter()
            .find(|c| c.id.as_deref() == Some(id))
            .ok_or(())?
            .clone();
        material.api_key = Some(result.material.access.clone());
        material.refresh_token = Some(result.material.refresh.clone());
        material.luma_oauth = result.material.luma_oauth.clone();
        project(snapshot.document(), provider, id, &material)
            .map(Some)
            .ok_or(())
    }
}

/// Shared by online admission and archived recovery; all unrelated policy and credentials stay unchanged.
pub(crate) fn project(
    before: &RouteConfigYaml,
    provider: &str,
    id: &str,
    material: &ProviderCredentialYaml,
) -> Option<RouteConfigYaml> {
    let pi = before
        .providers
        .iter()
        .position(|p| p.id == provider && crate::routing::config::luma_mcp::is_provider(p))?;
    let p = &before.providers[pi];
    let ci = p
        .credentials
        .iter()
        .position(|c| c.id.as_deref() == Some(id))?;
    let original = GrantMaterial::from_credential(&p.credentials[ci], &p.api_key).ok()?;
    let successor = GrantMaterial::from_credential(material, &p.api_key).ok()?;
    if original.luma_oauth.is_none()
        || !original.permits(&successor)
        || material.expires_at.is_some()
        || material.token_expires_in_secs.is_some()
    {
        return None;
    }
    let mut redacted = redact_route_document(before).ok()?;
    redacted.document.providers[pi].credentials[ci].luma_oauth = material.luma_oauth.clone();
    let access = format!("/providers/{pi}/credentials/{ci}/api_key");
    let refresh = format!("/providers/{pi}/credentials/{ci}/refresh_token");
    let mut patches: Vec<_> = redacted
        .secrets
        .into_iter()
        .filter(|s| s.path != access && s.path != refresh)
        .map(|s| SecretPatch::keep(s.path))
        .collect();
    patches.push(SecretPatch::replace(
        access,
        serde_json::json!(successor.access),
    ));
    patches.push(SecretPatch::replace(
        refresh,
        serde_json::json!(successor.refresh),
    ));
    let document = resolve_secret_patches(before, redacted.document, &patches).ok()?;
    crate::console::document::validate_route_document(document.clone()).ok()?;
    Some(document)
}
