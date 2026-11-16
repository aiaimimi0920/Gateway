//! Tokens use normal SecretPatch admission; reauthorization preserves operator metadata and identity.
use super::{session::Ready, Binding};
use crate::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    error::GatewayError,
    routing::config::{ProviderCredentialYaml, RouteConfigYaml},
    state::AppState,
};
use serde_json::json;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub(super) struct PreparedSave {
    pub document: RouteConfigYaml,
    pub credential_id: String,
    pub valid_until: tokio::time::Instant,
}
pub(super) fn prepare(
    state: &AppState,
    binding: &Binding,
    ready: &Ready,
) -> Result<PreparedSave, GatewayError> {
    let snapshot = super::validate_target(state, binding)?;
    if ready.receipt.is_expired() || ready.context.site() != binding.site {
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
        .unwrap_or_else(|| format!("dim-{}", uuid::Uuid::new_v4()));
    let ci = if binding.credential.is_some() {
        let ci = provider
            .credentials
            .iter()
            .position(|c| c.id.as_deref() == Some(id.as_str()))
            .ok_or_else(invalid)?;
        let original = snapshot.document().providers[pi].credentials[ci]
            .dim_oauth
            .as_ref()
            .ok_or_else(invalid)?;
        if original.subject() != ready.context.subject()
            || original.user_id() != ready.context.user_id()
        {
            return Err(GatewayError::bad_request(
                "DIM reauthorization must match the saved account identity",
            )
            .with_code("dim_login_identity_mismatch"));
        }
        ci
    } else {
        let credential: ProviderCredentialYaml =
            serde_json::from_value(json!({"id":id})).map_err(|_| invalid())?;
        provider.credentials.push(credential);
        provider.credentials.len() - 1
    };
    let target = &mut provider.credentials[ci];
    target.dim_oauth = Some(ready.context.clone());
    target.refresh_endpoint = Some(ready.context.token_endpoint().into());
    target.refresh_client_id = Some(ready.context.client_id().into());
    target.expires_at = Some(
        OffsetDateTime::from_unix_timestamp(
            i64::try_from(ready.receipt.expires_at_unix()).map_err(|_| invalid())?,
        )
        .map_err(|_| invalid())?
        .format(&Rfc3339)
        .map_err(|_| invalid())?,
    );
    // Keep the absolute expiry from the exchange, not a fresh lifetime from the save time.
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
    patches.push(SecretPatch::replace(
        &paths[0],
        json!(ready.receipt.access_token()),
    ));
    patches.push(SecretPatch::replace(
        &paths[1],
        json!(ready.receipt.refresh_token()),
    ));
    patches.push(SecretPatch::clear(&paths[2]));
    let document = resolve_secret_patches(snapshot.document(), redacted.document, &patches)
        .map_err(|_| invalid())?;
    crate::console::document::validate_route_document(document.clone()).map_err(|_| invalid())?;
    Ok(PreparedSave {
        document,
        credential_id: id,
        valid_until: tokio::time::Instant::now() + ready.receipt.remaining(),
    })
}
fn invalid() -> GatewayError {
    GatewayError::bad_request("DIM credential could not be prepared with normal secret admission")
        .with_code("dim_login_save_rejected")
}
