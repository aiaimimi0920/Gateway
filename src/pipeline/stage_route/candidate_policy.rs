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
    let allowed_protocol_families = route_policy
        .and_then(|policy| policy.allowed_protocol_families.as_ref())
        .cloned();
    let Some(allowed_protocol_families) = allowed_protocol_families else {
        return candidates;
    };
    candidates
        .into_iter()
        .filter(|candidate| {
            allowed_protocol_families.iter().any(|allowed| {
                route_policy_family_matches_candidate(allowed, &candidate.protocol_family)
            })
        })
        .collect()
}
