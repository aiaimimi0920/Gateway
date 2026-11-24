//! Authorize a receipt against live grants and the same effective SAP credential.
//! No Redis projection, balance admission, mutation claim, or network send happens here.
use super::{reference, Receipt, RoutePin, Store};
use crate::{
    auth::session::AuthenticatedSession,
    db::access::pinned_route::{self, PinnedRoute},
    error::GatewayError,
    routing::candidate::{ProviderAccountPayload, RouteCandidate},
    state::AppState,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub(super) fn key(state: &AppState) -> Option<[u8; 32]> {
    if state.pg_pool.is_some() {
        let secret = state.config.gateway_api_key_secret.as_deref()?;
        if secret.is_empty() {
            return None;
        }
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).ok()?;
        mac.update(b"neuro-gateway/sap-response-hosted/v1\0");
        Some(mac.finalize().into_bytes().into())
    } else {
        state.local_runtime.as_ref()?.sap_response_key().copied()
    }
}

pub(super) fn owner_tag(secret: &[u8; 32], session: &AuthenticatedSession) -> Option<String> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).ok()?;
    mac.update(b"neuro-gateway/sap-response-owner/v1\0");
    // Body-supplied user identifiers are not authentication subjects.
    for field in [
        session.tenant_id.as_str(),
        session.project_id.as_str(),
        session.access_key_id.as_deref()?,
    ] {
        if !reference(field) {
            return None;
        }
        mac.update(&(field.len() as u64).to_be_bytes());
        mac.update(field.as_bytes());
    }
    Some(hex::encode(mac.finalize().into_bytes()))
}

/// Point-in-time authorization; the dispatch owner must recheck after awaits
/// such as OAuth and must not substitute a different candidate on rejection.
pub(super) async fn read_authorized(
    state: &AppState,
    session: &AuthenticatedSession,
    id: &str,
    account_group: Option<&str>,
) -> Result<Option<(Receipt, ProviderAccountPayload)>, GatewayError> {
    let Some(secret) = key(state) else {
        return Ok(None);
    };
    let Some(owner) = owner_tag(&secret, session) else {
        return Ok(None);
    };
    let Some(store) = Store::from_state(state) else {
        return Ok(None);
    };
    let Some(receipt) = store.read(id, &owner).await? else {
        return Ok(None);
    };
    let Some(pin) = store.route_pin(&receipt).await? else {
        return Ok(None);
    };
    let target = read_target(
        state,
        session,
        &pin,
        &receipt.provider_ref,
        &receipt.credential_ref,
        account_group,
    )
    .await?;
    Ok(target.and_then(|(payload, _)| {
        pin.matches_binding(
            &pin,
            &receipt,
            &secret,
            &receipt.provider_ref,
            &receipt.credential_ref,
            &payload,
        )
        .then_some((receipt, payload))
    }))
}

pub(super) async fn read_target(
    state: &AppState,
    session: &AuthenticatedSession,
    pin: &RoutePin,
    provider: &str,
    credential: &str,
    account_group: Option<&str>,
) -> Result<Option<(ProviderAccountPayload, String)>, GatewayError> {
    Ok(
        read_candidate(state, session, pin, provider, credential, account_group)
            .await?
            .map(|(candidate, model)| (candidate.payload, model)),
    )
}

pub(super) async fn read_candidate(
    state: &AppState,
    session: &AuthenticatedSession,
    pin: &RoutePin,
    provider: &str,
    credential: &str,
    account_group: Option<&str>,
) -> Result<Option<(RouteCandidate, String)>, GatewayError> {
    let target = if let Some(pool) = &state.pg_pool {
        let Some(platform_access) = pin.platform_access_ref.as_deref() else {
            return Ok(None);
        };
        pinned_route::read_candidate(
            pool,
            session,
            &state.route_config,
            PinnedRoute {
                model: &pin.public_model,
                source_key: &pin.source_key_ref,
                platform_access,
                provider,
                credential,
                account_group,
            },
        )
        .await?
    } else {
        read_local(state, session, pin, provider, credential, account_group).await?
    };
    Ok(target.filter(|(candidate, model)| {
        super::super::deployment(&candidate.payload)
            .is_ok_and(|endpoint| endpoint.deployment == *model)
    }))
}

async fn read_local(
    state: &AppState,
    session: &AuthenticatedSession,
    pin: &RoutePin,
    provider: &str,
    credential: &str,
    account_group: Option<&str>,
) -> Result<Option<(RouteCandidate, String)>, GatewayError> {
    let Some(local) = &state.local_runtime else {
        return Ok(None);
    };
    let Some(key_id) = crate::local_runtime::access_keys::local_key_id(state, Some(session)) else {
        return Ok(None);
    };
    if pin.platform_access_ref.is_some() || pin.source_key_ref != key_id {
        return Ok(None);
    }
    let snapshot = state.route_config.snapshot();
    let Ok(mut groups) = snapshot.account_group_constraint(account_group) else {
        return Ok(None);
    };
    let exact = (provider.to_owned(), credential.to_owned());
    // The explicit preferred identity skips round-robin bookkeeping; the
    // admission predicate still excludes every other credential.
    let mut candidates = snapshot
        .resolve_candidates_admitted_with_preferred(
            Some(&pin.public_model),
            &groups,
            &|p, c, _| p == provider && c == credential,
            false,
            Some(&exact),
        )
        .candidates;
    candidates.retain(|candidate| {
        candidate.provider_credential_id.is_none()
            && candidate.provider_account_id == provider
            && candidate.payload.credential_id.as_deref() == Some(credential)
    });
    if candidates.len() != 1 {
        return Ok(None);
    }
    let (_, metadata) = local
        .authorize_local_candidates_with_metadata(key_id, Some(&pin.public_model), &mut candidates)
        .await?;
    if !crate::db::scope_list_from_metadata(metadata.as_ref())
        .iter()
        .any(|scope| scope == "relay" || scope == "responses")
    {
        return Ok(None);
    }
    if let Some(ids) = crate::access_key_groups::group_ids(metadata.as_ref())? {
        groups.intersect(&snapshot.access_key_group_constraint(&ids));
    }
    Ok(candidates.pop().and_then(|candidate| {
        (groups.allows_candidate(&candidate)
            && state.route_config.snapshot().revision() == snapshot.revision())
        .then(|| {
            let model = candidate
                .upstream_model
                .clone()
                .unwrap_or_else(|| pin.public_model.clone());
            (candidate, model)
        })
    }))
}

#[cfg(all(test, feature = "line-sap-aicore-chat"))]
mod tests;
