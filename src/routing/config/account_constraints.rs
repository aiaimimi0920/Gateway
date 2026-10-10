//! Validated account-group membership and candidate identity.

use super::*;

fn account_group_allowed_accounts(
    groups: &[AccountGroupYaml],
    account_group_id: Option<&str>,
) -> Result<Option<HashSet<String>>, RouteAccountGroupSelectionError> {
    let Some(requested_group_id) = account_group_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    let Some(group) = groups
        .iter()
        .find(|group| group.id.trim() == requested_group_id)
    else {
        return Err(RouteAccountGroupSelectionError::NotFound(
            requested_group_id.to_string(),
        ));
    };

    if !group.enabled.unwrap_or(true) {
        return Err(RouteAccountGroupSelectionError::Disabled(
            requested_group_id.to_string(),
        ));
    }

    let allowed_accounts = group
        .provider_credential_ids
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<HashSet<_>>();

    Ok(Some(allowed_accounts))
}

pub(super) fn build_account_group_constraint(
    groups: &[AccountGroupYaml],
    account_group_id: Option<&str>,
) -> Result<RouteAccountGroupConstraint, RouteAccountGroupSelectionError> {
    let requested_group_id = account_group_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let allowed_account_ids = account_group_allowed_accounts(groups, account_group_id)?;
    Ok(RouteAccountGroupConstraint {
        requested_group_id,
        allowed_account_ids,
    })
}

fn route_candidate_account_id(candidate: &RouteCandidate) -> String {
    candidate
        .provider_credential_id
        .as_deref()
        .or(candidate.payload.credential_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| provider_default_account_id(&candidate.provider_account_id))
}

#[cfg(test)]
pub(super) fn candidate_account_group_member_id(candidate: &RouteCandidate) -> String {
    route_candidate_account_id(candidate)
}

impl RouteAccountGroupConstraint {
    pub fn intersect(&mut self, other: &Self) {
        if let Some(allowed) = &other.allowed_account_ids {
            match &mut self.allowed_account_ids {
                Some(current) => current.retain(|id| allowed.contains(id)),
                None => self.allowed_account_ids = Some(allowed.clone()),
            }
        }
    }

    pub fn requested_group_id(&self) -> Option<&str> {
        self.requested_group_id.as_deref()
    }

    pub fn allows_candidate(&self, candidate: &RouteCandidate) -> bool {
        self.allowed_account_ids
            .as_ref()
            .is_none_or(|allowed| allowed.contains(&route_candidate_account_id(candidate)))
    }

    pub fn filter_candidates(&self, candidates: Vec<RouteCandidate>) -> Vec<RouteCandidate> {
        candidates
            .into_iter()
            .filter(|candidate| self.allows_candidate(candidate))
            .collect()
    }
}

impl RouteConfigSnapshot {
    pub fn access_key_group_constraint(&self, group_ids: &[String]) -> RouteAccountGroupConstraint {
        let selected: HashSet<&str> = group_ids.iter().map(String::as_str).collect();
        let allowed_account_ids = self
            .effective_account_groups()
            .iter()
            .filter(|group| group.enabled.unwrap_or(true) && selected.contains(group.id.as_str()))
            .flat_map(|group| {
                group
                    .provider_credential_ids
                    .iter()
                    .map(|id| id.trim().to_owned())
            })
            .collect();
        RouteAccountGroupConstraint {
            requested_group_id: None,
            allowed_account_ids: Some(allowed_account_ids),
        }
    }
}
