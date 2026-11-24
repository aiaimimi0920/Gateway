//! Secret-patch preparation for saving an authorized Amazon Quick OAuth credential.

use super::Binding;
use crate::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    error::GatewayError,
    protocol::quick_oauth::{valid_token, AuthorizedSession, AUTH_MODE},
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
    ready: &AuthorizedSession,
    valid_until: tokio::time::Instant,
) -> Result<PreparedSave, GatewayError> {
    let snapshot = super::validate_target(state, binding)?;
    if tokio::time::Instant::now() >= valid_until {
        return Err(invalid());
    }
    ready.context.validate().map_err(|_| invalid())?;
    if !valid_token(&ready.bearer_token) || !valid_token(&ready.refresh_token) {
        return Err(invalid());
    }

    let mut redacted_context = ready.context.clone();
    let client_secret = redacted_context.take_client_secret().ok_or_else(invalid)?;
    let token_endpoint = redacted_context.token_endpoint();
    let client_id = redacted_context.client_id().to_owned();
    let suggested_name = redacted_context
        .email()
        .or_else(|| redacted_context.display_name())
        .or_else(|| redacted_context.account_name())
        .map(str::to_owned);

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
        .unwrap_or_else(|| format!("quick-{}", uuid::Uuid::new_v4()));
    let ci = if binding.credential.is_some() {
        provider
            .credentials
            .iter()
            .position(|c| c.id.as_deref() == Some(id.as_str()))
            .ok_or_else(invalid)?
    } else {
        let mut credential: ProviderCredentialYaml =
            serde_json::from_value(json!({ "id": id })).map_err(|_| invalid())?;
        credential.account_name = suggested_name;
        provider.credentials.push(credential);
        provider.credentials.len() - 1
    };

    let target = &mut provider.credentials[ci];
    target.auth_mode = Some(AUTH_MODE.into());
    target.quick_oauth = Some(redacted_context);
    target.refresh_endpoint = Some(token_endpoint);
    target.refresh_client_id = Some(client_id);
    target.token_expires_in_secs = Some(ready.expires_in_secs);
    target.expires_at = Some(ready.expires_at.clone());

    let prefix = format!("/providers/{pi}/credentials/{ci}");
    let paths = [
        format!("{prefix}/api_key"),
        format!("{prefix}/refresh_token"),
        format!("{prefix}/quick_oauth/client_secret"),
        format!("{prefix}/auth_token"),
    ];
    let mut patches: Vec<_> = redacted
        .secrets
        .into_iter()
        .filter(|d| !paths.contains(&d.path))
        .map(|d| SecretPatch::keep(d.path))
        .collect();
    patches.push(SecretPatch::replace(&paths[0], json!(ready.bearer_token)));
    patches.push(SecretPatch::replace(&paths[1], json!(ready.refresh_token)));
    patches.push(SecretPatch::replace(&paths[2], json!(client_secret)));
    patches.push(SecretPatch::clear(&paths[3]));

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
        "Amazon Quick credential could not be prepared with normal secret admission",
    )
    .with_code("quick_oauth_login_save_rejected")
}
