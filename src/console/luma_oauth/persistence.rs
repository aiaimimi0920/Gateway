//! Normal SecretPatch and document validation own writes; no private OAuth storage or identity inference.
use super::{session::Ready, Binding};
use crate::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    error::GatewayError,
    routing::config::{ProviderCredentialYaml, RouteConfigYaml},
    state::AppState,
};
use serde_json::json;
use tokio::time::Instant;

pub(super) struct PreparedSave {
    pub document: RouteConfigYaml,
    pub credential_id: String,
    pub valid_until: Instant,
}
pub(super) fn prepare(
    state: &AppState,
    binding: &Binding,
    ready: &Ready,
    deadline: Instant,
) -> Result<PreparedSave, GatewayError> {
    let snapshot = super::validate_target(state, binding)?;
    let mut valid_until = deadline;
    if ready.context.client_id() != binding.client
        || ready.context.redirect_uri() != binding.redirect
        || ready.context.offline_access() != binding.offline_access
        || ready.material.refresh_token().is_some() != binding.offline_access
    {
        return Err(invalid());
    }
    if let Some(at) = ready.material.expires_at() {
        let remaining = at - time::OffsetDateTime::now_utc();
        if remaining <= time::Duration::ZERO {
            return Err(invalid());
        }
        // Never renew the grant at save time; cap the existing session budget by original expiry.
        let now = Instant::now();
        valid_until = now
            + remaining
                .unsigned_abs()
                .min(deadline.saturating_duration_since(now));
    }
    if Instant::now() >= valid_until {
        return Err(invalid());
    }
    let mut redacted = redact_route_document(snapshot.document()).map_err(|_| invalid())?;
    let pi = redacted
        .document
        .providers
        .iter()
        .position(|p| p.id == binding.provider)
        .ok_or_else(invalid)?;
    let provider = &mut redacted.document.providers[pi];
    let id = binding
        .credential
        .clone()
        .unwrap_or_else(|| format!("luma-{}", uuid::Uuid::new_v4()));
    let ci = if binding.credential.is_some() {
        provider
            .credentials
            .iter()
            .position(|c| c.id.as_ref() == Some(&id))
            .ok_or_else(invalid)?
    } else {
        let credential: ProviderCredentialYaml =
            serde_json::from_value(json!({"id":id,"enabled":false})).map_err(|_| invalid())?;
        provider.credentials.push(credential);
        provider.credentials.len() - 1
    };
    let target = &mut provider.credentials[ci];
    target.luma_oauth = Some(ready.context.clone());
    target.enabled = Some(false);
    let path = format!("/providers/{pi}/credentials/{ci}/api_key");
    let refresh_path = format!("/providers/{pi}/credentials/{ci}/refresh_token");
    let mut patches: Vec<_> = redacted
        .secrets
        .into_iter()
        .filter(|d| d.path != path && d.path != refresh_path)
        .map(|d| SecretPatch::keep(d.path))
        .collect();
    patches.push(SecretPatch::replace(
        &path,
        json!(ready.material.access_token()),
    ));
    // A new access grant must never inherit an old refresh token, even for ordinary reauthorization.
    patches.push(match ready.material.refresh_token() {
        Some(value) => SecretPatch::replace(&refresh_path, json!(value)),
        None => SecretPatch::clear(&refresh_path),
    });
    let document = resolve_secret_patches(snapshot.document(), redacted.document, &patches)
        .map_err(|_| invalid())?;
    crate::console::document::validate_route_document(document.clone()).map_err(|_| invalid())?;
    Ok(PreparedSave {
        document,
        credential_id: id,
        valid_until,
    })
}
fn invalid() -> GatewayError {
    GatewayError::bad_request("Luma credential could not be prepared with normal secret admission")
        .with_code("luma_login_save_rejected")
}
