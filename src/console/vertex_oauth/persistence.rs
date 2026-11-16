//! New Google material becomes normal SecretPatches; token-bearing drafts never leave the server.
use super::{session::Ready, Binding, Session};
use crate::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    error::GatewayError,
    protocol::vertex_oauth::TOKEN_ENDPOINT,
    routing::config::{ProviderCredentialYaml, RouteConfigYaml},
    state::AppState,
};
use serde_json::json;
use time::format_description::well_known::Rfc3339;
pub(super) struct PreparedSave {
    pub document: RouteConfigYaml,
    pub credential_id: String,
}
pub(super) fn prepare(
    state: &AppState,
    binding: &Binding,
    session: &Session,
    ready: &Ready,
) -> Result<PreparedSave, GatewayError> {
    let snapshot = super::validate_target(state, binding, &session.target)?;
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
        .unwrap_or_else(|| format!("vertex-{}", uuid::Uuid::new_v4()));
    let ci = if binding.credential.is_some() {
        provider
            .credentials
            .iter()
            .position(|c| c.id.as_deref() == Some(id.as_str()))
            .ok_or_else(invalid)?
    } else {
        let credential: ProviderCredentialYaml =
            serde_json::from_value(json!({"id":id})).map_err(|_| invalid())?;
        provider.credentials.push(credential);
        provider.credentials.len() - 1
    };
    let target = &mut provider.credentials[ci];
    let mut context = ready.context.clone();
    let secret = context.take_client_secret();
    target.vertex_oauth = Some(context);
    target.refresh_endpoint = Some(TOKEN_ENDPOINT.into());
    target.refresh_client_id = Some(ready.context.client_id().into());
    target.expires_at = Some(
        ready
            .material
            .expires_at()
            .format(&Rfc3339)
            .map_err(|_| invalid())?,
    );
    target.token_expires_in_secs = Some(ready.material.expires_in());
    let prefix = format!("/providers/{pi}/credentials/{ci}");
    let paths = [
        format!("{prefix}/api_key"),
        format!("{prefix}/refresh_token"),
        format!("{prefix}/auth_token"),
        format!("{prefix}/vertex_oauth/client_secret"),
    ];
    let mut patches: Vec<_> = redacted
        .secrets
        .into_iter()
        .filter(|d| !paths.contains(&d.path))
        .map(|d| SecretPatch::keep(d.path))
        .collect();
    patches.push(SecretPatch::replace(
        &paths[0],
        json!(ready.material.access_token()),
    ));
    patches.push(SecretPatch::replace(
        &paths[1],
        json!(ready.material.refresh_token().ok_or_else(invalid)?),
    ));
    patches.push(SecretPatch::clear(&paths[2]));
    patches.push(match secret {
        Some(value) => SecretPatch::replace(&paths[3], json!(value)),
        None => SecretPatch::clear(&paths[3]),
    });
    let document = resolve_secret_patches(snapshot.document(), redacted.document, &patches)
        .map_err(|_| invalid())?;
    crate::console::document::validate_route_document(document.clone()).map_err(|_| invalid())?;
    Ok(PreparedSave {
        document,
        credential_id: id,
    })
}
fn invalid() -> GatewayError {
    GatewayError::bad_request(
        "Vertex credential could not be prepared with normal secret admission",
    )
    .with_code("vertex_login_save_rejected")
}
