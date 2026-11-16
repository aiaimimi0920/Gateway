//! Tokens use normal SecretPatch admission; reauthorization preserves operator metadata and identity.
use super::Binding;
use crate::protocol::lobsterai::{SessionMaterial, CLIENT_ATTRIBUTION, TOKEN_ENDPOINT};
use crate::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    error::GatewayError,
    routing::config::{ProviderCredentialYaml, RouteConfigYaml},
    state::AppState,
};
use serde_json::json;

pub(super) struct PreparedSave {
    pub document: RouteConfigYaml,
    pub credential_id: String,
    pub valid_until: tokio::time::Instant,
}
pub(super) fn prepare(
    state: &AppState,
    binding: &Binding,
    ready: &SessionMaterial,
    valid_until: tokio::time::Instant,
) -> Result<PreparedSave, GatewayError> {
    let snapshot = super::validate_target(state, binding)?;
    if tokio::time::Instant::now() >= valid_until {
        return Err(invalid());
    }
    ready
        .context
        .validate_binding(&ready.access_token, &ready.refresh_token)
        .map_err(|_| invalid())?;
    let mut context = ready.context.clone();
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
        .unwrap_or_else(|| format!("lobsterai-{}", uuid::Uuid::new_v4()));
    let ci = if binding.credential.is_some() {
        let ci = provider
            .credentials
            .iter()
            .position(|c| c.id.as_deref() == Some(id.as_str()))
            .ok_or_else(invalid)?;
        let original = snapshot.document().providers[pi].credentials[ci]
            .lobsterai
            .as_ref()
            .ok_or_else(invalid)?;
        context = original.reauthorization(&context).map_err(|_| {
            GatewayError::bad_request(
                "LobsterAI reauthorization must match the saved account identity",
            )
            .with_code("lobsterai_login_identity_mismatch")
        })?;
        ci
    } else {
        let credential: ProviderCredentialYaml =
            serde_json::from_value(json!({"id":id})).map_err(|_| invalid())?;
        provider.credentials.push(credential);
        provider.credentials.len() - 1
    };
    let target = &mut provider.credentials[ci];
    target.lobsterai = Some(context);
    target.refresh_endpoint = Some(TOKEN_ENDPOINT.into());
    target.refresh_client_id = Some(CLIENT_ATTRIBUTION.into());
    target.expires_at = None;
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
        valid_until,
    })
}
fn invalid() -> GatewayError {
    GatewayError::bad_request(
        "LobsterAI credential could not be prepared with normal secret admission",
    )
    .with_code("lobsterai_login_save_rejected")
}
