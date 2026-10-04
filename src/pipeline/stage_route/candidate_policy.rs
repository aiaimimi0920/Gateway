//! Protocol-family finalization and route-policy candidate admission.

use super::*;

pub(super) fn finalize_candidate_pairs_for_request<T: Clone>(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    candidates: Vec<RouteCandidate>,
    projected_rows: Option<Vec<T>>,
) -> (Vec<RouteCandidate>, Option<Vec<T>>) {
    let mut same_family = Vec::new();
    let mut others = Vec::new();
    match projected_rows {
        Some(rows) => {
            for (mut candidate, row) in candidates.into_iter().zip(rows.into_iter()) {
                let Some(is_same_family) = finalize_candidate_protocol_family(&mut candidate, req)
                else {
                    continue;
                };
                if is_same_family {
                    same_family.push((candidate, row));
                } else {
                    others.push((candidate, row));
                }
            }
            same_family.extend(others);
            let (candidates, rows): (Vec<_>, Vec<_>) = same_family.into_iter().unzip();
            (candidates, Some(rows))
        }
        None => {
            let mut same_only = Vec::new();
            let mut other_only = Vec::new();
            for mut candidate in candidates {
                let Some(is_same_family) = finalize_candidate_protocol_family(&mut candidate, req)
                else {
                    continue;
                };
                if is_same_family {
                    same_only.push(candidate);
                } else {
                    other_only.push(candidate);
                }
            }
            same_only.extend(other_only);
            (same_only, None)
        }
    }
}

pub(super) fn filter_candidates_by_route_policy_family(
    candidates: Vec<RouteCandidate>,
    route_policy: Option<&crate::db::GatewayRoutePolicyConfig>,
) -> Vec<RouteCandidate> {
    let allowed_protocol_families =
        route_policy.and_then(|policy| policy.allowed_protocol_families.as_ref());
    candidates
        .into_iter()
        .filter(|candidate| {
            provider_allowed_by_policy(&candidate.provider_account_id, route_policy)
                && allowed_protocol_families.is_none_or(|families| {
                    families.iter().any(|allowed| {
                        route_policy_family_matches_candidate(allowed, &candidate.protocol_family)
                    })
                })
        })
        .collect()
}

/// Revalidate the merged Redis/YAML/DB queue using exact account IDs, not brands,
/// models, adapter names or guessed paid status. Empty lists retain DB semantics.
pub(crate) fn provider_allowed_by_policy(
    provider_id: &str,
    policy: Option<&crate::db::GatewayRoutePolicyConfig>,
) -> bool {
    let Some(ids) = policy.and_then(|policy| policy.allowed_provider_account_ids.as_ref()) else {
        return true;
    };
    ids.iter().all(|id| id.trim().is_empty()) || ids.iter().any(|id| id.trim() == provider_id)
}

/// Unconstrained initial selection is not authorization to replay on another account.
pub(crate) fn provider_explicitly_allowed_by_policy(
    provider_id: &str,
    policy: Option<&crate::db::GatewayRoutePolicyConfig>,
) -> bool {
    policy
        .and_then(|policy| policy.allowed_provider_account_ids.as_ref())
        .is_some_and(|ids| {
            ids.iter()
                .any(|id| !id.trim().is_empty() && id.trim() == provider_id)
        })
}
