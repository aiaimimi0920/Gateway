//! Browser-returned material is server-owned; normal SecretPatch and revision CAS are the only save path.
use super::{Binding, Session};
use crate::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    error::GatewayError,
    protocol::freebuff_login::{Credentials, SessionContext},
    routing::config::{ProviderCredentialYaml, RouteConfigYaml},
    state::AppState,
};
use serde_json::json;

pub(super) struct PreparedSave {
    pub document: RouteConfigYaml,
    pub credential_id: String,
}
pub(super) fn prepare(
    state: &AppState,
    binding: &Binding,
    session: &Session,
    material: &Credentials,
) -> Result<PreparedSave, GatewayError> {
    let snapshot = super::validate_target(state, binding, &session.fingerprint)?;
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
        .unwrap_or_else(|| format!("freebuff-{}", uuid::Uuid::new_v4()));
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
    // Approval expiry is not token expiry; never invent an absolute token lifetime or refresh owner.
    target.expires_at = None;
    target.token_expires_in_secs = None;
    target.session_auth = None;
    target.freebuff_session = Some(SessionContext::default());
    let prefix = format!("/providers/{pi}/credentials/{ci}");
    let paths = [
        format!("{prefix}/api_key"),
        format!("{prefix}/auth_token"),
        format!("{prefix}/freebuff_session/fingerprint_id"),
        format!("{prefix}/freebuff_session/fingerprint_hash"),
    ];
    let mut patches: Vec<_> = redacted
        .secrets
        .into_iter()
        .filter(|d| !paths.contains(&d.path))
        .map(|d| SecretPatch::keep(d.path))
        .collect();
    patches.push(SecretPatch::replace(
        &paths[0],
        json!(material.auth_token()),
    ));
    patches.push(SecretPatch::clear(&paths[1]));
    patches.push(SecretPatch::replace(
        &paths[2],
        json!(material.fingerprint_id()),
    ));
    patches.push(SecretPatch::replace(
        &paths[3],
        json!(material.fingerprint_hash()),
    ));
    let document = resolve_secret_patches(snapshot.document(), redacted.document, &patches)
        .map_err(|_| invalid())?;
    crate::console::document::validate_route_document(document.clone()).map_err(|_| invalid())?;
    Ok(PreparedSave {
        document,
        credential_id: id,
    })
}
fn invalid() -> GatewayError {
    GatewayError::bad_request("FreeBuff session could not be prepared with normal secret admission")
        .with_code("freebuff_login_save_rejected")
}
