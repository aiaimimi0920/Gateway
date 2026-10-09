//! Validate additive policies before they can become an active route revision.
use crate::{console::document::RouteConfigDiagnostics, routing::config::RouteConfigYaml};

pub(super) fn validate(document: &RouteConfigYaml, diagnostics: &mut RouteConfigDiagnostics) {
    for (index, provider) in document.providers.iter().enumerate() {
        let path = format!("/providers/{index}");
        if provider.test_plans.len() > 32
            || provider
                .test_plans
                .iter()
                .map(|plan| &plan.id)
                .collect::<std::collections::HashSet<_>>()
                .len()
                != provider.test_plans.len()
        {
            diagnostics.push_error(
                "test_policy_invalid",
                format!("{path}/test_plans"),
                "At most 32 test plans with unique IDs are allowed per provider.",
            );
        }
        for (plan_index, plan) in provider.test_plans.iter().enumerate() {
            if let Err(error) = plan.validate(provider) {
                diagnostics.push_error(
                    "test_policy_invalid",
                    format!("{path}/test_plans/{plan_index}"),
                    error,
                );
            }
        }
        let mut policies = vec![
            (format!("{path}/test_policy"), provider.test_policy.as_ref()),
            (
                format!("{path}/default_account_test_policy"),
                provider.default_account_test_policy.as_ref(),
            ),
        ];
        if provider.subpool_test_policies.len() > 128 {
            diagnostics.push_error(
                "test_policy_invalid",
                format!("{path}/subpool_test_policies"),
                "At most 128 subpool policies are allowed.",
            );
        }
        for (id, policy) in &provider.subpool_test_policies {
            if !crate::credential_test_policy::exact_model(id) {
                diagnostics.push_error(
                    "test_policy_invalid",
                    format!("{path}/subpool_test_policies"),
                    "Invalid subpool policy ID.",
                );
            }
            policies.push((
                format!(
                    "{path}/subpool_test_policies/{}",
                    crate::console::document::encode_pointer_segment(id)
                ),
                Some(policy),
            ));
        }
        for (credential_index, credential) in provider.credentials.iter().enumerate() {
            policies.push((
                format!("{path}/credentials/{credential_index}/test_policy"),
                credential.test_policy.as_ref(),
            ));
        }
        for (path, policy) in policies {
            if let Some(error) = policy.and_then(|policy| policy.validate().err()) {
                diagnostics.push_error("test_policy_invalid", path, error);
            }
        }
    }
}
