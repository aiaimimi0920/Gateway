//! Provider-local named-plan expansion; each plan uses its own policy and schedule identity.
use super::{CredentialProbeTarget, RouteConfigSnapshot, ScheduledCredentialProbeTarget};
use crate::credential_test_plan::{CredentialTestPlan, TestScope};

impl RouteConfigSnapshot {
    pub(crate) fn named_test_plan(&self, provider: &str, id: &str) -> Option<&CredentialTestPlan> {
        self.document()
            .providers
            .iter()
            .find(|entry| entry.id == provider)?
            .test_plans
            .iter()
            .find(|plan| plan.id == id)
    }

    pub(crate) fn test_scope_matches(
        &self,
        target: &CredentialProbeTarget,
        scope: &TestScope,
    ) -> bool {
        match scope {
            TestScope::Pool => true,
            TestScope::Account { id } => target.credential_id == *id,
            TestScope::Subpool { id } => self.credential_subpool_id(target) == Some(id.as_str()),
        }
    }

    pub(crate) fn test_plan_matches(
        &self,
        target: &CredentialProbeTarget,
        plan: &CredentialTestPlan,
    ) -> bool {
        plan.scopes
            .iter()
            .any(|scope| self.test_scope_matches(target, scope))
    }

    pub(super) fn append_named_test_schedules(
        &self,
        targets: &mut Vec<ScheduledCredentialProbeTarget>,
    ) {
        for provider in &self.document().providers {
            let accounts = self
                .credential_probe_targets_for_provider(&provider.id)
                .unwrap_or_default();
            for plan in provider
                .test_plans
                .iter()
                .take(32)
                .filter(|plan| plan.policy.automatic_enabled && plan.validate(provider).is_ok())
            {
                for target in accounts
                    .iter()
                    .filter(|target| self.test_plan_matches(target, plan))
                {
                    targets.push(ScheduledCredentialProbeTarget {
                        target: target.clone(),
                        interval_minutes: plan.policy.interval_minutes,
                        plan_id: Some(plan.id.clone()),
                    });
                }
            }
        }
    }
}
