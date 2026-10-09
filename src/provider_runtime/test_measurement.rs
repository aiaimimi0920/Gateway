//! Versioned test-set identity and end-to-end per-model observations; old results stay optional.
use super::{test_quota::TestQuotaObservation, test_results::TestCaseResult};
use crate::credential_test_policy::CredentialTestPolicy;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestModelMeasurement {
    pub test_set_id: String,
    pub elapsed_ms: Option<u64>,
    pub quota: TestQuotaObservation,
}

pub(super) fn test_set_id(plan: &CredentialTestPolicy) -> String {
    let mut cases: Vec<_> = plan.cases.iter().filter(|case| case.enabled).collect();
    cases.sort_by(|a, b| a.id.cmp(&b.id));
    hex::encode(Sha256::digest(
        serde_json::to_vec(&("weighted-v1", cases)).expect("test cases"),
    ))
}

pub(super) fn elapsed(model: &str, cases: &[TestCaseResult]) -> Option<u64> {
    let rows: Vec<_> = cases.iter().filter(|case| case.model == model).collect();
    if rows.is_empty() {
        return None;
    }
    rows.into_iter()
        .try_fold(0_u64, |sum, row| sum.checked_add(row.elapsed_ms?))
}
