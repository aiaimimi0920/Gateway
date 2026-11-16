//! New authorization replaces all credential auth, including a missing refresh/expiry, through SecretPatch.
use super::{legacy, Binding, Session};
use crate::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    error::GatewayError,
    protocol::kiro_oauth::{AuthorizationContext, TokenMaterial},
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
    context: &AuthorizationContext,
    material: &TokenMaterial,
) -> Result<PreparedSave, GatewayError> {
    let snapshot = super::validate_target(state, binding, &session.machine)?;
    // Resolve the already-validated consumer, including credential-level profile overrides.
    let compiled = snapshot
        .get_providers()
        .into_iter()
        .find(|p| p.id == binding.provider)
        .ok_or_else(invalid)?;
    let payload = if let Some(id) = &binding.credential {
        &compiled
            .credential_pool
            .iter()
            .find(|c| &c.id == id)
            .ok_or_else(invalid)?
            .payload
    } else {
        &compiled.payload
    };
    let runtime_target = crate::protocol::kiro_oauth::runtime::owns_payload(payload)
        .then(|| crate::protocol::kiro_oauth::RuntimeTarget::from_payload(payload))
        .transpose()
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
        .unwrap_or_else(|| format!("kiro-{}", uuid::Uuid::new_v4()));
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
    legacy::normalize_device(
        &provider.extra_body,
        &mut target.extra_body,
        &session.machine,
    );
    target.refresh_endpoint = Some(context.token_endpoint());
    target.refresh_client_id = Some(context.client_id().into());
    target.expires_at = material
        .expires_at()
        .map(|expiry| expiry.format(&Rfc3339))
        .transpose()
        .map_err(|_| invalid())?;
    target.token_expires_in_secs = None;
    // A new account cannot inherit an old nested or provider-level session expiry.
    target.session_auth = Some(crate::credential_runtime::SessionAuthConfig {
        transport: "bearer".into(),
        primary_cookie_name: None,
        secondary_cookie_name: None,
        header_name: Some("authorization".into()),
        expires_at: target.expires_at.clone(),
    });
    let mut metadata = context.clone();
    if let Some(target) = runtime_target {
        metadata.bind_runtime(target);
    }
    let secret = metadata.take_client_secret().ok_or_else(invalid)?;
    target.kiro_oauth = Some(metadata);
    let prefix = format!("/providers/{pi}/credentials/{ci}");
    let paths = [
        format!("{prefix}/api_key"),
        format!("{prefix}/refresh_token"),
        format!("{prefix}/auth_token"),
        format!("{prefix}/kiro_oauth/client_secret"),
    ];
    let mut patches = Vec::new();
    let extra_prefix = format!("{prefix}/extra_body/");
    for descriptor in redacted.secrets {
        if paths.contains(&descriptor.path) {
            continue;
        }
        let retired = descriptor
            .path
            .strip_prefix(&extra_prefix)
            .is_some_and(|path| {
                let name = path
                    .split('/')
                    .next()
                    .unwrap()
                    .replace("~1", "/")
                    .replace("~0", "~");
                legacy::retired_extra(&name)
            });
        patches.push(if retired {
            SecretPatch::clear(descriptor.path)
        } else {
            SecretPatch::keep(descriptor.path)
        });
    }
    patches.push(SecretPatch::replace(
        &paths[0],
        json!(material.access_token()),
    ));
    patches.push(match material.refresh_token() {
        Some(value) => SecretPatch::replace(&paths[1], json!(value)),
        None => SecretPatch::clear(&paths[1]),
    });
    patches.push(SecretPatch::clear(&paths[2]));
    patches.push(SecretPatch::replace(&paths[3], json!(secret)));
    let mut document = resolve_secret_patches(snapshot.document(), redacted.document, &patches)
        .map_err(|_| invalid())?;
    // Retain redacted containers until the ordinary resolver clears every nested secret;
    // then retire only this credential's legacy copies before typed business validation.
    document.providers[pi].credentials[ci]
        .extra_body
        .retain(|key, _| !legacy::retired_extra(key));
    crate::console::document::validate_route_document(document.clone()).map_err(|_| invalid())?;
    Ok(PreparedSave {
        document,
        credential_id: id,
    })
}
fn invalid() -> GatewayError {
    GatewayError::bad_request("Kiro credential could not be prepared with normal secret admission")
        .with_code("kiro_login_save_rejected")
}
