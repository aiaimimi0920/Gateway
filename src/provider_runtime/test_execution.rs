//! Shared automatic/manual matrix execution. No fallback, hidden retries or quality penalties.
use super::test_results::{
    case_result, checked_at, model_assessment, TestAssessment, TestCaseResult, TestRunResult,
};
use super::{ConsoleProbeRequest, ProviderPayloadProbeStatus};
use crate::{
    credential_test_policy::{exact_model, CredentialTestPolicy, ModelSelection},
    routing::config::CredentialProbeTarget,
    state::AppState,
};
use std::{collections::BTreeSet, time::Instant};

pub(crate) struct RoundBudget {
    deadline: Instant,
    admitted: usize,
    revision: String,
}
impl RoundBudget {
    pub(crate) fn new(revision: &str) -> Self {
        Self {
            deadline: Instant::now() + std::time::Duration::from_secs(300),
            admitted: 0,
            revision: revision.into(),
        }
    }
    pub(crate) fn available(&self) -> bool {
        self.admitted < 128 && Instant::now() < self.deadline
    }
    pub(crate) fn matches(&self, state: &AppState) -> bool {
        state.route_config.snapshot().revision().id() == self.revision
    }
    pub(crate) fn admit(&mut self) -> bool {
        if !self.available() {
            return false;
        }
        self.admitted += 1;
        true
    }
}

pub(crate) fn plan_for_request(
    state: &AppState,
    target: &CredentialProbeTarget,
    request: &ConsoleProbeRequest,
) -> Option<(CredentialTestPolicy, String)> {
    let saved = state.route_config.snapshot().credential_test_policy(target);
    let Some(temporary) = request.test_plan.as_ref() else {
        return if request.prompt.is_empty() {
            saved
        } else {
            None
        };
    };
    let lower_override = saved.filter(|(_, source)| match request.scope.as_ref() {
        Some(super::ConsoleTestScope::Account { .. }) => false,
        Some(super::ConsoleTestScope::Subpool { .. }) => source == "account",
        _ => source != "pool",
    });
    lower_override.or_else(|| {
        Some((
            temporary.clone(),
            match request.scope.as_ref() {
                Some(super::ConsoleTestScope::Account { .. }) => "account:temporary",
                Some(super::ConsoleTestScope::Subpool { .. }) => "subpool:temporary",
                _ => "pool:temporary",
            }
            .into(),
        ))
    })
}

pub(crate) fn models_for_plan(
    state: &AppState,
    target: &CredentialProbeTarget,
    plan: &CredentialTestPolicy,
) -> Vec<String> {
    if plan.model_selection == ModelSelection::Selected {
        return plan.models.clone();
    }
    let Some(provider) = state
        .route_config
        .snapshot()
        .get_providers()
        .into_iter()
        .find(|p| p.id == target.provider_id)
    else {
        return vec![];
    };
    let credential = provider
        .credential_pool
        .iter()
        .find(|c| c.id == target.credential_id);
    let declared = credential
        .map(|c| c.supported_models.as_slice())
        .filter(|models| !models.is_empty())
        .unwrap_or(&provider.supported_models);
    let models: BTreeSet<_> = declared
        .iter()
        .map(String::as_str)
        .chain(
            target
                .payload
                .default_model
                .as_deref()
                .filter(|_| declared.is_empty()),
        )
        .filter(|model| exact_model(model))
        .map(str::to_string)
        .collect();
    models.into_iter().collect()
}

pub(crate) async fn run(
    state: &AppState,
    target: &CredentialProbeTarget,
    plan: &CredentialTestPolicy,
    source: &str,
    mode: &str,
    budget: &mut RoundBudget,
) -> TestRunResult {
    let mut models = models_for_plan(state, target, plan);
    let selected: Vec<_> = plan.cases.iter().filter(|case| case.enabled).collect();
    let over_limit = models.len().saturating_mul(selected.len()) > 128;
    if over_limit {
        models.truncate(128);
    }
    let mut cases = Vec::new();
    let mut measurements = std::collections::HashMap::new();
    let stopped = if plan.validate().is_err() {
        Some("Invalid saved test plan.")
    } else if !target.enabled {
        Some("Credential is disabled.")
    } else if over_limit {
        Some("More than 128 model/case combinations; select fewer models or cases.")
    } else {
        None
    };
    if !over_limit {
        for model in &models {
            let observe = stopped.is_none() && budget.available() && budget.matches(state);
            let before = if observe {
                super::test_quota::capture(state, target).await
            } else {
                None
            };
            for case in &selected {
                let reason = stopped
                    .or_else(|| {
                        (!budget.matches(state)).then_some("Route revision changed; call not sent.")
                    })
                    .or_else(|| {
                        (!budget.admit())
                            .then_some("Round call/time budget reached; call not sent.")
                    });
                if let Some(reason) = reason {
                    cases.push(TestCaseResult {
                        case_id: case.id.clone(),
                        name: case.name.clone(),
                        model: model.clone(),
                        difficulty: case.difficulty,
                        status: "not-run".into(),
                        answer: None,
                        expected_answer: case.expected_answer.clone(),
                        correct: None,
                        message: reason.into(),
                        elapsed_ms: None,
                    });
                    continue;
                }
                let request = ConsoleProbeRequest {
                    prompt: case.prompt.clone(),
                    model: Some(model.clone()),
                    credential_ids: None,
                    test_plan: None,
                    scope: None,
                    plan_id: None,
                };
                let started = Instant::now();
                let (_, report) =
                    super::probe_console_target_with_request(state, target, Some(&request)).await;
                let mut result = case_result(case, model, &report);
                result.elapsed_ms =
                    Some(started.elapsed().as_millis().min(u64::MAX as u128) as u64);
                cases.push(result);
            }
            let after = if observe && before.is_some() {
                super::test_quota::capture(state, target).await
            } else {
                None
            };
            measurements.insert(
                model.clone(),
                super::test_measurement::TestModelMeasurement {
                    test_set_id: super::test_measurement::test_set_id(plan),
                    elapsed_ms: super::test_measurement::elapsed(model, &cases),
                    quota: super::test_quota::observation(before, after),
                },
            );
        }
    }
    let assessments = models
        .iter()
        .map(|model| {
            let mut result = model_assessment(model, &cases);
            result.measurement = measurements.remove(model);
            result
        })
        .collect();
    let passed = cases.iter().any(|case| case.status == "passed");
    let failed = cases.iter().any(|case| case.status == "failed");
    let status = if passed {
        ProviderPayloadProbeStatus::Passed
    } else if failed {
        ProviderPayloadProbeStatus::Failed
    } else {
        ProviderPayloadProbeStatus::Unsupported
    };
    let message = if over_limit {
        "Test matrix exceeds 128 calls per account; no calls sent.".into()
    } else if models.is_empty() {
        "No exact configured models; choose explicit models or configure supported_models/default_model.".into()
    } else {
        format!(
            "Test round: {} replies, {} failures, {} unexecuted/unsupported.",
            cases.iter().filter(|c| c.status == "passed").count(),
            cases.iter().filter(|c| c.status == "failed").count(),
            cases
                .iter()
                .filter(|c| c.status != "passed" && c.status != "failed")
                .count()
        )
    };
    TestRunResult {
        credential_id: target.credential_id.clone(),
        provider_id: target.provider_id.clone(),
        probe_point: "Configured model/test matrix".into(),
        status,
        message,
        checked_at: checked_at(),
        assessment: Some(TestAssessment {
            plan_id: None,
            policy_source: source.into(),
            mode: mode.into(),
            models: assessments,
            cases,
        }),
    }
}
