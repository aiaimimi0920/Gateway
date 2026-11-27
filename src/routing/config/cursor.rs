//! Reserved Cursor model guards. The ordinary HTTP router, provider pools
//! and model graph must never dispatch reserved Cursor models, and legacy
//! `cursor_acp` declarations fail closed so they cannot load as HTTP pools.

use super::RouteConfigYaml;
use crate::error::{FallbackHint, GatewayError};

const LEGACY_ACP_AUTH_MODE: &str = "cursor_acp";
pub(crate) const MODEL: &str = "cursor-personal";
pub(crate) mod local_auto;

pub(super) fn validate_document(doc: &RouteConfigYaml) -> anyhow::Result<()> {
    reject_legacy_acp(doc)?;
    reject_reserved_models(doc)?;
    Ok(())
}

fn reject_legacy_acp(doc: &RouteConfigYaml) -> anyhow::Result<()> {
    anyhow::ensure!(
        !doc.providers.iter().any(|p| {
            p.auth_mode
                .as_deref()
                .is_some_and(|mode| mode.trim().eq_ignore_ascii_case(LEGACY_ACP_AUTH_MODE))
                || p.credentials.iter().any(|c| {
                    c.auth_mode
                        .as_deref()
                        .is_some_and(|mode| mode.trim().eq_ignore_ascii_case(LEGACY_ACP_AUTH_MODE))
                })
        }),
        "Legacy Cursor ACP declarations are unsupported; use cursor_remote instead"
    );
    Ok(())
}

// The private model remains reserved before import and after account removal.
// Otherwise an alias or wildcard could silently spend a different account.
fn reject_reserved_models(doc: &RouteConfigYaml) -> anyhow::Result<()> {
    let is_model = |s: &str| is_model(s) || local_auto::reserved(s);
    anyhow::ensure!(
        !doc.model_routes
            .iter()
            .any(|route| is_model(&route.pattern))
            && !doc
                .aliases
                .iter()
                .any(|(alias, target)| is_model(alias) || is_model(target)),
        "The private Cursor model cannot appear in ordinary model routes or aliases"
    );
    anyhow::ensure!(
        !doc.providers.iter().any(|q| {
            q.supported_models.iter().any(|s| is_model(s))
                || q.default_model.as_deref().is_some_and(is_model)
                || q.model_map.keys().any(|s| is_model(s))
                || q.model_map_targets.keys().any(|s| is_model(s))
                || q.model_map.values().any(|s| local_auto::reserved(s))
                || q.model_map_targets
                    .values()
                    .flatten()
                    .any(|s| local_auto::reserved(s))
                || q.credentials.iter().any(|c| {
                    c.supported_models.iter().any(|s| is_model(s))
                        || c.discovery
                            .as_ref()
                            .is_some_and(|d| d.models.iter().any(|s| is_model(s)))
                        || c.model_catalog
                            .as_ref()
                            .is_some_and(|d| d.models.iter().any(|s| is_model(s)))
                        || c.model_selection.as_ref().is_some_and(|selection| {
                            selection
                                .manual
                                .iter()
                                .chain(&selection.fetched)
                                .any(|s| is_model(s))
                        })
                })
        }),
        "The private Cursor ACP model cannot be declared by an ordinary provider"
    );
    Ok(())
}

fn is_model(model: &str) -> bool {
    model.trim().eq_ignore_ascii_case(MODEL)
}

pub(crate) fn private_request(model: Option<&str>) -> bool {
    // Recognition cannot depend on a route snapshot or account presence.
    // The private executor separately requires its frozen authorization snapshot.
    model.is_some_and(is_model)
}

/// Until a qualified private account is bound, do not allow the literal
/// private model to spill into a wildcard, a retry, or a managed provider.
pub(crate) fn reject_unavailable_model(model: Option<&str>) -> Result<(), GatewayError> {
    if !private_request(model) {
        return Ok(());
    }
    Err(unavailable())
}

pub(crate) fn unavailable() -> GatewayError {
    stop(
        GatewayError::service_unavailable("Private Cursor runtime is unavailable")
            .with_code("cursor_acp_execution_unavailable"),
    )
}

pub(crate) fn stop(mut error: GatewayError) -> GatewayError {
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: "Private Cursor model cannot be substituted by another provider".into(),
    };
    error
}

#[cfg(test)]
mod tests;
