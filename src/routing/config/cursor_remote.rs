//! Private remote account records. Never expose secrets to generic HTTP executors.
use super::{ProviderConfigYaml, RouteConfigYaml};
use serde_json::json;
pub(crate) const AUTH_MODE: &str = "cursor_remote";
pub(crate) fn is_provider(p: &ProviderConfigYaml) -> bool {
    p.auth_mode.as_deref() == Some(AUTH_MODE)
}
pub(super) fn validate_document(doc: &RouteConfigYaml) -> anyhow::Result<()> {
    for p in &doc.providers {
        anyhow::ensure!(
            !p.auth_mode
                .as_deref()
                .is_some_and(|m| m.trim().eq_ignore_ascii_case(AUTH_MODE))
                || is_provider(p),
            "Cursor remote auth_mode must use its exact canonical value"
        );
        anyhow::ensure!(
            !p.credentials.iter().any(|c| c
                .auth_mode
                .as_deref()
                .is_some_and(|m| m.trim().eq_ignore_ascii_case(AUTH_MODE))),
            "Cursor remote auth_mode must be declared at provider level"
        );
        if !is_provider(p) {
            continue;
        }
        anyhow::ensure!(p.credentials.len() <= 64, "Cursor account limit exceeded");
        let credentials: Vec<_> = p
            .credentials
            .iter()
            .map(|c| {
                json!({
                    "id":c.id, "account_name":c.account_name, "enabled":c.enabled,
                    "auth_token":c.auth_token, "expires_at":c.expires_at,
                })
            })
            .collect();
        let allowed: ProviderConfigYaml = serde_json::from_value(json!({
            "id":p.id, "label":p.label, "vendor_key":p.vendor_key, "vendor_name":p.vendor_name,
            "base_url":"", "auth_mode":AUTH_MODE, "explicit_credentials_only":true,
            "credentials":credentials,
        }))?;
        anyhow::ensure!(
            serde_json::to_value(p)? == serde_json::to_value(allowed)?,
            "Cursor remote accounts forbid generic routes, transport and automation"
        );
        anyhow::ensure!(
            p.credentials
                .iter()
                .filter(|c| c.enabled == Some(true))
                .count()
                <= 1,
            "Only one Cursor remote account may be explicitly enabled per provider"
        );
        for c in &p.credentials {
            if c.enabled == Some(true) {
                anyhow::ensure!(
                    c.auth_token.is_some() && c.account_name.is_some(),
                    "Enabled Cursor remote account requires verified identity material"
                );
            }
            anyhow::ensure!(
                c.id.as_deref().is_some_and(valid_id),
                "Cursor account requires a stable ID"
            );
            if let Some(token) = &c.auth_token {
                let claims = crate::protocol::cursor_auth::claims(token)?;
                let expiry = c
                    .expires_at
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("Cursor expiry missing"))?;
                anyhow::ensure!(
                    time::OffsetDateTime::parse(
                        expiry,
                        &time::format_description::well_known::Rfc3339
                    )?
                    .unix_timestamp()
                        == claims.exp,
                    "Cursor expiry does not match token"
                );
            }
        }
        anyhow::ensure!(
            !doc.model_routes.iter().any(|r| r
                .provider_ids
                .iter()
                .any(|id| id.eq_ignore_ascii_case(&p.id))),
            "Cursor import-only accounts cannot appear in model routes"
        );
    }
    Ok(())
}
fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 256
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
}
