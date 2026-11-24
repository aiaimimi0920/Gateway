//! Account-group error mapping and aligned candidate/row filtering.

use super::*;

pub(super) fn key_group_candidates_error(
    snapshot: &crate::routing::config::RouteConfigSnapshot,
    constraint: &RouteAccountGroupConstraint,
    model: Option<&str>,
    rejected: &[crate::routing::config::RejectedCandidate],
) -> GatewayError {
    if snapshot.has_authorized_model(model, constraint) {
        let rejected_summary = build_rejected_summary(rejected);
        GatewayError::service_unavailable(format!(
            "Authorized upstream accounts are disabled or temporarily unavailable. {}",
            rejected_summary
        ))
        .with_code("authorized_candidates_unavailable")
    } else {
        crate::access_key_groups::denied()
    }
}

/// Build a compact human-readable summary of rejected candidates with their
/// provider account IDs, labels and rejection reasons. The summary is bounded
/// to avoid bloating error payloads.
pub(super) fn build_rejected_summary(
    rejected: &[crate::routing::config::RejectedCandidate],
) -> String {
    if rejected.is_empty() {
        return String::new();
    }
    let limit = 10;
    let entries: Vec<String> = rejected
        .iter()
        .take(limit)
        .map(|r| {
            let cred = r.real_credential_ref.as_deref().unwrap_or("<default>");
            format!(
                "{}({}) cred={} reason={}",
                r.provider_account_id, r.provider_label, cred, r.reason
            )
        })
        .collect();
    let overflow = if rejected.len() > limit {
        format!(" (+{} more)", rejected.len() - limit)
    } else {
        String::new()
    };
    format!("Rejected: [{}]{}", entries.join(", "), overflow)
}

pub(super) fn map_account_group_selection_error(
    error: crate::routing::config::RouteAccountGroupSelectionError,
) -> GatewayError {
    match error {
        crate::routing::config::RouteAccountGroupSelectionError::NotFound(group_id) => {
            GatewayError::bad_request(format!(
                "Requested account group '{}' was not found",
                group_id
            ))
            .with_code("account_group_not_found")
        }
        crate::routing::config::RouteAccountGroupSelectionError::Disabled(group_id) => {
            GatewayError::bad_request(format!(
                "Requested account group '{}' is disabled",
                group_id
            ))
            .with_code("account_group_disabled")
        }
    }
}

pub(super) fn account_group_candidates_unavailable_error(
    constraint: &RouteAccountGroupConstraint,
    model: Option<&str>,
) -> Option<GatewayError> {
    let account_group_id = constraint.requested_group_id()?;
    let model_label = model.unwrap_or("<none>");
    Some(
        GatewayError::bad_request(format!(
            "No providers/accounts configured for requested account group '{}' and model '{}'",
            account_group_id, model_label
        ))
        .with_code("account_group_candidates_unavailable"),
    )
}

pub(super) fn filter_candidate_pairs_by_account_group<T>(
    constraint: &RouteAccountGroupConstraint,
    candidates: Vec<RouteCandidate>,
    rows: Vec<T>,
) -> (Vec<RouteCandidate>, Vec<T>) {
    candidates
        .into_iter()
        .zip(rows)
        .filter(|(candidate, _)| constraint.allows_candidate(candidate))
        .unzip()
}
