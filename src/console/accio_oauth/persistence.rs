//! Server material becomes ordinary SecretPatches; never return a token-bearing draft to the UI.
use super::{Binding, Session};
use crate::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    error::GatewayError,
    protocol::accio_oauth::{TokenMaterial, REFRESH_ENDPOINT},
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
    material: &TokenMaterial,
) -> Result<PreparedSave, GatewayError> {
    let snapshot = super::validate_target(state, binding)?;
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
        .unwrap_or_else(|| format!("accio-{}", uuid::Uuid::new_v4()));
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
    // New authorization replaces authentication and old account context, not operator metadata.
    target.extra_body.retain(|key, _| !auth_extra(key));
    target.headers.retain(|key, _| {
        !auth_header(key)
            && !matches!(
                key.to_ascii_lowercase().as_str(),
                "utdid" | "x-utdid" | "cna" | "x-cna"
            )
    });
    target.headers.insert("utdid".into(), session.utdid.clone());
    target.refresh_endpoint = Some(REFRESH_ENDPOINT.into());
    target.refresh_client_id = Some(session.client_id.clone());
    target.expires_at = Some(
        material
            .expires_at()
            .format(&Rfc3339)
            .map_err(|_| invalid())?,
    );
    // No relative lifetime is proved by this wire. The original absolute instant is authoritative.
    target.token_expires_in_secs = None;
    let prefix = format!("/providers/{pi}/credentials/{ci}");
    let replaced = [
        format!("{prefix}/api_key"),
        format!("{prefix}/refresh_token"),
        format!("{prefix}/auth_token"),
        format!("{prefix}/accio_sgcookie"),
    ];
    let mut patches = Vec::new();
    for descriptor in redacted.secrets {
        if replaced.contains(&descriptor.path) {
            continue;
        }
        let clear = descriptor
            .path
            .strip_prefix(&format!("{prefix}/extra_body/"))
            .is_some_and(auth_extra)
            || descriptor
                .path
                .strip_prefix(&format!("{prefix}/headers/"))
                .is_some_and(auth_header);
        patches.push(if clear {
            SecretPatch::clear(descriptor.path)
        } else {
            SecretPatch::keep(descriptor.path)
        });
    }
    patches.push(SecretPatch::replace(
        &replaced[0],
        json!(material.access_token()),
    ));
    patches.push(SecretPatch::replace(
        &replaced[1],
        json!(material.refresh_token()),
    ));
    patches.push(SecretPatch::clear(&replaced[2]));
    // Unlike refresh, this is a new authorization with no verified same-account assertion.
    // An absent new SG cannot silently retain a possibly unrelated old account's SG.
    patches.push(match material.sgcookie() {
        Some(value) => SecretPatch::replace(&replaced[3], json!(value)),
        None => SecretPatch::clear(&replaced[3]),
    });
    let document = resolve_secret_patches(snapshot.document(), redacted.document, &patches)
        .map_err(|_| invalid())?;
    crate::console::document::validate_route_document(document.clone()).map_err(|_| invalid())?;
    Ok(PreparedSave {
        document,
        credential_id: id,
    })
}
pub(super) fn auth_extra(key: &str) -> bool {
    matches!(
        key,
        "token"
            | "accessToken"
            | "access_token"
            | "apiKey"
            | "api_key"
            | "refreshToken"
            | "refresh_token"
            | "sgcookie"
            | "accio_sgcookie"
            | "utdid"
            | "x-utdid"
            | "empid"
            | "empId"
            | "accountId"
            | "account_id"
            | "tenant"
            | "tenantId"
            | "tenant_id"
    )
}
pub(super) fn auth_header(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "cookie" | "authorization" | "proxy-authorization" | "x-api-key" | "api-key"
    )
}
fn invalid() -> GatewayError {
    GatewayError::bad_request("Accio credential could not be prepared with normal secret admission")
        .with_code("accio_login_save_rejected")
}
