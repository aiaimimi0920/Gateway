//! Explicit save uses ordinary secret admission and preserves stable IDs and operator metadata.
use super::Binding;
use crate::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    error::GatewayError,
    protocol::nous_portal::{AuthorizedTokens, TOKEN_ENDPOINT},
    routing::config::{ProviderCredentialYaml, RouteConfigYaml},
    state::AppState,
};
use serde_json::json;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub(super) struct PreparedSave {
    pub document: RouteConfigYaml,
    pub credential_id: String,
    pub expires_at_unix: u64,
}
pub(super) fn prepare(
    state: &AppState,
    binding: &Binding,
    ready: &AuthorizedTokens,
) -> Result<PreparedSave, GatewayError> {
    let snapshot = super::validate_target(state, binding)?;
    ready
        .context
        .validate_access(&ready.access_token)
        .map_err(|_| invalid())?;
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
        .unwrap_or_else(|| format!("nous-{}", uuid::Uuid::new_v4()));
    let ci = if binding.credential.is_some() {
        let ci = provider
            .credentials
            .iter()
            .position(|c| c.id.as_ref() == Some(&id))
            .ok_or_else(invalid)?;
        let original = snapshot.document().providers[pi].credentials[ci]
            .nous_portal
            .as_ref()
            .ok_or_else(invalid)?;
        // Independent grants do not prove refresh-chain continuity. Unknown subjects never authorize replacement.
        if !original.same_known_subject_hint(&ready.context) {
            return Err(GatewayError::bad_request(
                "Nous reauthorization must match the saved known subject and client.",
            )
            .with_code("nous_login_identity_mismatch"));
        }
        ci
    } else {
        let credential: ProviderCredentialYaml =
            serde_json::from_value(json!({"id":id})).map_err(|_| invalid())?;
        provider.credentials.push(credential);
        provider.credentials.len() - 1
    };
    let target = &mut provider.credentials[ci];
    target.nous_portal = Some(ready.context.clone());
    target.base_url = Some(ready.context.inference_target().base_url().into());
    target.refresh_endpoint = Some(TOKEN_ENDPOINT.into());
    target.refresh_client_id = Some(ready.context.client_id().into());
    target.expires_at = Some(
        OffsetDateTime::from_unix_timestamp(ready.expires_at_unix as i64)
            .map_err(|_| invalid())?
            .format(&Rfc3339)
            .map_err(|_| invalid())?,
    );
    target.token_expires_in_secs = None;
    let prefix = format!("/providers/{pi}/credentials/{ci}");
    let paths = [
        format!("{prefix}/api_key"),
        format!("{prefix}/refresh_token"),
        format!("{prefix}/auth_token"),
    ];
    let mut patches: Vec<_> = redacted
        .secrets
        .into_iter()
        .filter(|d| !paths.contains(&d.path))
        .map(|d| SecretPatch::keep(d.path))
        .collect();
    patches.push(SecretPatch::replace(&paths[0], json!(ready.access_token)));
    patches.push(SecretPatch::replace(&paths[1], json!(ready.refresh_token)));
    patches.push(SecretPatch::clear(&paths[2]));
    let document = resolve_secret_patches(snapshot.document(), redacted.document, &patches)
        .map_err(|_| invalid())?;
    crate::console::document::validate_route_document(document.clone()).map_err(|_| invalid())?;
    Ok(PreparedSave {
        document,
        credential_id: id,
        expires_at_unix: ready.expires_at_unix,
    })
}
fn invalid() -> GatewayError {
    GatewayError::bad_request("Nous credential could not be prepared with normal secret admission")
        .with_code("nous_login_save_rejected")
}
