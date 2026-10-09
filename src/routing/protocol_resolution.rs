//! Protocol-family candidate selection and route policy matching.

mod model_policy;
mod request_compatibility;
mod surfaces;

#[cfg(test)]
mod tests;

pub use surfaces::{requested_wire_protocol_family, surface_supported_wire_protocol_families};

pub use model_policy::resolve_supported_wire_protocol_families_for_model;

use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::registry::{
    canonicalize_wire_protocol_family_key, protocol_family_selector_matches,
};
use crate::routing::candidate::RouteCandidate;

use self::request_compatibility::request_compatible_wire_protocol_families;

pub const SAME_PROTOCOL_FAMILY_PRIORITY_BONUS: i32 = 5;

pub fn finalize_candidate_protocol_family(
    candidate: &mut RouteCandidate,
    req: &CanonicalRelayRequest,
) -> Option<bool> {
    finalize_candidate_protocol_family_with_override(candidate, req, None)
}

pub fn finalize_candidate_protocol_family_with_override(
    candidate: &mut RouteCandidate,
    req: &CanonicalRelayRequest,
    forced: Option<&str>,
) -> Option<bool> {
    if !candidate.payload.discovered_protocols.is_empty() {
        return crate::provider_discovery::select_protocol_with_override(candidate, req, forced);
    }
    let supported = if candidate.supported_protocol_families.is_empty() {
        surface_supported_wire_protocol_families(&candidate.adapter, &candidate.protocol_family)
    } else {
        candidate
            .supported_protocol_families
            .iter()
            .map(|value| canonicalize_wire_protocol_family_key(value))
            .collect()
    };
    let request_compatible = request_compatible_wire_protocol_families(
        &candidate.payload,
        req.endpoint_kind,
        &candidate.protocol_family,
    );
    let compatible = request_compatible
        .into_iter()
        .filter(|family| supported.iter().any(|allowed| allowed == family))
        .collect::<Vec<_>>();
    if compatible.is_empty() {
        return None;
    }

    let requested = requested_wire_protocol_family(req);
    let default_family = canonicalize_wire_protocol_family_key(&candidate.protocol_family);
    let same_family = requested.as_ref().is_some_and(|family| {
        compatible.iter().any(|value| value == family)
            && forced.is_none_or(|target| target == family)
    });

    let selected = if let Some(target) = forced {
        if !compatible.iter().any(|family| family == target) {
            return None;
        }
        target.to_owned()
    } else if let Some(requested) = requested {
        if compatible.iter().any(|value| value == &requested) {
            requested
        } else if compatible.iter().any(|value| value == &default_family) {
            default_family
        } else {
            compatible[0].clone()
        }
    } else if compatible.iter().any(|value| value == &default_family) {
        default_family
    } else {
        compatible[0].clone()
    };

    candidate.protocol_family = selected;
    if same_family {
        candidate.priority = candidate
            .priority
            .saturating_add(SAME_PROTOCOL_FAMILY_PRIORITY_BONUS);
    }
    Some(same_family)
}

pub fn route_policy_family_matches_surface(
    allowed_family: &str,
    adapter: &str,
    protocol_family: &str,
) -> bool {
    surface_supported_wire_protocol_families(adapter, protocol_family)
        .iter()
        .any(|value| protocol_family_selector_matches(allowed_family, value))
}

pub fn route_policy_family_matches_candidate(
    allowed_family: &str,
    candidate_protocol_family: &str,
) -> bool {
    protocol_family_selector_matches(allowed_family, candidate_protocol_family)
}
