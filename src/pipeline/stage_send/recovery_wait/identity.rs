//! Refresh one refused account, not a new routing decision or quota reservation.
use crate::{
    error::GatewayError,
    pipeline::PipelineContext,
    provider_identity::ProviderAliases,
    routing::{
        candidate::RouteCandidate,
        config::{RouteAccountGroupConstraint, RouteConfigSnapshot},
    },
};

pub(super) async fn snapshot(
    state: &crate::state::AppState,
) -> Result<ProviderAliases, GatewayError> {
    match &state.local_runtime {
        Some(local) => local.provider_identity_snapshot().await,
        None => Ok(Default::default()),
    }
}

fn present(snapshot: &RouteConfigSnapshot, provider: &str) -> bool {
    snapshot
        .document()
        .providers
        .iter()
        .any(|p| p.id == provider)
}

fn explicit_account(snapshot: &RouteConfigSnapshot, provider: &str, account: Option<&str>) -> bool {
    account.is_some_and(|account| {
        snapshot.document().providers.iter().any(|p| {
            p.id == provider
                && p.credentials
                    .iter()
                    .any(|c| c.id.as_deref() == Some(account))
        })
    })
}

pub(super) struct Binding<'a> {
    pub(super) source: &'a RouteConfigSnapshot,
    pub(super) source_id: &'a str,
    pub(super) target: &'a str,
    pub(super) renamed: bool,
}

pub(super) fn bind<'a>(
    ctx: &'a PipelineContext,
    snapshot: &'a RouteConfigSnapshot,
    aliases: &'a ProviderAliases,
    candidate: &'a RouteCandidate,
) -> Result<Option<Binding<'a>>, GatewayError> {
    let denied = crate::access_key_groups::denied;
    let source = ctx
        .cash_route_snapshot
        .as_ref()
        .map(|source| {
            aliases
                .select_present(&candidate.provider_account_id, |id| present(source, id))
                .map(|id| id.map(|id| (source, id)))
        })
        .transpose()
        .map_err(|_| denied())?
        .flatten();
    let owned = candidate.provider_credential_id.is_none()
        && source.is_some_and(|(s, id)| {
            s.credential_dispatch_allowed(id, candidate.payload.credential_id.as_deref())
                .is_some()
        });
    if !owned {
        // Database/projected accounts require their own migration owner. A route
        // alias must not rewrite their provider IDs or retained quota projections.
        return if aliases
            .counterpart(&candidate.provider_account_id)
            .is_some()
        {
            Err(denied())
        } else {
            Ok(None)
        };
    }
    let target = aliases
        .select_present(&candidate.provider_account_id, |id| present(snapshot, id))
        .map_err(|_| denied())?
        .ok_or_else(denied)?;
    let (source, source_id) = source.unwrap();
    let renamed = source_id != target || target != candidate.provider_account_id;
    if renamed
        && (!explicit_account(
            source,
            source_id,
            candidate.payload.credential_id.as_deref(),
        ) || !explicit_account(snapshot, target, candidate.payload.credential_id.as_deref()))
    {
        // Never derive a new `provider::default` credential or match by model alone.
        return Err(denied());
    }
    Ok(Some(Binding {
        source,
        source_id,
        target,
        renamed,
    }))
}

pub(super) fn refresh(
    ctx: &PipelineContext,
    snapshot: &RouteConfigSnapshot,
    constraint: &RouteAccountGroupConstraint,
    key_groups: bool,
    aliases: &ProviderAliases,
    candidate: &RouteCandidate,
) -> Result<RouteCandidate, GatewayError> {
    let Some(Binding {
        target, renamed, ..
    }) = bind(ctx, snapshot, aliases, candidate)?
    else {
        return Ok(candidate.clone());
    };
    let default_id = format!("{}::default", candidate.provider_account_id);
    let resolution = snapshot.resolve_candidates_admitted(
        ctx.canonical_req.requested_model.as_deref(),
        constraint,
        &|provider, account, _| {
            provider == target
                && account
                    == candidate
                        .payload
                        .credential_id
                        .as_deref()
                        .unwrap_or(&default_id)
        },
        key_groups,
    );
    resolution
        .candidates
        .into_iter()
        .filter_map(|mut fresh| {
            crate::routing::protocol_resolution::finalize_candidate_protocol_family_with_override(
                &mut fresh,
                &ctx.canonical_req,
                ctx.forced_upstream_protocol.as_deref(),
            )?;
            Some(fresh)
        })
        .find(|fresh| {
            fresh.provider_account_id == target
                && fresh.payload.credential_id == candidate.payload.credential_id
                && fresh.upstream_model == candidate.upstream_model
                && fresh.adapter == candidate.adapter
                && (!renamed
                    || (fresh.protocol_profile == candidate.protocol_profile
                        && fresh.protocol_family == candidate.protocol_family))
        })
        .ok_or_else(crate::access_key_groups::denied)
}

#[cfg(test)]
mod tests;
