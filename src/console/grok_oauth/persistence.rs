//! Turn a server-owned receipt into an ordinary redacted draft and explicit secret patches.
use super::{Binding, LoginError};
use crate::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    error::GatewayError,
    protocol::grok_oauth::{business, AuthorizationContext, GrantReceipt},
    routing::config::{ProviderCredentialYaml, RouteConfigSnapshot, RouteConfigYaml},
    state::AppState,
};
use serde_json::json;
use std::sync::Arc;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub(super) struct PreparedSave {
    pub document: RouteConfigYaml,
    pub credential_id: String,
}
pub(crate) fn validate_target(
    state: &AppState,
    binding: &Binding,
) -> Result<Arc<RouteConfigSnapshot>, GatewayError> {
    if state.shutdown.is_requested() || state.lifecycle.is_draining() {
        return Err(super::login_error(LoginError::Stopped));
    }
    if state
        .route_config_runtime
        .as_ref()
        .is_none_or(|r| r.coordinator().is_none())
    {
        return Err(
            GatewayError::service_unavailable("Writable console runtime is required")
                .with_code("console_mutation_not_supported"),
        );
    }
    let snapshot = state.route_config.snapshot();
    if snapshot.revision().id() != binding.revision() {
        return Err(GatewayError::conflict(
            "Grok login target revision changed; restart authorization",
        )
        .with_code("console_revision_conflict"));
    }
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == binding.provider())
        .filter(|p| {
            p.preset.as_deref() == Some(business::PRESET_ID) && business::valid_base(&p.base_url)
        })
        .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
    if binding.credential.as_ref().is_some_and(|id| {
        !provider
            .credentials
            .iter()
            .any(|c| c.id.as_ref() == Some(id))
    }) {
        return Err(super::login_error(LoginError::Unavailable));
    }
    Ok(snapshot)
}
pub(super) fn prepare(
    state: &AppState,
    binding: &Binding,
    receipt: &GrantReceipt,
) -> Result<PreparedSave, GatewayError> {
    let snapshot = validate_target(state, binding)?;
    let context =
        AuthorizationContext::from_receipt(receipt).map_err(|e| super::login_error(e.into()))?;
    let mut redacted = redact_route_document(snapshot.document()).map_err(|_| invalid())?;
    let pi = redacted
        .document
        .providers
        .iter()
        .position(|p| p.id == binding.provider())
        .ok_or_else(invalid)?;
    let provider = &mut redacted.document.providers[pi];
    let id = binding
        .credential
        .clone()
        .unwrap_or_else(|| format!("grok-{}", uuid::Uuid::new_v4()));
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
    target.grok_oauth = Some(context);
    target.expires_at = receipt
        .expires_at_unix_secs()
        .map(|seconds| {
            let seconds = i64::try_from(seconds).map_err(|_| invalid())?;
            OffsetDateTime::from_unix_timestamp(seconds)
                .map_err(|_| invalid())?
                .format(&Rfc3339)
                .map_err(|_| invalid())
        })
        .transpose()?;
    target.refresh_endpoint = None;
    target.refresh_client_id = None;
    target.token_expires_in_secs = receipt.tokens().expires_in;
    let prefix = format!("/providers/{pi}/credentials/{ci}");
    let replaced = [
        format!("{prefix}/api_key"),
        format!("{prefix}/refresh_token"),
        format!("{prefix}/auth_token"),
    ];
    let mut patches: Vec<_> = redacted
        .secrets
        .into_iter()
        .filter(|descriptor| !replaced.contains(&descriptor.path))
        .map(|descriptor| SecretPatch::keep(descriptor.path))
        .collect();
    patches.push(SecretPatch::replace(
        &replaced[0],
        json!(receipt.tokens().access_token),
    ));
    patches.push(match &receipt.tokens().refresh_token {
        Some(refresh) => SecretPatch::replace(&replaced[1], json!(refresh)),
        None => SecretPatch::clear(&replaced[1]),
    });
    patches.push(SecretPatch::clear(&replaced[2]));
    let document = resolve_secret_patches(snapshot.document(), redacted.document, &patches)
        .map_err(|_| invalid())?;
    // Compile before consuming the one-shot receipt; normal commit still revalidates and CASes.
    crate::console::document::validate_route_document(document.clone()).map_err(|_| invalid())?;
    Ok(PreparedSave {
        document,
        credential_id: id,
    })
}
fn invalid() -> GatewayError {
    GatewayError::bad_request("Grok credential could not be prepared with normal secret admission")
        .with_code("grok_login_save_rejected")
}
