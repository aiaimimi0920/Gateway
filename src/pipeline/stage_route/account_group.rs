//! Account-group error mapping and aligned candidate/row filtering.

use super::*;

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
