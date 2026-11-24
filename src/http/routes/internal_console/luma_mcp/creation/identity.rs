//! Recover task access from archived credentials and published refresh evidence, never client IDs alone.
use super::{super::Input, GatewayError};
use crate::{
    console::{
        refresh_grants::{GrantPhase, GrantRequest},
        RouteConfigRedisRevision, RouteConfigRuntime,
    },
    routing::config::{ProviderCredentialYaml, RouteConfigYaml},
};
use std::collections::HashSet;

pub(super) async fn verify(
    runtime: Option<&RouteConfigRuntime>,
    original_revision: &str,
    target: &Input,
) -> Result<(), GatewayError> {
    if original_revision == target.expected_revision {
        return Ok(());
    }
    let runtime = runtime.ok_or_else(unavailable)?;
    let owner = runtime.coordinator().cloned().ok_or_else(unavailable)?;
    let revision = original_revision.to_owned();
    // Archives contain secrets; keep them in memory and off the async executor's file-I/O path.
    let archive = tokio::task::spawn_blocking(move || owner.load_revision(&revision))
        .await
        .map_err(|_| unavailable())?
        .map_err(|_| unavailable())?
        .ok_or_else(unavailable)?;
    let original =
        RouteConfigRedisRevision::new(archive.metadata().clone(), archive.document().clone())
            .map_err(|_| unavailable())?;
    let current = runtime.route_config().snapshot();
    if current.revision().id() != target.expected_revision {
        return Err(super::revision_changed());
    }
    let old = credential(original.document(), target)?;
    let new = credential(current.document(), target)?;
    // Policy/name edits do not change the account. Include the actual secrets and full OAuth context.
    if old.api_key == new.api_key
        && old.refresh_token == new.refresh_token
        && old.luma_oauth == new.luma_oauth
    {
        return Ok(());
    }
    let current =
        RouteConfigRedisRevision::new(current.revision().clone(), current.document().clone())
            .map_err(|_| unavailable())?;
    let mut prior =
        GrantRequest::from_revision(&original, &target.provider_id, &target.credential_id)
            .map_err(|_| unavailable())?;
    let next = GrantRequest::from_revision(&current, &target.provider_id, &target.credential_id)
        .map_err(|_| unavailable())?;
    let mut visited = HashSet::new();
    // Follow at most 32 persisted rotations; no refresh request, token write or background retry.
    for _ in 0..32 {
        if !visited.insert(prior.material_id()) {
            return Err(unavailable());
        }
        let record = runtime
            .find_refresh_grant(&prior)
            .await
            .map_err(|_| unavailable())?
            .ok_or_else(unavailable)?;
        if record.phase() != GrantPhase::Published
            || record.provider_id() != target.provider_id
            || record.credential_id() != target.credential_id
            || !record.original().equivalent(&prior.material)
        {
            return Err(unavailable());
        }
        let result = record.result().ok_or_else(unavailable)?;
        if result.material.equivalent(&next.material) {
            return Ok(());
        }
        prior.material = result.material.clone();
    }
    Err(unavailable())
}

fn credential<'a>(
    document: &'a RouteConfigYaml,
    target: &Input,
) -> Result<&'a ProviderCredentialYaml, GatewayError> {
    document
        .providers
        .iter()
        .find(|p| p.id == target.provider_id && crate::routing::config::luma_mcp::is_provider(p))
        .and_then(|p| {
            p.credentials
                .iter()
                .find(|c| c.id.as_deref() == Some(target.credential_id.as_str()))
        })
        .filter(|c| c.luma_oauth.is_some() && c.api_key.as_ref().is_some_and(|key| !key.is_empty()))
        .ok_or_else(unavailable)
}

fn unavailable() -> GatewayError {
    GatewayError::conflict(
        "The original Luma account continuity could not be verified; keep the task ID.",
    )
    .with_code("luma_mcp_task_identity_unverified")
}
