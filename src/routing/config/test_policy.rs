//! Pool-local single-parent policy inheritance; top-level routing groups are unrelated.
use super::*;
use crate::credential_test_policy::CredentialTestPolicy;
#[cfg(test)]
mod tests;

impl RouteConfigSnapshot {
    pub(super) fn apply_test_policy_schedules(
        &self,
        mut legacy: Vec<ScheduledCredentialProbeTarget>,
    ) -> Vec<ScheduledCredentialProbeTarget> {
        for provider in &self.compiled.providers {
            for target in self
                .credential_probe_targets_for_provider(&provider.id)
                .unwrap_or_default()
            {
                if let Some((policy, _)) = self.credential_test_policy(&target) {
                    legacy.retain(|scheduled| {
                        scheduled.target.provider_id != target.provider_id
                            || scheduled.target.credential_id != target.credential_id
                    });
                    if policy.automatic_enabled && policy.validate().is_ok() {
                        legacy.push(ScheduledCredentialProbeTarget {
                            plan_id: None,
                            target,
                            interval_minutes: policy.interval_minutes,
                        });
                    }
                }
            }
        }
        self.append_named_test_schedules(&mut legacy);
        legacy
    }
    pub(crate) fn credential_test_policy(
        &self,
        target: &CredentialProbeTarget,
    ) -> Option<(CredentialTestPolicy, String)> {
        let provider = self
            .document()
            .providers
            .iter()
            .find(|p| p.id == target.provider_id)?;
        let credential = provider
            .credentials
            .iter()
            .enumerate()
            .find(|(index, credential)| {
                effective_credential_id(&provider.id, *index, credential) == target.credential_id
            })
            .map(|(_, credential)| credential);
        if let Some(policy) = credential.and_then(|c| c.test_policy.as_ref()).or_else(|| {
            (provider.credentials.is_empty())
                .then_some(provider.default_account_test_policy.as_ref())
                .flatten()
        }) {
            return Some((policy.clone(), "account".into()));
        }
        if let Some((id, policy)) = credential
            .and_then(|c| c.credential_identity_category_id.as_deref())
            .and_then(|id| {
                provider
                    .subpool_test_policies
                    .get(id)
                    .map(|policy| (id, policy))
            })
        {
            return Some((policy.clone(), format!("subpool:{id}")));
        }
        provider
            .test_policy
            .as_ref()
            .map(|policy| (policy.clone(), "pool".into()))
    }

    pub(crate) fn credential_subpool_id(&self, target: &CredentialProbeTarget) -> Option<&str> {
        let provider = self
            .document()
            .providers
            .iter()
            .find(|p| p.id == target.provider_id)?;
        provider
            .credentials
            .iter()
            .enumerate()
            .find(|(index, credential)| {
                effective_credential_id(&provider.id, *index, credential) == target.credential_id
            })
            .and_then(|(_, credential)| credential.credential_identity_category_id.as_deref())
    }
}
